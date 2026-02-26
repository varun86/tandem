use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

pub enum TestKey {
    Enter,
    Esc,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    F1,
    Char(char),
    Ctrl(char),
    Alt(char),
}

pub struct TuiPtyHarness {
    child: Box<dyn portable_pty::Child + Send>,
    writer: Box<dyn Write + Send>,
    reader_rx: Receiver<Vec<u8>>,
    parser: vt100::Parser,
    frame_log: Vec<String>,
}

impl TuiPtyHarness {
    pub fn spawn_tandem_tui() -> anyhow::Result<Self> {
        let bin = std::env::var("CARGO_BIN_EXE_tandem-tui")
            .map_err(|_| anyhow::anyhow!("CARGO_BIN_EXE_tandem-tui is not set"))?;
        Self::spawn_command(&bin, &[])
    }

    pub fn spawn_command(bin: &str, args: &[&str]) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(bin);
        for arg in args {
            cmd.arg(*arg);
        }
        cmd.env("TANDEM_TUI_TEST_MODE", "1");
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
            frame_log: Vec::new(),
        })
    }

    pub fn send_key(&mut self, key: TestKey) -> anyhow::Result<()> {
        let sequence = match key {
            TestKey::Enter => "\r".to_string(),
            TestKey::Esc => "\x1b".to_string(),
            TestKey::Tab => "\t".to_string(),
            TestKey::BackTab => "\x1b[Z".to_string(),
            TestKey::Up => "\x1b[A".to_string(),
            TestKey::Down => "\x1b[B".to_string(),
            TestKey::Right => "\x1b[C".to_string(),
            TestKey::Left => "\x1b[D".to_string(),
            TestKey::F1 => "\x1bOP".to_string(),
            TestKey::Char(c) => c.to_string(),
            TestKey::Ctrl(c) => {
                let upper = c.to_ascii_uppercase();
                let code = (upper as u8) & 0x1f;
                (code as char).to_string()
            }
            TestKey::Alt(c) => format!("\x1b{}", c),
        };
        self.writer.write_all(sequence.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn send_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.writer.write_all(text.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn wait_for_text(&mut self, needle: &str, timeout: Duration) -> anyhow::Result<()> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.drain_output();
            let frame = self.screen_text();
            if frame.contains(needle) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        anyhow::bail!("timed out waiting for text: {needle}");
    }

    pub fn screen_text(&self) -> String {
        self.parser.screen().contents()
    }

    pub fn drain_output(&mut self) {
        while let Ok(chunk) = self.reader_rx.try_recv() {
            self.parser.process(&chunk);
            self.frame_log.push(self.parser.screen().contents());
            if self.frame_log.len() > 60 {
                let drain = self.frame_log.len() - 60;
                self.frame_log.drain(0..drain);
            }
        }
    }

    pub fn dump_artifacts(&self, dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("last_frame.txt"), self.screen_text())?;
        let history_dir = dir.join("frame_history");
        std::fs::create_dir_all(&history_dir)?;
        for (idx, frame) in self.frame_log.iter().enumerate() {
            let name = format!("{:03}.txt", idx);
            std::fs::write(history_dir.join(name), frame)?;
        }
        Ok(())
    }

    pub fn terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for TuiPtyHarness {
    fn drop(&mut self) {
        self.terminate();
    }
}
