use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::util::time::now_ms;

const RECEIPT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AutomationAttemptReceiptRecord {
    pub(crate) version: u32,
    pub(crate) run_id: String,
    pub(crate) node_id: String,
    pub(crate) attempt: u32,
    pub(crate) session_id: String,
    pub(crate) seq: u64,
    pub(crate) ts_ms: u64,
    pub(crate) event_type: String,
    pub(crate) payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AutomationAttemptReceiptEventInput {
    pub(crate) event_type: String,
    pub(crate) payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AutomationAttemptReceiptDraft {
    pub(crate) run_id: String,
    pub(crate) node_id: String,
    pub(crate) attempt: u32,
    pub(crate) session_id: String,
    pub(crate) event_type: String,
    pub(crate) payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AutomationAttemptReceiptSingleAppendSummary {
    pub(crate) path: PathBuf,
    pub(crate) seq: u64,
    pub(crate) record_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct AutomationAttemptReceiptReconcileSummary {
    pub(crate) found: bool,
    pub(crate) last_seq: u64,
    pub(crate) elapsed_ms: u64,
    pub(crate) attempts: u32,
}

fn sanitize_segment(raw: &str) -> String {
    let value = raw
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if value.is_empty() {
        "unknown".to_string()
    } else {
        value
    }
}

pub(crate) fn automation_attempt_receipts_path(
    receipts_root: &Path,
    run_id: &str,
    node_id: &str,
) -> PathBuf {
    let run = sanitize_segment(run_id);
    let node = sanitize_segment(node_id);
    receipts_root.join(run).join(format!("{node}.jsonl"))
}

pub(crate) fn automation_attempt_receipts_root() -> PathBuf {
    crate::config::paths::resolve_automation_attempt_receipts_dir()
}

pub(crate) fn automation_attempt_receipts_root_for_state_dir(
    state_dir: impl AsRef<Path>,
) -> PathBuf {
    state_dir.as_ref().join("automation_attempt_receipts")
}

pub(crate) fn automation_attempt_receipt_path_for_state_dir(
    state_dir: impl AsRef<Path>,
    run_id: &str,
    node_id: &str,
) -> PathBuf {
    let root = automation_attempt_receipts_root_for_state_dir(state_dir);
    automation_attempt_receipts_path(&root, run_id, node_id)
}

pub(crate) fn automation_attempt_forensic_path(
    workspace_root: &str,
    run_id: &str,
    node_id: &str,
    attempt: u32,
) -> PathBuf {
    Path::new(workspace_root)
        .join(".tandem")
        .join("runs")
        .join(sanitize_segment(run_id))
        .join("attempts")
        .join(sanitize_segment(node_id))
        .join(format!("{attempt}.json"))
}

fn extract_line_seq(line: &str) -> Option<u64> {
    serde_json::from_str::<AutomationAttemptReceiptRecord>(line)
        .ok()
        .map(|record| record.seq)
        .or_else(|| {
            serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|value| value.get("seq").and_then(Value::as_u64))
        })
}

async fn read_last_seq(path: &Path) -> u64 {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(value) => value,
        Err(_) => return 0,
    };
    content
        .lines()
        .filter_map(extract_line_seq)
        .max()
        .unwrap_or(0)
}

async fn receipt_ledger_lock_for(path: &Path) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<
        tokio::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    > = OnceLock::new();
    let map = LOCKS.get_or_init(|| tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = map.lock().await;
    guard
        .entry(path.to_string_lossy().to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

pub(crate) async fn read_automation_attempt_receipt_records(
    path: &Path,
) -> anyhow::Result<Vec<AutomationAttemptReceiptRecord>> {
    let content = tokio::fs::read_to_string(path).await?;
    Ok(content
        .lines()
        .filter_map(|line| serde_json::from_str::<AutomationAttemptReceiptRecord>(line).ok())
        .collect())
}

pub(crate) async fn append_automation_attempt_receipts(
    receipts_root: &Path,
    run_id: &str,
    node_id: &str,
    attempt: u32,
    session_id: &str,
    events: &[AutomationAttemptReceiptEventInput],
) -> anyhow::Result<AutomationAttemptReceiptSingleAppendSummary> {
    if events.is_empty() {
        let path = automation_attempt_receipts_path(receipts_root, run_id, node_id);
        return Ok(AutomationAttemptReceiptSingleAppendSummary {
            path: path.clone(),
            seq: read_last_seq(&path).await,
            record_count: 0,
        });
    }
    let path = automation_attempt_receipts_path(receipts_root, run_id, node_id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let receipt_lock = receipt_ledger_lock_for(&path).await;
    let _receipt_guard = receipt_lock.lock().await;

    let mut next_seq = read_last_seq(&path).await.saturating_add(1);
    for event in events {
        let record = AutomationAttemptReceiptRecord {
            version: RECEIPT_SCHEMA_VERSION,
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            attempt,
            session_id: session_id.to_string(),
            seq: next_seq,
            ts_ms: now_ms() as u64,
            event_type: event.event_type.trim().to_string(),
            payload: event.payload.clone(),
        };
        let line = serde_json::to_string(&record)?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        next_seq = next_seq.saturating_add(1);
    }

    Ok(AutomationAttemptReceiptSingleAppendSummary {
        path,
        seq: next_seq.saturating_sub(1),
        record_count: events.len() as u64,
    })
}

pub(crate) async fn append_automation_attempt_receipt(
    state_dir: impl AsRef<Path>,
    draft: AutomationAttemptReceiptDraft,
) -> anyhow::Result<AutomationAttemptReceiptSingleAppendSummary> {
    let root = automation_attempt_receipts_root_for_state_dir(state_dir);
    let summary = append_automation_attempt_receipts(
        &root,
        &draft.run_id,
        &draft.node_id,
        draft.attempt,
        &draft.session_id,
        &[AutomationAttemptReceiptEventInput {
            event_type: draft.event_type,
            payload: draft.payload,
        }],
    )
    .await?;
    let path = summary.path.clone();
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let record_count = content
        .lines()
        .filter(|line| serde_json::from_str::<AutomationAttemptReceiptRecord>(line).is_ok())
        .count() as u64;
    Ok(AutomationAttemptReceiptSingleAppendSummary {
        path,
        seq: summary.seq,
        record_count,
    })
}

pub(crate) async fn persist_automation_attempt_forensic_record(
    workspace_root: &str,
    run_id: &str,
    node_id: &str,
    attempt: u32,
    payload: &Value,
) -> anyhow::Result<PathBuf> {
    let path = automation_attempt_forensic_path(workspace_root, run_id, node_id, attempt);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let serialized = serde_json::to_string_pretty(payload)?;
    tokio::fs::write(&path, serialized).await?;
    Ok(path)
}

pub(crate) async fn reconcile_automation_attempt_receipts(
    path: &Path,
    expected_min_seq: u64,
    max_wait_ms: u64,
    poll_interval_ms: u64,
) -> AutomationAttemptReceiptReconcileSummary {
    let start_ms = now_ms() as u64;
    let mut attempts = 0u32;
    let effective_min_seq = if expected_min_seq == 0 {
        1
    } else {
        expected_min_seq
    };
    let poll_interval_ms = poll_interval_ms.max(1);

    loop {
        attempts = attempts.saturating_add(1);
        let current_seq = read_last_seq(path).await;
        if current_seq >= effective_min_seq {
            let elapsed_ms = now_ms() as u64 - start_ms;
            return AutomationAttemptReceiptReconcileSummary {
                found: true,
                last_seq: current_seq,
                elapsed_ms,
                attempts,
            };
        }
        let elapsed_ms = now_ms() as u64 - start_ms;
        if elapsed_ms >= max_wait_ms {
            return AutomationAttemptReceiptReconcileSummary {
                found: false,
                last_seq: current_seq,
                elapsed_ms,
                attempts,
            };
        }
        let remaining_ms = max_wait_ms.saturating_sub(elapsed_ms);
        let sleep_ms = poll_interval_ms.min(remaining_ms);
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Barrier;

    #[tokio::test]
    async fn append_automation_attempt_receipts_appends_and_increments_seq() {
        let root =
            std::env::temp_dir().join(format!("tandem-attempt-receipts-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.expect("create root");

        let summary = append_automation_attempt_receipts(
            &root,
            "automation-v2-run-123",
            "research_sources",
            1,
            "session-123",
            &[AutomationAttemptReceiptEventInput {
                event_type: "attempt_summary".to_string(),
                payload: json!({"ok": true}),
            }],
        )
        .await
        .expect("append first");
        assert_eq!(summary.record_count, 1);
        assert_eq!(summary.seq, 1);

        let ledger_path =
            automation_attempt_receipts_path(&root, "automation-v2-run-123", "research_sources");
        let mut existing = tokio::fs::read_to_string(&ledger_path)
            .await
            .expect("ledger content");
        // Simulate malformed row to ensure seq derivation tolerates garbage.
        existing.push_str("not-json\n");
        tokio::fs::write(&ledger_path, existing)
            .await
            .expect("write malformed row");

        let summary2 = append_automation_attempt_receipts(
            &root,
            "automation-v2-run-123",
            "research_sources",
            2,
            "session-123",
            &[AutomationAttemptReceiptEventInput {
                event_type: "validation_summary".to_string(),
                payload: json!({"status": "completed"}),
            }],
        )
        .await
        .expect("append second");
        assert_eq!(summary2.record_count, 1);
        assert_eq!(summary2.seq, 2);
    }

    #[tokio::test]
    async fn append_automation_attempt_receipts_serializes_concurrent_appends() {
        let root =
            std::env::temp_dir().join(format!("tandem-attempt-receipts-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.expect("create root");
        let path = automation_attempt_receipts_path(&root, "automation-v2-run-321", "notify_user");
        let barrier = Arc::new(Barrier::new(3));
        let first_events = vec![AutomationAttemptReceiptEventInput {
            event_type: "attempt_summary".to_string(),
            payload: json!({"status": "blocked"}),
        }];
        let second_events = vec![AutomationAttemptReceiptEventInput {
            event_type: "validation_summary".to_string(),
            payload: json!({"status": "completed"}),
        }];

        let first_root = root.clone();
        let first_barrier = barrier.clone();
        let first = tokio::spawn(async move {
            first_barrier.wait().await;
            append_automation_attempt_receipts(
                &first_root,
                "automation-v2-run-321",
                "notify_user",
                1,
                "session-321",
                &first_events,
            )
            .await
        });
        let second_root = root.clone();
        let second_barrier = barrier.clone();
        let second = tokio::spawn(async move {
            second_barrier.wait().await;
            append_automation_attempt_receipts(
                &second_root,
                "automation-v2-run-321",
                "notify_user",
                1,
                "session-321",
                &second_events,
            )
            .await
        });

        barrier.wait().await;
        let first = first.await.expect("append first").expect("append first");
        let second = second.await.expect("append second").expect("append second");

        assert_eq!(first.record_count, 1);
        assert_eq!(second.record_count, 1);
        let mut records = read_automation_attempt_receipt_records(&path)
            .await
            .expect("read concurrent records");
        assert_eq!(records.len(), 2);
        records.sort_by_key(|record| record.seq);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[1].seq, 2);
        let mut event_types = records
            .iter()
            .map(|record| record.event_type.as_str())
            .collect::<Vec<_>>();
        event_types.sort_unstable();
        assert_eq!(event_types, vec!["attempt_summary", "validation_summary"]);
    }

    #[tokio::test]
    async fn read_automation_attempt_receipt_records_returns_timeline_entries() {
        let root =
            std::env::temp_dir().join(format!("tandem-attempt-receipts-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.expect("create root");

        let path =
            automation_attempt_receipts_path(&root, "automation-v2-run-456", "generate_report");
        let summary = append_automation_attempt_receipts(
            &root,
            "automation-v2-run-456",
            "generate_report",
            3,
            "session-2",
            &[
                AutomationAttemptReceiptEventInput {
                    event_type: "attempt_summary".to_string(),
                    payload: json!({"receipt_kind":"attempt_summary","status":"blocked"}),
                },
                AutomationAttemptReceiptEventInput {
                    event_type: "validation_summary".to_string(),
                    payload: json!({"receipt_kind":"validation_summary","validator_summary":{"outcome":"blocked"}}),
                },
            ],
        )
        .await
        .expect("seed receipts");
        assert_eq!(summary.record_count, 2);

        let records = read_automation_attempt_receipt_records(&path)
            .await
            .expect("read receipt records");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[0].event_type, "attempt_summary");
        assert_eq!(records[0].attempt, 3);
        assert_eq!(records[0].session_id, "session-2");
        assert_eq!(records[1].seq, 2);
        assert_eq!(records[1].event_type, "validation_summary");
    }

    #[test]
    fn automation_attempt_receipts_path_sanitizes_segments() {
        let root = PathBuf::from("/tmp/receipts");
        let path = automation_attempt_receipts_path(&root, "automation/v2/run", "research:sources");
        assert!(
            path.ends_with(PathBuf::from("automation-v2-run/research-sources.jsonl")),
            "unexpected path: {}",
            path.display()
        );
    }

    #[tokio::test]
    async fn persist_automation_attempt_forensic_record_writes_attempt_json() {
        let workspace_root =
            std::env::temp_dir().join(format!("tandem-attempt-forensics-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&workspace_root)
            .await
            .expect("create workspace root");
        let payload = json!({
            "attempt": 2,
            "final_backend_actionability_state": "needs_repair",
            "blocker_category": "provider_transport_failure"
        });

        let path = persist_automation_attempt_forensic_record(
            workspace_root.to_str().expect("workspace root"),
            "run-123",
            "research:sources",
            2,
            &payload,
        )
        .await
        .expect("persist forensic record");

        assert!(path.ends_with(PathBuf::from(
            ".tandem/runs/run-123/attempts/research-sources/2.json"
        )));
        let stored = tokio::fs::read_to_string(&path)
            .await
            .expect("read forensic record");
        let parsed: Value = serde_json::from_str(&stored).expect("parse forensic json");
        assert_eq!(parsed, payload);
    }

    #[tokio::test]
    async fn reconcile_attempt_receipts_waits_for_delayed_append() {
        let root =
            std::env::temp_dir().join(format!("tandem-receipt-reconcile-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.expect("create root");
        let ledger_path =
            automation_attempt_receipts_path(&root, "automation-v2-run-456", "generate_report");

        let append_root = root.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            let _ = append_automation_attempt_receipts(
                &append_root,
                "automation-v2-run-456",
                "generate_report",
                1,
                "session-456",
                &[AutomationAttemptReceiptEventInput {
                    event_type: "attempt_summary".to_string(),
                    payload: json!({"ok": true}),
                }],
            )
            .await;
        });

        let summary = reconcile_automation_attempt_receipts(&ledger_path, 1, 500, 20).await;
        assert!(summary.found);
        assert!(summary.last_seq >= 1);
        assert!(summary.attempts > 0);
    }

    #[tokio::test]
    async fn reconcile_attempt_receipts_times_out_when_missing() {
        let root =
            std::env::temp_dir().join(format!("tandem-receipt-timeout-{}", uuid::Uuid::new_v4()));
        let ledger_path =
            automation_attempt_receipts_path(&root, "automation-v2-run-789", "research_sources");

        let summary = reconcile_automation_attempt_receipts(&ledger_path, 1, 120, 30).await;
        assert!(!summary.found);
        assert_eq!(summary.last_seq, 0);
        assert!(summary.attempts > 0);
    }

    #[tokio::test]
    async fn reconcile_attempt_receipts_ignores_malformed_jsonl() {
        let root =
            std::env::temp_dir().join(format!("tandem-receipt-malformed-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&root).await.expect("create root");
        let ledger_path =
            automation_attempt_receipts_path(&root, "automation-v2-run-999", "notify_user");
        if let Some(parent) = ledger_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .expect("create parent");
        }
        tokio::fs::write(&ledger_path, "not-json\n")
            .await
            .expect("seed malformed");

        let append_root = root.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            let _ = append_automation_attempt_receipts(
                &append_root,
                "automation-v2-run-999",
                "notify_user",
                1,
                "session-999",
                &[AutomationAttemptReceiptEventInput {
                    event_type: "validation_summary".to_string(),
                    payload: json!({"status": "completed"}),
                }],
            )
            .await;
        });

        let summary = reconcile_automation_attempt_receipts(&ledger_path, 1, 500, 25).await;
        assert!(summary.found);
        assert!(summary.last_seq >= 1);
    }
}
