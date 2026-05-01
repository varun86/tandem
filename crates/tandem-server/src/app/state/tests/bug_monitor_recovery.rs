use crate::{
    bug_monitor::service::recover_overdue_bug_monitor_triage_runs, BugMonitorConfig,
    BugMonitorDraftRecord, BugMonitorIncidentRecord,
};

use super::{test_state_with_path, tmp_resource_file};

fn bug_monitor_recovery_state(name: &str) -> crate::app::state::AppState {
    let mut state = test_state_with_path(tmp_resource_file(name));
    state.bug_monitor_config_path = tmp_resource_file(&format!("{name}-config"));
    state.bug_monitor_drafts_path = tmp_resource_file(&format!("{name}-drafts"));
    state.bug_monitor_incidents_path = tmp_resource_file(&format!("{name}-incidents"));
    state.bug_monitor_posts_path = tmp_resource_file(&format!("{name}-posts"));
    state.automation_v2_runs_path = tmp_resource_file(&format!("{name}-runs"));
    state
}

fn timed_out_draft(draft_id: &str, triage_run_id: &str) -> BugMonitorDraftRecord {
    BugMonitorDraftRecord {
        draft_id: draft_id.to_string(),
        fingerprint: "fingerprint-recovery".to_string(),
        repo: "frumu-ai/tandem".to_string(),
        status: "draft_ready".to_string(),
        created_at_ms: 1,
        triage_run_id: Some(triage_run_id.to_string()),
        title: Some("Failure detected in automation_v2.run.failed".to_string()),
        detail: Some("original workflow failure detail".to_string()),
        github_status: Some("triage_timed_out".to_string()),
        last_post_error: Some("triage run timed out before publishing".to_string()),
        ..Default::default()
    }
}

fn incident_for_draft(
    incident_id: &str,
    draft_id: &str,
    triage_run_id: &str,
) -> BugMonitorIncidentRecord {
    BugMonitorIncidentRecord {
        incident_id: incident_id.to_string(),
        fingerprint: "fingerprint-recovery".to_string(),
        event_type: "automation_v2.run.failed".to_string(),
        status: "triage_timed_out".to_string(),
        repo: "frumu-ai/tandem".to_string(),
        workspace_root: "/tmp/tandem".to_string(),
        title: "Failure detected in automation_v2.run.failed".to_string(),
        occurrence_count: 1,
        created_at_ms: 1,
        updated_at_ms: 1,
        draft_id: Some(draft_id.to_string()),
        triage_run_id: Some(triage_run_id.to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn overdue_recovery_retries_unposted_timed_out_triage_drafts() {
    let state = bug_monitor_recovery_state("bug-monitor-retry-timed-out-draft");
    state
        .put_bug_monitor_config(BugMonitorConfig {
            enabled: true,
            paused: false,
            repo: Some("frumu-ai/tandem".to_string()),
            triage_timeout_ms: Some(0),
            ..Default::default()
        })
        .await
        .expect("put bug monitor config");

    let draft_id = "failure-draft-retry-timed-out";
    let triage_run_id = "automation-v2-run-retry-timed-out";
    let incident_id = "failure-incident-retry-timed-out";
    state
        .put_bug_monitor_draft(timed_out_draft(draft_id, triage_run_id))
        .await
        .expect("put timed out draft");
    state
        .put_bug_monitor_incident(incident_for_draft(incident_id, draft_id, triage_run_id))
        .await
        .expect("put incident");

    let recovered = recover_overdue_bug_monitor_triage_runs(&state)
        .await
        .expect("recover overdue triage");

    assert_eq!(
        recovered,
        vec![(draft_id.to_string(), Some(incident_id.to_string()))]
    );
}

#[tokio::test]
async fn overdue_recovery_skips_timed_out_triage_drafts_with_github_issue() {
    let state = bug_monitor_recovery_state("bug-monitor-skip-posted-timed-out-draft");
    state
        .put_bug_monitor_config(BugMonitorConfig {
            enabled: true,
            paused: false,
            repo: Some("frumu-ai/tandem".to_string()),
            triage_timeout_ms: Some(0),
            ..Default::default()
        })
        .await
        .expect("put bug monitor config");

    let mut draft = timed_out_draft(
        "failure-draft-posted-timed-out",
        "automation-v2-run-posted-timed-out",
    );
    draft.issue_number = Some(68);
    draft.github_issue_url = Some("https://github.com/frumu-ai/tandem/issues/68".to_string());
    state
        .put_bug_monitor_draft(draft)
        .await
        .expect("put posted timed out draft");

    let recovered = recover_overdue_bug_monitor_triage_runs(&state)
        .await
        .expect("recover overdue triage");

    assert!(recovered.is_empty());
}
