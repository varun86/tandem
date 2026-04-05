use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tandem_types::EngineEvent;

const MAX_INLINE_ROLLBACK_SNAPSHOT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCheckpointOutcome {
    Succeeded,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCheckpointSnapshotStatus {
    InlineText,
    TooLarge,
    Binary,
    NotNeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationCheckpointRollbackSnapshot {
    pub status: MutationCheckpointSnapshotStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationCheckpointFileRecord {
    pub path: String,
    pub resolved_path: String,
    pub existed_before: bool,
    pub existed_after: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_hash: Option<String>,
    pub changed: bool,
    pub rollback_snapshot: MutationCheckpointRollbackSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationCheckpointRecord {
    pub session_id: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_cwd: Option<String>,
    pub outcome: MutationCheckpointOutcome,
    pub files: Vec<MutationCheckpointFileRecord>,
    pub file_count: usize,
    pub changed_file_count: usize,
}

#[derive(Debug, Clone)]
pub struct MutationCheckpointBaseline {
    tool: String,
    workspace_root: Option<String>,
    effective_cwd: Option<String>,
    files: Vec<MutationCheckpointFileBaseline>,
}

#[derive(Debug, Clone)]
struct MutationCheckpointFileBaseline {
    path: String,
    resolved_path: PathBuf,
    existed_before: bool,
    before_hash: Option<String>,
    rollback_snapshot: MutationCheckpointRollbackSnapshot,
}

pub fn mutation_checkpoint_event(record: MutationCheckpointRecord) -> EngineEvent {
    EngineEvent::new(
        "mutation.checkpoint.recorded",
        json!({
            "sessionID": record.session_id.clone(),
            "messageID": record.message_id.clone(),
            "tool": record.tool.clone(),
            "record": record,
        }),
    )
}

pub fn prepare_mutation_checkpoint(tool: &str, args: &Value) -> Option<MutationCheckpointBaseline> {
    if !is_mutating_tool(tool) {
        return None;
    }
    let files = extract_mutation_target_paths(tool, args);
    if files.is_empty() {
        return None;
    }
    let workspace_root = string_field(args, "__workspace_root");
    let effective_cwd = string_field(args, "__effective_cwd");
    let files = files
        .into_iter()
        .map(|path| {
            let resolved_path =
                resolve_workspace_path(&path, effective_cwd.as_deref(), workspace_root.as_deref());
            let (existed_before, before_hash, rollback_snapshot) =
                snapshot_path_baseline(&resolved_path);
            MutationCheckpointFileBaseline {
                path,
                resolved_path,
                existed_before,
                before_hash,
                rollback_snapshot,
            }
        })
        .collect::<Vec<_>>();
    Some(MutationCheckpointBaseline {
        tool: tool.to_string(),
        workspace_root,
        effective_cwd,
        files,
    })
}

pub fn finalize_mutation_checkpoint_record(
    session_id: &str,
    message_id: &str,
    tool_call_id: Option<&str>,
    baseline: &MutationCheckpointBaseline,
    outcome: MutationCheckpointOutcome,
) -> MutationCheckpointRecord {
    let files = baseline
        .files
        .iter()
        .map(|file| {
            let (existed_after, after_hash) = snapshot_path_state(&file.resolved_path);
            let changed = file.existed_before != existed_after || file.before_hash != after_hash;
            MutationCheckpointFileRecord {
                path: file.path.clone(),
                resolved_path: file.resolved_path.to_string_lossy().to_string(),
                existed_before: file.existed_before,
                existed_after,
                before_hash: file.before_hash.clone(),
                after_hash,
                changed,
                rollback_snapshot: file.rollback_snapshot.clone(),
            }
        })
        .collect::<Vec<_>>();
    let changed_file_count = files.iter().filter(|file| file.changed).count();
    MutationCheckpointRecord {
        session_id: session_id.to_string(),
        message_id: message_id.to_string(),
        tool_call_id: tool_call_id.map(str::to_string),
        tool: baseline.tool.clone(),
        workspace_root: baseline.workspace_root.clone(),
        effective_cwd: baseline.effective_cwd.clone(),
        file_count: files.len(),
        changed_file_count,
        files,
        outcome,
    }
}

fn is_mutating_tool(tool: &str) -> bool {
    matches!(
        tool.trim(),
        "write" | "edit" | "apply_patch" | "delete" | "delete_file"
        // bash/shell: included because redirect operations (>) write files outside the
        // normal write/edit path and should also get pre-mutation snapshots.
        | "bash" | "shell"
    )
}

fn extract_mutation_target_paths(tool: &str, args: &Value) -> Vec<String> {
    let mut paths = match tool.trim() {
        "write" | "edit" | "delete" | "delete_file" => string_fields(args, &["path", "file_path"]),
        "apply_patch" => args
            .get("patchText")
            .and_then(Value::as_str)
            .map(extract_apply_patch_paths)
            .unwrap_or_default(),
        "bash" | "shell" => {
            // Extract redirect targets from the shell command string.
            // Matches patterns like:  cmd > /path/to/file  and  cmd >> /path/to/file
            let command = args
                .get("command")
                .or_else(|| args.get("cmd"))
                .and_then(Value::as_str)
                .unwrap_or("");
            extract_bash_redirect_targets(command)
        }
        _ => Vec::new(),
    };
    paths.sort();
    paths.dedup();
    paths
}

fn extract_apply_patch_paths(patch: &str) -> Vec<String> {
    let mut paths = HashSet::new();
    for line in patch.lines() {
        let trimmed = line.trim();
        let marker = trimmed
            .strip_prefix("*** Add File: ")
            .or_else(|| trimmed.strip_prefix("*** Update File: "))
            .or_else(|| trimmed.strip_prefix("*** Delete File: "));
        if let Some(path) = marker.map(str::trim).filter(|value| !value.is_empty()) {
            paths.insert(path.to_string());
        }
    }
    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort();
    paths
}

/// Extract file paths that a bash command will write to via shell redirect operators (`>` / `>>`).
/// Only captures paths that look like actual filesystem paths (start with `/`, `./`, or `~/`).
/// Does not attempt full shell parsing — this is best-effort for checkpoint snapshotting.
fn extract_bash_redirect_targets(command: &str) -> Vec<String> {
    let mut targets = Vec::new();
    // Split on redirect operators, then grab the first token of each right-hand side.
    // Order matters: check ">>" before ">" to avoid splitting ">>" into ">" + ">".
    for part in command.split(">>").flat_map(|s| s.split('>')) {
        let candidate = part.trim().split_whitespace().next().unwrap_or("").trim();
        if candidate.starts_with('/') || candidate.starts_with("./") || candidate.starts_with("~/")
        {
            targets.push(candidate.to_string());
        }
    }
    targets.sort();
    targets.dedup();
    targets
}

fn resolve_workspace_path(
    path: &str,
    effective_cwd: Option<&str>,
    workspace_root: Option<&str>,
) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        return candidate;
    }
    if let Some(cwd) = effective_cwd {
        return PathBuf::from(cwd).join(path);
    }
    if let Some(root) = workspace_root {
        return PathBuf::from(root).join(path);
    }
    candidate
}

fn snapshot_path_state(path: &Path) -> (bool, Option<String>) {
    match read_path_bytes(path) {
        Ok(bytes) => (true, Some(hash_bytes(&bytes))),
        Err(_) => (false, None),
    }
}

fn snapshot_path_baseline(
    path: &Path,
) -> (bool, Option<String>, MutationCheckpointRollbackSnapshot) {
    match read_path_bytes(path) {
        Ok(bytes) => (
            true,
            Some(hash_bytes(&bytes)),
            rollback_snapshot_from_bytes(&bytes),
        ),
        Err(_) => (
            false,
            None,
            MutationCheckpointRollbackSnapshot {
                status: MutationCheckpointSnapshotStatus::NotNeeded,
                content: None,
                byte_count: None,
            },
        ),
    }
}

fn rollback_snapshot_from_bytes(bytes: &[u8]) -> MutationCheckpointRollbackSnapshot {
    if bytes.len() > MAX_INLINE_ROLLBACK_SNAPSHOT_BYTES {
        return MutationCheckpointRollbackSnapshot {
            status: MutationCheckpointSnapshotStatus::TooLarge,
            content: None,
            byte_count: Some(bytes.len()),
        };
    }
    match String::from_utf8(bytes.to_vec()) {
        Ok(content) => MutationCheckpointRollbackSnapshot {
            status: MutationCheckpointSnapshotStatus::InlineText,
            content: Some(content),
            byte_count: Some(bytes.len()),
        },
        Err(_) => MutationCheckpointRollbackSnapshot {
            status: MutationCheckpointSnapshotStatus::Binary,
            content: None,
            byte_count: Some(bytes.len()),
        },
    }
}

fn read_path_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn string_field(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_fields(args: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| string_field(args, key))
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_mutation_checkpoint_extracts_apply_patch_paths() {
        let baseline = prepare_mutation_checkpoint(
            "apply_patch",
            &json!({
                "patchText": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-a\n+b\n*** Add File: src/new.rs\n+hello\n*** End Patch\n",
                "__workspace_root": "/workspace"
            }),
        )
        .expect("baseline");

        let paths = baseline
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["src/lib.rs", "src/new.rs"]);
    }

    #[test]
    fn finalize_mutation_checkpoint_detects_file_creation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("created.txt");
        let baseline = prepare_mutation_checkpoint(
            "write",
            &json!({
                "path": "created.txt",
                "__effective_cwd": dir.path().to_string_lossy().to_string()
            }),
        )
        .expect("baseline");

        std::fs::write(&path, "hello world").expect("write created file");

        let record = finalize_mutation_checkpoint_record(
            "session-1",
            "message-1",
            Some("call-1"),
            &baseline,
            MutationCheckpointOutcome::Succeeded,
        );

        assert_eq!(record.file_count, 1);
        assert_eq!(record.changed_file_count, 1);
        assert!(record.files[0].changed);
        assert!(!record.files[0].existed_before);
        assert!(record.files[0].existed_after);
        assert_eq!(
            record.files[0].rollback_snapshot.status,
            MutationCheckpointSnapshotStatus::NotNeeded
        );
    }

    #[test]
    fn finalize_mutation_checkpoint_captures_inline_before_snapshot_for_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tracked.txt");
        std::fs::write(&path, "before").expect("write initial file");
        let baseline = prepare_mutation_checkpoint(
            "edit",
            &json!({
                "path": "tracked.txt",
                "__effective_cwd": dir.path().to_string_lossy().to_string()
            }),
        )
        .expect("baseline");

        std::fs::write(&path, "after").expect("write updated file");

        let record = finalize_mutation_checkpoint_record(
            "session-1",
            "message-1",
            Some("call-1"),
            &baseline,
            MutationCheckpointOutcome::Succeeded,
        );

        assert_eq!(
            record.files[0].rollback_snapshot.status,
            MutationCheckpointSnapshotStatus::InlineText
        );
        assert_eq!(
            record.files[0].rollback_snapshot.content.as_deref(),
            Some("before")
        );
    }
}
