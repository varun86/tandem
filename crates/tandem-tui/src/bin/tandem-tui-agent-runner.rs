use anyhow::Context;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Serialize)]
struct AgentStep {
    goal: String,
    step: usize,
    actions: Vec<AgentAction>,
    #[serde(default)]
    assertions: Vec<AgentAssertion>,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    bug: Option<BugReport>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AgentAction {
    #[serde(rename = "type")]
    action_type: String,
    value: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
struct AgentAssertion {
    #[serde(rename = "type")]
    assertion_type: String,
    value: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct BugReport {
    title: String,
    expected: String,
    actual: String,
    repro_actions: Vec<String>,
    #[serde(default)]
    evidence: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct StepResult {
    step: usize,
    ok: bool,
    assertion_failures: Vec<String>,
    artifact_dir: String,
}

#[derive(Debug, Serialize)]
struct Observation {
    frame_text: String,
    debug_hint: Option<String>,
    artifact_dir: String,
}

struct PtyRunner {
    child: Box<dyn portable_pty::Child + Send>,
    writer: Box<dyn Write + Send>,
    reader_rx: Receiver<Vec<u8>>,
    parser: vt100::Parser,
}

fn resolve_tui_binary() -> anyhow::Result<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(raw) = std::env::var("TANDEM_TUI_BIN") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(trimmed));
        }
    }
    if let Ok(raw) = std::env::var("CARGO_BIN_EXE_tandem-tui") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(trimmed));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("tandem-tui"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("target/debug/tandem-tui"));
        candidates.push(cwd.join("target/release/tandem-tui"));
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join("tandem-tui"));
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().to_string());
        }
    }

    anyhow::bail!(
        "Unable to locate tandem-tui binary. Set TANDEM_TUI_BIN to an absolute path, \
         or build it first with `cargo build -p tandem-tui`."
    )
}

