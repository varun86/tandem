use serde_json::Value;

pub(super) fn planner_test_override_payload(
    primary_env: &str,
    include_legacy: bool,
) -> Option<Value> {
    let raw = std::env::var(primary_env).ok().or_else(|| {
        include_legacy
            .then(|| std::env::var("TANDEM_WORKFLOW_PLANNER_TEST_RESPONSE").ok())
            .flatten()
    })?;
    if raw.trim().is_empty() {
        return None;
    }
    tandem_plan_compiler::api::extract_json_value_from_text(&raw)
}

pub(super) fn planner_build_timeout_ms() -> u64 {
    std::env::var("TANDEM_WORKFLOW_PLANNER_BUILD_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| value.clamp(250, 300_000))
        .unwrap_or(90_000)
}

pub(super) fn planner_revision_timeout_ms() -> u64 {
    std::env::var("TANDEM_WORKFLOW_PLANNER_REVISION_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| value.clamp(250, 300_000))
        .unwrap_or(60_000)
}

pub(super) fn classify_planner_provider_failure_reason(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("array too long") || lower.contains("maximum length 128") {
        "tool_schema_too_large"
    } else if lower.contains("user not found")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
        || lower.contains("invalid api key")
        || lower.contains("403")
        || lower.contains("401")
    {
        "provider_auth_failed"
    } else if lower.contains("invalid function name")
        || lower.contains("function_declarations")
        || lower.contains("tools[0]")
    {
        "provider_tool_schema_invalid"
    } else {
        "provider_request_failed"
    }
}
