use serde_json::Value;
use std::collections::BTreeSet;
use tandem_plan_compiler::api as compiler_api;
use tandem_types::{Message, MessagePart, MessageRole};

use super::*;
use compiler_api::{
    prepare_build_request, Clock, McpToolCatalog, PlanStore, PlannerBuildConfig,
    PlannerBuildResult, PlannerInvocationFailure, PlannerLlmInvocation, PlannerLlmInvoker,
    PlannerModelRegistry, PlannerSessionStore, TelemetrySink, WorkspaceResolver,
};

pub(crate) struct WorkflowPlannerHost<'a> {
    pub(crate) state: &'a AppState,
}

pub(crate) async fn resolve_workspace_root(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String, String> {
    let root = state.workspace_index.snapshot().await.root;
    let cwd = std::env::current_dir()
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    compiler_api::resolve_workspace_root_candidate(requested, &root, cwd.as_deref())
}

pub(crate) fn normalize_workflow_step_metadata(step: &mut crate::WorkflowPlanStep) {
    compiler_api::normalize_workflow_step_metadata(
        step,
        |step| step.step_id.as_str(),
        |step| step.kind.as_str(),
        |step| step.objective.as_str(),
        |step| {
            step.output_contract
                .as_ref()
                .map(|contract| {
                    compiler_api::output_contract_is_research_brief(
                        &contract.kind,
                        contract.validator.map(|kind| kind.stable_key()),
                    )
                })
                .unwrap_or(false)
        },
        |step| {
            step.output_contract
                .as_ref()
                .map(|contract| contract.enforcement.is_none())
                .unwrap_or(true)
        },
        |step, value| {
            if let Some(contract) = step.output_contract.as_mut() {
                if contract.enforcement.is_none() {
                    if let Ok(enforcement) = serde_json::from_value(value) {
                        contract.enforcement = Some(enforcement);
                    }
                }
            }
        },
        |step| step.metadata.as_ref(),
        |step, value| {
            step.metadata = Some(value);
        },
    );
}

pub(crate) fn normalize_workflow_plan_file_contracts(plan: &mut crate::WorkflowPlan) {
    compiler_api::derive_workflow_step_file_contracts(plan);
}

pub(crate) async fn build_workflow_plan(
    state: &AppState,
    prompt: &str,
    explicit_schedule: Option<&Value>,
    plan_source: &str,
    allowed_mcp_servers: Vec<String>,
    workspace_root: Option<&str>,
    operator_preferences: Option<Value>,
) -> Result<
    PlannerBuildResult<
        crate::routines::types::RoutineMisfirePolicy,
        crate::AutomationFlowInputRef,
        crate::AutomationFlowOutputContract,
    >,
    String,
> {
    let plan_id = format!("wfplan-{}", uuid::Uuid::new_v4());
    let planner_version = "v1".to_string();

    let host = WorkflowPlannerHost { state };
    let request = prepare_build_request(
        plan_id,
        planner_version,
        plan_source.to_string(),
        prompt,
        explicit_schedule,
        "UTC",
        crate::RoutineMisfirePolicy::RunOnce,
        allowed_mcp_servers,
        workspace_root,
        operator_preferences,
    );

    let mut result = compiler_api::build_workflow_plan_with_planner(
        &host,
        request,
        PlannerBuildConfig {
            session_title: "Workflow Planner Create".to_string(),
            timeout_ms: super::workflow_planner_policy::planner_build_timeout_ms(),
            override_env: "TANDEM_WORKFLOW_PLANNER_TEST_BUILD_RESPONSE".to_string(),
        },
        normalize_workflow_step_metadata,
        compiler_api::plan_step_with_dep::<
            crate::AutomationFlowInputRef,
            crate::AutomationFlowOutputContract,
        >(
            "execute_goal",
            "execute",
            "Execute the requested automation goal directly.",
            "worker",
            &[] as &[String],
            Vec::new(),
            Some(compiler_api::default_execute_goal_output_contract_seed().into()),
            None,
        ),
    )
    .await;
    normalize_workflow_plan_file_contracts(&mut result.plan);

    Ok(result)
}

#[async_trait::async_trait]
impl<'a> PlannerModelRegistry for WorkflowPlannerHost<'a> {
    async fn is_provider_configured(&self, provider_id: &str) -> bool {
        self.state
            .providers
            .list()
            .await
            .into_iter()
            .any(|provider| provider.id == provider_id)
    }
}

#[async_trait::async_trait]
impl<'a> PlanStore for WorkflowPlannerHost<'a> {
    async fn get_draft(&self, plan_id: &str) -> Result<Option<Value>, String> {
        self.state
            .get_workflow_plan_draft(plan_id)
            .await
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| truncate_text(&error.to_string(), 500))
    }

    async fn put_draft(&self, _plan_id: &str, draft: Value) -> Result<(), String> {
        let draft: crate::WorkflowPlanDraftRecord = serde_json::from_value(draft)
            .map_err(|error| truncate_text(&error.to_string(), 500))?;
        self.state.put_workflow_plan_draft(draft).await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl<'a> McpToolCatalog for WorkflowPlannerHost<'a> {
    async fn capability_summary(&self, allowed_mcp_servers: &[String]) -> Value {
        build_planner_capability_summary(self.state, allowed_mcp_servers).await
    }
}

#[async_trait::async_trait]
impl<'a> PlannerLlmInvoker for WorkflowPlannerHost<'a> {
    async fn invoke_planner_llm(
        &self,
        invocation: PlannerLlmInvocation,
    ) -> Result<Value, PlannerInvocationFailure> {
        invoke_planner_llm(
            self.state,
            &invocation.session_title,
            &invocation.workspace_root,
            invocation.model,
            invocation.prompt,
            invocation.run_key,
            invocation.timeout_ms,
            &invocation.override_env,
        )
        .await
    }
}

#[async_trait::async_trait]
impl<'a> WorkspaceResolver for WorkflowPlannerHost<'a> {
    async fn resolve_workspace_root(&self, requested: Option<&str>) -> Result<String, String> {
        resolve_workspace_root(self.state, requested).await
    }
}

impl<'a> TelemetrySink for WorkflowPlannerHost<'a> {
    fn warn(&self, message: &str) {
        tracing::warn!("{message}");
    }
}

impl<'a> Clock for WorkflowPlannerHost<'a> {
    fn now_ms(&self) -> u64 {
        crate::now_ms()
    }
}

#[async_trait::async_trait]
impl<'a> PlannerSessionStore for WorkflowPlannerHost<'a> {
    async fn create_planner_session(
        &self,
        title: &str,
        workspace_root: &str,
    ) -> Result<String, String> {
        let mut session = Session::new(Some(title.to_string()), Some(workspace_root.to_string()));
        let session_id = session.id.clone();
        session.workspace_root = Some(workspace_root.to_string());
        self.state
            .storage
            .save_session(session)
            .await
            .map_err(|error| truncate_text(&error.to_string(), 500))?;
        Ok(session_id)
    }

    async fn append_planner_user_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Result<(), String> {
        self.state
            .storage
            .append_message(
                session_id,
                Message::new(
                    MessageRole::User,
                    vec![MessagePart::Text {
                        text: prompt.to_string(),
                    }],
                ),
            )
            .await
            .map_err(|error| truncate_text(&error.to_string(), 500))
    }

    async fn append_planner_assistant_response(
        &self,
        session_id: &str,
        response: &str,
    ) -> Result<(), String> {
        self.state
            .storage
            .append_message(
                session_id,
                Message::new(
                    MessageRole::Assistant,
                    vec![MessagePart::Text {
                        text: response.to_string(),
                    }],
                ),
            )
            .await
            .map_err(|error| truncate_text(&error.to_string(), 500))
    }
}

async fn invoke_planner_llm(
    state: &AppState,
    session_title: &str,
    workspace_root: &str,
    model: tandem_types::ModelSpec,
    prompt: String,
    _run_key: String,
    timeout_ms: u64,
    override_env: &str,
) -> Result<Value, PlannerInvocationFailure> {
    if let Some(payload) =
        super::workflow_planner_policy::planner_test_override_payload(override_env, true)
    {
        return Ok(payload);
    }
    let host = WorkflowPlannerHost { state };
    let original_prompt = prompt.clone();
    let workspace_root = resolve_workspace_root(state, Some(workspace_root))
        .await
        .map_err(|error| PlannerInvocationFailure {
            reason: "invalid_workspace_root".to_string(),
            detail: Some(error),
        })?;
    let session_id =
        compiler_api::begin_planner_session(&host, session_title, &workspace_root, &prompt).await?;
    let output = super::workflow_planner_transport::invoke_planner_provider(
        state,
        &session_id,
        &model,
        prompt,
        timeout_ms,
    )
    .await?;

    if output.trim().is_empty() {
        return Err(PlannerInvocationFailure {
            reason: "empty_output".to_string(),
            detail: Some("Workflow planner completed without assistant text.".to_string()),
        });
    }
    if let Some(value) = extract_planner_json_value(&output) {
        compiler_api::finish_planner_session(&host, &session_id, &output).await?;
        return Ok(value);
    }

    tracing::warn!(
        "workflow planner returned non-JSON text; requesting a JSON-only repair response"
    );
    compiler_api::finish_planner_session(&host, &session_id, &output).await?;
    let repair_prompt = build_planner_json_repair_prompt(session_title, &original_prompt, &output);
    compiler_api::PlannerSessionStore::append_planner_user_prompt(
        &host,
        &session_id,
        &repair_prompt,
    )
    .await
    .map_err(|error| PlannerInvocationFailure {
        reason: "storage_error".to_string(),
        detail: Some(truncate_text(&error, 500)),
    })?;
    let repair_output = super::workflow_planner_transport::invoke_planner_provider(
        state,
        &session_id,
        &model,
        repair_prompt,
        timeout_ms,
    )
    .await?;
    compiler_api::finish_planner_session(&host, &session_id, &repair_output).await?;
    extract_planner_json_value(&repair_output).ok_or_else(|| PlannerInvocationFailure {
        reason: "invalid_json".to_string(),
        detail: Some(
            "Workflow planner returned text without valid JSON, including after a repair retry."
                .to_string(),
        ),
    })
}

async fn build_planner_capability_summary(
    state: &AppState,
    allowed_mcp_servers: &[String],
) -> Value {
    if allowed_mcp_servers.is_empty() {
        let mut summary = compiler_api::build_planner_capability_summary(&[]);
        if let Some(object) = summary.as_object_mut() {
            object.insert(
                "mcp_inventory".to_string(),
                json!({
                    "connected_server_names": [],
                    "enabled_server_names": [],
                    "inventory_version": 1,
                    "registered_tools": [],
                    "remote_tools": [],
                    "servers": [],
                }),
            );
            object.insert(
                "mcp_inventory_source".to_string(),
                Value::String("allowlist_empty".to_string()),
            );
        }
        return summary;
    }

    let (mcp_inventory, mcp_inventory_source) = planner_mcp_inventory_snapshot(state).await;
    let mut server_tools = Vec::new();
    for server in allowed_mcp_servers {
        let tools = state.mcp.server_tools(server).await;
        let mut tool_names = Vec::new();
        for tool in tools.iter() {
            let tool_name = tool.namespaced_name.trim().to_string();
            if !tool_name.is_empty() {
                tool_names.push(tool_name);
            }
        }
        server_tools.push(compiler_api::PlannerMcpServerToolSet {
            server: server.to_string(),
            tool_names,
        });
    }
    let filtered_inventory = filter_mcp_inventory_to_allowed(mcp_inventory, allowed_mcp_servers);
    let mut summary = compiler_api::build_planner_capability_summary(&server_tools);
    if let Some(object) = summary.as_object_mut() {
        object.insert("mcp_inventory".to_string(), filtered_inventory);
        object.insert(
            "mcp_inventory_source".to_string(),
            Value::String(mcp_inventory_source.to_string()),
        );
    }
    summary
}

async fn planner_mcp_inventory_snapshot(state: &AppState) -> (Value, &'static str) {
    match state.tools.execute("mcp_list", json!({})).await {
        Ok(result) => {
            if let Some(metadata) = result.metadata.as_object().cloned().map(Value::Object) {
                return (metadata, "mcp_list");
            }
            if let Ok(parsed) = serde_json::from_str::<Value>(result.output.trim()) {
                return (parsed, "mcp_list");
            }
            tracing::warn!(
                "mcp_list tool returned an unparseable inventory; falling back to direct snapshot"
            );
        }
        Err(error) => {
            tracing::warn!(error = %error, "planner preflight mcp_list invocation failed");
        }
    }
    (
        crate::http::mcp::mcp_inventory_snapshot(state).await,
        "runtime_snapshot",
    )
}

fn build_planner_json_repair_prompt(
    session_title: &str,
    original_prompt: &str,
    invalid_output: &str,
) -> String {
    let prompt = original_prompt.trim();
    let output = invalid_output.trim();
    format!(
        concat!(
            "You are revising a Tandem automation workflow plan.\n",
            "Planner intelligence lives in the model. Return JSON only.\n",
            "The previous planner response was not valid JSON.\n",
            "Return one valid JSON object that matches the planner contract exactly.\n",
            "Do not add markdown fences, prose, explanations, or commentary.\n",
            "Session title: {}\n",
            "Original prompt:\n{}\n\n",
            "Invalid planner response to repair:\n{}\n"
        ),
        session_title.trim(),
        prompt,
        output
    )
}

fn extract_planner_json_value(text: &str) -> Option<Value> {
    compiler_api::extract_json_value_from_text(text)
        .map(normalize_planner_payload_aliases)
        .or_else(|| salvage_planner_payload_from_text(text))
}

fn normalize_planner_payload_aliases(mut payload: Value) -> Value {
    if let Some(object) = payload.as_object_mut() {
        if !object.contains_key("plan") {
            if let Some(workflow_plan) = object.remove("workflow_plan") {
                object.insert("plan".to_string(), workflow_plan);
            }
        }
    }
    payload
}

fn salvage_planner_payload_from_text(text: &str) -> Option<Value> {
    let action = parse_json_string_field(text, "action")?;
    let mut payload = serde_json::Map::new();
    payload.insert("action".to_string(), Value::String(action.clone()));

    if let Some(plan) = parse_json_value_field(text, "plan")
        .or_else(|| parse_json_value_field(text, "workflow_plan"))
    {
        payload.insert("plan".to_string(), plan);
    }
    if let Some(assistant_text) = parse_json_string_field(text, "assistant_text") {
        if !assistant_text.trim().is_empty() {
            payload.insert("assistant_text".to_string(), Value::String(assistant_text));
        }
    }
    if let Some(change_summary) = parse_json_value_field(text, "change_summary") {
        payload.insert("change_summary".to_string(), change_summary);
    }
    if let Some(clarifier) = parse_json_value_field(text, "clarifier") {
        payload.insert("clarifier".to_string(), clarifier);
    }

    match action.as_str() {
        "build" | "revise" => {
            if !payload.contains_key("plan") {
                return None;
            }
        }
        "clarify" => {
            if !payload.contains_key("clarifier") {
                return None;
            }
        }
        "keep" => {}
        _ => return None,
    }

    Some(Value::Object(payload))
}

fn parse_json_value_field(text: &str, key: &str) -> Option<Value> {
    let start = find_json_field_value_start(text, key)?;
    let first = text[start..].chars().next()?;
    if first != '{' && first != '[' {
        return None;
    }
    let fragment = extract_balanced_json_fragment_at(text, start)?;
    serde_json::from_str::<Value>(fragment).ok()
}

fn parse_json_string_field(text: &str, key: &str) -> Option<String> {
    let start = find_json_field_value_start(text, key)?;
    let rest = &text[start..];
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escape = false;
    for ch in chars {
        if escape {
            out.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => return Some(out),
            _ => out.push(ch),
        }
    }
    None
}

fn find_json_field_value_start(text: &str, key: &str) -> Option<usize> {
    let needle = format!("\"{key}\"");
    let mut search_from = 0usize;
    while let Some(relative) = text[search_from..].find(&needle) {
        let key_start = search_from + relative;
        let mut idx = key_start + needle.len();
        while let Some(ch) = text[idx..].chars().next() {
            if ch.is_whitespace() {
                idx += ch.len_utf8();
                continue;
            }
            if ch == ':' {
                idx += ch.len_utf8();
                break;
            }
            idx = key_start + 1;
            break;
        }
        if idx <= key_start + needle.len() {
            search_from = key_start + 1;
            continue;
        }
        while let Some(ch) = text[idx..].chars().next() {
            if ch.is_whitespace() {
                idx += ch.len_utf8();
            } else {
                break;
            }
        }
        return Some(idx);
    }
    None
}

fn extract_balanced_json_fragment_at(text: &str, start: usize) -> Option<&str> {
    let slice = text.get(start..)?;
    let mut iter = slice.char_indices();
    let (_, first) = iter.next()?;
    if first != '{' && first != '[' {
        return None;
    }

    let mut stack = vec![first];
    let mut in_string = false;
    let mut escape = false;
    for (offset, ch) in iter {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' | '[' => stack.push(ch),
            '}' | ']' => {
                let open = stack.pop()?;
                if !matches!((open, ch), ('{', '}') | ('[', ']')) {
                    return None;
                }
                if stack.is_empty() {
                    let end = start + offset + ch.len_utf8();
                    return text.get(start..end);
                }
            }
            _ => {}
        }
    }
    None
}

fn filter_mcp_inventory_to_allowed(inventory: Value, allowed_mcp_servers: &[String]) -> Value {
    let allowed = allowed_mcp_servers
        .iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>();
    let filtered_servers = inventory
        .get("servers")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|row| {
                    row.get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .map(|name| allowed.contains(name))
                        .unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut connected_server_names = Vec::new();
    let mut enabled_server_names = Vec::new();
    let mut registered_tools = BTreeSet::new();
    let mut remote_tools = BTreeSet::new();
    for server in filtered_servers.iter() {
        let Some(name) = server.get("name").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if server
            .get("connected")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            connected_server_names.push(name.to_string());
        }
        if server
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            enabled_server_names.push(name.to_string());
        }
        for tool_name in server
            .get("registered_tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            registered_tools.insert(tool_name.to_string());
        }
        for tool_name in server
            .get("remote_tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            remote_tools.insert(tool_name.to_string());
        }
    }

    json!({
        "connected_server_names": connected_server_names,
        "enabled_server_names": enabled_server_names,
        "inventory_version": inventory.get("inventory_version").and_then(Value::as_u64).unwrap_or(1),
        "registered_tools": registered_tools.into_iter().collect::<Vec<_>>(),
        "remote_tools": remote_tools.into_iter().collect::<Vec<_>>(),
        "servers": filtered_servers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn planner_capability_summary_skips_inventory_when_allowlist_is_empty() {
        let state = crate::http::tests::test_state().await;
        let summary = build_planner_capability_summary(&state, &[]).await;
        let inventory = summary
            .get("mcp_inventory")
            .and_then(Value::as_object)
            .expect("mcp inventory");
        assert_eq!(
            summary.get("mcp_inventory_source").and_then(Value::as_str),
            Some("allowlist_empty")
        );
        let servers = inventory
            .get("servers")
            .and_then(Value::as_array)
            .expect("servers");
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn planner_capability_summary_filters_inventory_to_allowlist() {
        let state = crate::http::tests::test_state().await;
        let summary = build_planner_capability_summary(&state, &["github".to_string()]).await;
        let inventory = summary
            .get("mcp_inventory")
            .and_then(Value::as_object)
            .expect("mcp inventory");
        assert_eq!(
            summary.get("mcp_inventory_source").and_then(Value::as_str),
            Some("mcp_list")
        );
        let servers = inventory
            .get("servers")
            .and_then(Value::as_array)
            .expect("servers");
        assert_eq!(servers.len(), 1);
        assert_eq!(
            servers[0].get("name").and_then(Value::as_str),
            Some("github")
        );
    }

    #[test]
    fn extract_planner_json_value_salvages_workflow_plan_alias() {
        let text = r#"{"action":"build","assistant_text":"ok","workflow_plan":{"title":"Demo","steps":[]}}"#;
        let value = extract_planner_json_value(text).expect("value");
        assert_eq!(value.get("action").and_then(Value::as_str), Some("build"));
        assert!(value.get("plan").is_some());
    }

    #[test]
    fn extract_planner_json_value_salvages_malformed_assistant_text() {
        let text = r#"{"action":"build","assistant_text":"{"plan":{"title":"Demo","steps":[]}}","plan":{"title":"Demo","steps":[]}}"#;
        let value = extract_planner_json_value(text).expect("value");
        assert_eq!(value.get("action").and_then(Value::as_str), Some("build"));
        assert_eq!(
            value
                .get("plan")
                .and_then(|plan| plan.get("title"))
                .and_then(Value::as_str),
            Some("Demo")
        );
    }

    #[test]
    fn planner_json_repair_prompt_requests_json_only() {
        let prompt = build_planner_json_repair_prompt(
            "Workflow Planner Create",
            "Build a workflow",
            "Here is the plan: research, analyze, report.",
        );
        assert!(prompt.contains("Return JSON only"));
        assert!(prompt.contains("previous planner response was not valid JSON"));
        assert!(prompt.contains("Invalid planner response to repair"));
        assert!(prompt.contains("Build a workflow"));
    }
}