impl PtyRunner {
    fn spawn() -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let bin = resolve_tui_binary()?;
        let mut cmd = CommandBuilder::new(bin);
        cmd.env("TANDEM_TUI_TEST_MODE", "1");
        cmd.env("TANDEM_TUI_TEST_SKIP_ENGINE", "1");
        cmd.env("TANDEM_TUI_SYNC_RENDER", "off");
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let (tx, rx) = mpsc::channel::<Vec<u8>>();

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            child,
            writer,
            reader_rx: rx,
            parser: vt100::Parser::new(40, 120, 0),
        })
    }

    fn drain_output(&mut self) {
        while let Ok(chunk) = self.reader_rx.try_recv() {
            self.parser.process(&chunk);
        }
    }

    fn frame_text(&self) -> String {
        self.parser.screen().contents()
    }

    fn send_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.writer.write_all(text.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    fn send_key_token(&mut self, token: &str) -> anyhow::Result<()> {
        let key = token.trim();
        let seq = match key {
            "UP" => "\x1b[A".to_string(),
            "DOWN" => "\x1b[B".to_string(),
            "LEFT" => "\x1b[D".to_string(),
            "RIGHT" => "\x1b[C".to_string(),
            "ENTER" => "\r".to_string(),
            "ESC" => "\x1b".to_string(),
            "TAB" => "\t".to_string(),
            "BACKTAB" => "\x1b[Z".to_string(),
            "F1" => "\x1bOP".to_string(),
            _ if key.starts_with("CHAR:") => key.replacen("CHAR:", "", 1),
            _ if key.starts_with("CTRL:") => {
                let raw = key.replacen("CTRL:", "", 1);
                let c = raw.chars().next().context("CTRL key missing char")?;
                let upper = c.to_ascii_uppercase() as u8;
                ((upper & 0x1f) as char).to_string()
            }
            _ if key.starts_with("ALT:") => {
                let raw = key.replacen("ALT:", "", 1);
                let c = raw.chars().next().context("ALT key missing char")?;
                format!("\x1b{}", c)
            }
            _ => anyhow::bail!("unsupported key token: {key}"),
        };

        self.writer.write_all(seq.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PtyRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

fn make_artifact_dir(base: Option<&str>) -> anyhow::Result<PathBuf> {
    let dir = if let Some(path) = base {
        PathBuf::from(path)
    } else {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();
        PathBuf::from(format!(".tmp/tui-agent-runs/{}", ts))
    };
    fs::create_dir_all(&dir)?;
    fs::create_dir_all(dir.join("frame_history"))?;
    Ok(dir)
}

fn append_json_line(path: &Path, value: &serde_json::Value) -> anyhow::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

fn debug_hint(frame: &str) -> Option<String> {
    frame
        .lines()
        .find(|line| line.contains("TEST modal="))
        .map(|s| s.trim().to_string())
}

fn run_assertions(frame: &str, assertions: &[AgentAssertion]) -> Vec<String> {
    let mut failures = Vec::new();
    for assertion in assertions {
        match assertion.assertion_type.as_str() {
            "contains" => {
                if !frame.contains(&assertion.value) {
                    failures.push(format!("missing expected text: {}", assertion.value));
                }
            }
            "not_contains" => {
                if frame.contains(&assertion.value) {
                    failures.push(format!("unexpected text present: {}", assertion.value));
                }
            }
            other => failures.push(format!("unsupported assertion type: {}", other)),
        }
    }
    failures
}

#[derive(Default)]
struct CliArgs {
    scenario_file: Option<String>,
    replay_run: Option<String>,
    artifact_dir: Option<String>,
    record_scenario_out: Option<String>,
    max_steps: Option<usize>,
    startup_wait_ms: u64,
}

fn parse_args() -> anyhow::Result<CliArgs> {
    let mut out = CliArgs {
        startup_wait_ms: 250,
        ..CliArgs::default()
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--scenario-file" => {
                let Some(path) = it.next() else {
                    anyhow::bail!("--scenario-file requires a path");
                };
                out.scenario_file = Some(path);
            }
            "--replay-run" => {
                let Some(path) = it.next() else {
                    anyhow::bail!("--replay-run requires a path to run.jsonl");
                };
                out.replay_run = Some(path);
            }
            "--artifact-dir" => {
                let Some(path) = it.next() else {
                    anyhow::bail!("--artifact-dir requires a path");
                };
                out.artifact_dir = Some(path);
            }
            "--record-scenario-out" => {
                let Some(path) = it.next() else {
                    anyhow::bail!("--record-scenario-out requires a path");
                };
                out.record_scenario_out = Some(path);
            }
            "--max-steps" => {
                let Some(raw) = it.next() else {
                    anyhow::bail!("--max-steps requires a number");
                };
                let parsed = raw
                    .parse::<usize>()
                    .with_context(|| format!("invalid --max-steps value: {}", raw))?;
                out.max_steps = Some(parsed);
            }
            "--startup-wait-ms" => {
                let Some(raw) = it.next() else {
                    anyhow::bail!("--startup-wait-ms requires a number");
                };
                out.startup_wait_ms = raw
                    .parse::<u64>()
                    .with_context(|| format!("invalid --startup-wait-ms value: {}", raw))?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: tandem-tui-agent-runner [--scenario-file <path>] [--replay-run <run.jsonl>] [--artifact-dir <path>] [--record-scenario-out <path>] [--max-steps <n>] [--startup-wait-ms <ms>]"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {}", other),
        }
    }
    Ok(out)
}

fn decode_step_line(line: &str) -> anyhow::Result<AgentStep> {
    if let Ok(step) = serde_json::from_str::<AgentStep>(line) {
        return Ok(step);
    }
    let value: serde_json::Value = serde_json::from_str(line)
        .with_context(|| "line is neither AgentStep nor JSON value".to_string())?;
    if let Some(step_input) = value.get("step_input") {
        return serde_json::from_value(step_input.clone())
            .with_context(|| "invalid step_input payload".to_string());
    }
    let goal = value
        .get("goal")
        .and_then(|v| v.as_str())
        .unwrap_or("replay step")
        .to_string();
    let step_num = value.get("step").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let actions: Vec<AgentAction> = serde_json::from_value(
        value
            .get("actions")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
    )
    .with_context(|| "invalid actions payload".to_string())?;
    let assertions: Vec<AgentAssertion> = serde_json::from_value(
        value
            .get("assertions")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
    )
    .unwrap_or_default();
    let notes = value
        .get("notes")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let bug = value
        .get("bug")
        .cloned()
        .and_then(|v| serde_json::from_value::<BugReport>(v).ok());
    Ok(AgentStep {
        goal,
        step: step_num,
        actions,
        assertions,
        notes,
        bug,
    })
}

fn process_step(
    runner: &mut PtyRunner,
    artifact_dir: &Path,
    run_log_path: &Path,
    record_scenario_out: Option<&Path>,
    frame_counter: &mut usize,
    step: AgentStep,
) -> anyhow::Result<()> {
    if let Some(path) = record_scenario_out {
        append_json_line(path, &serde_json::to_value(&step)?)?;
    }

    let start = Instant::now();
    runner.drain_output();
    let frame_before = runner.frame_text();
    let before_path = artifact_dir
        .join("frame_history")
        .join(format!("{:05}_before.txt", *frame_counter));
    fs::write(before_path, &frame_before)?;

    let mut action_errors: Vec<String> = Vec::new();
    for action in &step.actions {
        match action.action_type.as_str() {
            "key" => {
                if let Some(token) = action.value.as_str() {
                    if let Err(err) = runner.send_key_token(token) {
                        action_errors.push(err.to_string());
                    }
                } else {
                    action_errors.push("key action requires string value".to_string());
                }
            }
            "text" => {
                if let Some(text) = action.value.as_str() {
                    if let Err(err) = runner.send_text(text) {
                        action_errors.push(err.to_string());
                    }
                } else {
                    action_errors.push("text action requires string value".to_string());
                }
            }
            "wait_ms" => {
                if let Some(ms) = action.value.as_u64() {
                    std::thread::sleep(Duration::from_millis(ms));
                } else {
                    action_errors.push("wait_ms action requires numeric value".to_string());
                }
            }
            other => action_errors.push(format!("unsupported action type: {}", other)),
        }
        std::thread::sleep(Duration::from_millis(20));
        runner.drain_output();
    }

    std::thread::sleep(Duration::from_millis(60));
    runner.drain_output();
    let frame_after = runner.frame_text();
    let after_path = artifact_dir
        .join("frame_history")
        .join(format!("{:05}_after.txt", *frame_counter));
    fs::write(after_path, &frame_after)?;
    *frame_counter += 1;

    let mut assertion_failures = run_assertions(&frame_after, &step.assertions);
    assertion_failures.extend(action_errors);

    let step_log = json!({
        "goal": step.goal,
        "step": step.step,
        "notes": step.notes,
        "duration_ms": start.elapsed().as_millis(),
        "actions": step.actions,
        "assertions": step.assertions,
        "step_input": step,
        "assertion_failures": assertion_failures,
    });
    append_json_line(run_log_path, &step_log)?;

    fs::write(artifact_dir.join("last_frame.txt"), &frame_after)?;

    if let Some(bug) = step.bug {
        fs::write(
            artifact_dir.join("bug_report.json"),
            serde_json::to_vec_pretty(&bug)?,
        )?;
        fs::write(artifact_dir.join("frame_before.txt"), frame_before)?;
        fs::write(artifact_dir.join("frame_after.txt"), frame_after)?;
    }

    let result = StepResult {
        step: step.step,
        ok: assertion_failures.is_empty(),
        assertion_failures,
        artifact_dir: artifact_dir.display().to_string(),
    };
    println!("{}", serde_json::to_string(&result)?);

    let current_frame = runner.frame_text();
    let obs = Observation {
        frame_text: current_frame.clone(),
        debug_hint: debug_hint(&current_frame),
        artifact_dir: artifact_dir.display().to_string(),
    };
    println!("{}", serde_json::to_string(&obs)?);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let mut runner = PtyRunner::spawn().context("failed to spawn tandem-tui in PTY")?;
    let artifact_dir = make_artifact_dir(args.artifact_dir.as_deref())?;
    let run_log_path = artifact_dir.join("run.jsonl");
    let record_scenario_out = args.record_scenario_out.as_ref().map(Path::new);

    std::thread::sleep(Duration::from_millis(args.startup_wait_ms));
    runner.drain_output();
    let frame = runner.frame_text();
    let initial_obs = Observation {
        frame_text: frame.clone(),
        debug_hint: debug_hint(&frame),
        artifact_dir: artifact_dir.display().to_string(),
    };
    println!("{}", serde_json::to_string(&initial_obs)?);

    let mut frame_counter: usize = 0;
    let mut processed_steps: usize = 0;
    if let Some(path) = args.replay_run {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read replay: {}", path))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(limit) = args.max_steps {
                if processed_steps >= limit {
                    break;
                }
            }
            let step = match decode_step_line(trimmed) {
                Ok(s) => s,
                Err(err) => {
                    let err_json = json!({ "error": format!("invalid replay line: {}", err) });
                    println!("{}", serde_json::to_string(&err_json)?);
                    continue;
                }
            };
            process_step(
                &mut runner,
                &artifact_dir,
                &run_log_path,
                record_scenario_out,
                &mut frame_counter,
                step,
            )?;
            processed_steps += 1;
        }
    } else if let Some(path) = args.scenario_file {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read scenario file: {}", path))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(limit) = args.max_steps {
                if processed_steps >= limit {
                    break;
                }
            }
            let step = match decode_step_line(trimmed) {
                Ok(s) => s,
                Err(err) => {
                    let err_json = json!({ "error": format!("invalid scenario line: {}", err) });
                    println!("{}", serde_json::to_string(&err_json)?);
                    continue;
                }
            };
            process_step(
                &mut runner,
                &artifact_dir,
                &run_log_path,
                record_scenario_out,
                &mut frame_counter,
                step,
            )?;
            processed_steps += 1;
        }
    } else {
        let stdin = std::io::stdin();
        for line in BufReader::new(stdin.lock()).lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(limit) = args.max_steps {
                if processed_steps >= limit {
                    break;
                }
            }
            let step = match decode_step_line(trimmed) {
                Ok(s) => s,
                Err(err) => {
                    let err_json = json!({ "error": format!("invalid step json: {}", err) });
                    println!("{}", serde_json::to_string(&err_json)?);
                    continue;
                }
            };
            process_step(
                &mut runner,
                &artifact_dir,
                &run_log_path,
                record_scenario_out,
                &mut frame_counter,
                step,
            )?;
            processed_steps += 1;
        }
    }

    Ok(())
}
