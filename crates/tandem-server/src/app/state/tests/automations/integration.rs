use super::*;
use async_trait::async_trait;
use futures::{stream, Stream};
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use tandem_providers::{ChatMessage, Provider, StreamChunk, TokenUsage};
use tandem_tools::Tool;
use tandem_types::{
    Message, MessagePart, MessageRole, ModelInfo, ProviderInfo, Session, ToolMode, ToolResult,
    ToolSchema,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
struct PromptRecord {
    prompt: String,
    tool_names: Vec<String>,
    tool_mode: String,
    model_override: Option<String>,
}

#[derive(Clone)]
struct ScriptedProvider {
    records: Arc<Mutex<Vec<PromptRecord>>>,
    scripts: Arc<Mutex<VecDeque<Vec<StreamChunk>>>>,
}

impl ScriptedProvider {
    fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(Vec::new())),
            scripts: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    async fn push_script(&self, script: Vec<StreamChunk>) {
        self.scripts.lock().await.push_back(script);
    }

    async fn records(&self) -> Vec<PromptRecord> {
        self.records.lock().await.clone()
    }
}

#[async_trait]
impl Provider for ScriptedProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "scripted".to_string(),
            name: "Scripted".to_string(),
            models: vec![ModelInfo {
                id: "scripted-model".to_string(),
                provider_id: "scripted".to_string(),
                display_name: "Scripted Model".to_string(),
                context_window: 8192,
            }],
        }
    }

    async fn complete(
        &self,
        _prompt: &str,
        _model_override: Option<&str>,
    ) -> anyhow::Result<String> {
        anyhow::bail!("scripted provider only supports streaming");
    }

    async fn stream(
        &self,
        messages: Vec<ChatMessage>,
        model_override: Option<&str>,
        tool_mode: ToolMode,
        tools: Option<Vec<ToolSchema>>,
        _cancel: CancellationToken,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>> {
        let prompt = messages
            .iter()
            .map(|message| format!("{}: {}", message.role, message.content))
            .collect::<Vec<_>>()
            .join("\n");
        let mut tool_names = tools
            .unwrap_or_default()
            .into_iter()
            .map(|schema| schema.name)
            .collect::<Vec<_>>();
        tool_names.sort();
        tool_names.dedup();
        self.records.lock().await.push(PromptRecord {
            prompt,
            tool_names,
            tool_mode: format!("{tool_mode:?}"),
            model_override: model_override.map(str::to_string),
        });

        let script = self
            .scripts
            .lock()
            .await
            .pop_front()
            .expect("scripted provider exhausted");

        Ok(Box::pin(stream::iter(script.into_iter().map(Ok))))
    }
}

#[derive(Clone)]
struct RecordingTool {
    schema: ToolSchema,
    output: String,
    metadata: serde_json::Value,
    calls: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl RecordingTool {
    fn new(
        name: &str,
        description: &str,
        input_schema: serde_json::Value,
        output: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            schema: ToolSchema::new(name, description, input_schema),
            output: output.into(),
            metadata,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn calls(&self) -> Vec<serde_json::Value> {
        self.calls.lock().await.clone()
    }
}

#[async_trait]
impl Tool for RecordingTool {
    fn schema(&self) -> ToolSchema {
        self.schema.clone()
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        self.calls.lock().await.push(args);
        Ok(ToolResult {
            output: self.output.clone(),
            metadata: self.metadata.clone(),
        })
    }
}

fn tool_turn(calls: Vec<(&str, &str, serde_json::Value)>) -> Vec<StreamChunk> {
    let mut chunks = Vec::new();
    for (index, (id, name, args)) in calls.into_iter().enumerate() {
        let call_id = if id.is_empty() {
            format!("call_{}", index + 1)
        } else {
            id.to_string()
        };
        chunks.push(StreamChunk::ToolCallStart {
            id: call_id.clone(),
            name: name.to_string(),
        });
        chunks.push(StreamChunk::ToolCallDelta {
            id: call_id.clone(),
            args_delta: args.to_string(),
        });
        chunks.push(StreamChunk::ToolCallEnd { id: call_id });
    }
    chunks.push(StreamChunk::Done {
        finish_reason: "tool_calls".to_string(),
        usage: None,
    });
    chunks
}

fn json_tool_turn(tool: &str, args: serde_json::Value) -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta(
            serde_json::to_string(&json!({
                "tool": tool,
                "args": args
            }))
            .expect("tool call json"),
        ),
        StreamChunk::Done {
            finish_reason: "tool_calls".to_string(),
            usage: None,
        },
    ]
}

fn final_turn(text: &str) -> Vec<StreamChunk> {
    vec![
        StreamChunk::TextDelta(text.to_string()),
        StreamChunk::Done {
            finish_reason: "stop".to_string(),
            usage: Some(TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
            }),
        },
    ]
}

fn brief_research_node(
    node_id: &str,
    output_path: &str,
    web_research_expected: bool,
) -> AutomationFlowNode {
    AutomationFlowNode {
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        node_id: node_id.to_string(),
        agent_id: "researcher".to_string(),
        objective: "Write a research brief grounded in the workspace".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "brief".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        max_tool_calls: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": output_path,
                "web_research_expected": web_research_expected,
                "source_coverage_required": true
            }
        })),
    }
}

fn citations_research_node(node_id: &str, output_path: &str) -> AutomationFlowNode {
    AutomationFlowNode {
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        node_id: node_id.to_string(),
        agent_id: "researcher".to_string(),
        objective: "Write a grounded citation handoff".to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "citations".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ResearchBrief),
            enforcement: None,
            schema: None,
            summary_guidance: Some("Return a citation handoff.".to_string()),
        }),
        retry_policy: None,
        timeout_ms: None,
        max_tool_calls: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": output_path,
                "source_coverage_required": true,
                "preferred_mcp_servers": ["tandem-mcp"]
            }
        })),
    }
}

fn analyze_findings_node(
    node_id: &str,
    output_path: &str,
    workspace_file: &str,
) -> AutomationFlowNode {
    AutomationFlowNode {
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        node_id: node_id.to_string(),
        agent_id: "analyst".to_string(),
        objective:
            "Synthesize the clustered findings into structured JSON and update the durable analysis file."
                .to_string(),
        depends_on: vec!["cluster_topics".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "cluster_topics".to_string(),
            alias: "clusters".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "structured_json".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::StructuredJson),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        max_tool_calls: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": output_path,
                "output_files": [workspace_file]
            }
        })),
    }
}

fn compare_results_node(node_id: &str, output_path: &str) -> AutomationFlowNode {
    AutomationFlowNode {
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        node_id: node_id.to_string(),
        agent_id: "editor".to_string(),
        objective: "Review existing persistent blog memory and recent Tandem blog history to produce a recent blog review.".to_string(),
        depends_on: vec!["collect_inputs".to_string(), "research_sources".to_string()],
        input_refs: vec![
            AutomationFlowInputRef {
                from_step_id: "collect_inputs".to_string(),
                alias: "run_context".to_string(),
            },
            AutomationFlowInputRef {
                from_step_id: "research_sources".to_string(),
                alias: "tandem_grounding".to_string(),
            },
        ],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::GenericArtifact),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        max_tool_calls: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "output_path": output_path,
                "preferred_mcp_servers": ["blog-mcp"]
            }
        })),
    }
}

fn delivery_node(node_id: &str, recipient: &str) -> AutomationFlowNode {
    AutomationFlowNode {
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        node_id: node_id.to_string(),
        agent_id: "operator".to_string(),
        objective: format!(
            "Send the finalized report to {} using the validated artifact body as the delivery source of truth.",
            recipient
        ),
        depends_on: vec!["generate_report".to_string()],
        input_refs: vec![AutomationFlowInputRef {
            from_step_id: "generate_report".to_string(),
            alias: "final_report".to_string(),
        }],
        output_contract: Some(AutomationFlowOutputContract {
            kind: "approval_gate".to_string(),
            validator: Some(crate::AutomationOutputValidatorKind::ReviewDecision),
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: None,
        timeout_ms: None,
        max_tool_calls: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "delivery": {
                "method": "email",
                "to": recipient,
                "content_type": "text/html",
                "inline_body_only": true,
                "attachments": false
            }
        })),
    }
}

fn code_loop_node(node_id: &str, output_path: &str) -> AutomationFlowNode {
    AutomationFlowNode {
        knowledge: tandem_orchestrator::KnowledgeBinding::default(),
        node_id: node_id.to_string(),
        agent_id: "engineer".to_string(),
        objective:
            "Inspect the code, patch the smallest root cause, rerun verification, and write a concise implementation handoff."
                .to_string(),
        depends_on: Vec::new(),
        input_refs: Vec::new(),
        output_contract: Some(AutomationFlowOutputContract {
            kind: "report_markdown".to_string(),
            validator: None,
            enforcement: None,
            schema: None,
            summary_guidance: None,
        }),
        retry_policy: Some(json!({
            "max_attempts": 2
        })),
        timeout_ms: None,
        max_tool_calls: None,
        stage_kind: Some(AutomationNodeStageKind::Workstream),
        gate: None,
        metadata: Some(json!({
            "builder": {
                "task_kind": "code_change",
                "verification_command": "cargo test",
                "output_path": output_path
            }
        })),
    }
}

fn automation_with_single_node(
    automation_id: &str,
    node: AutomationFlowNode,
    workspace_root: &std::path::Path,
    allowlist: Vec<String>,
) -> AutomationV2Spec {
    let mut automation = AutomationSpecBuilder::new(automation_id)
        .name(format!("{automation_id} test"))
        .nodes(vec![node])
        .workspace_root(workspace_root.to_string_lossy().to_string())
        .build();
    let agent = automation.agents.first_mut().expect("test agent");
    agent.agent_id = "researcher".to_string();
    agent.template_id = None;
    agent.display_name = "Researcher".to_string();
    agent.tool_policy.allowlist = allowlist;
    agent.tool_policy.denylist.clear();
    agent.mcp_policy.allowed_servers = Vec::new();
    agent.mcp_policy.allowed_tools = None;
    automation
}

async fn install_provider_and_tools(
    state: &AppState,
    provider: &ScriptedProvider,
    tools: Vec<(&str, Arc<RecordingTool>)>,
) {
    state
        .providers
        .replace_for_test(
            vec![Arc::new(provider.clone())],
            Some("scripted".to_string()),
        )
        .await;
    for (name, tool) in tools {
        state.tools.register_tool(name.to_string(), tool).await;
    }
}

fn prompt_contains_only_run_scoped_path(record: &PromptRecord, output_path: &str) {
    assert!(
        record.prompt.contains(output_path),
        "prompt did not include the run-scoped output path {output_path:?}"
    );
    assert!(
        !record.prompt.contains(".tandem/artifacts/"),
        "prompt still mentioned the legacy workspace-scoped artifact path"
    );
}

fn assistant_session_with_tool_invocations(
    title: &str,
    workspace_root: &std::path::Path,
    invocations: Vec<(&str, serde_json::Value, serde_json::Value, Option<&str>)>,
) -> Session {
    let mut session = Session::new(
        Some(title.to_string()),
        Some(workspace_root.to_string_lossy().to_string()),
    );
    session.messages.push(Message::new(
        MessageRole::Assistant,
        invocations
            .into_iter()
            .map(|(tool, args, result, error)| MessagePart::ToolInvocation {
                tool: tool.to_string(),
                args,
                result: Some(result),
                error: error.map(str::to_string),
            })
            .collect(),
    ));
    session
}

async fn persist_validated_output(
    state: &AppState,
    run_id: &str,
    node_id: &str,
    output: serde_json::Value,
    status: AutomationRunStatus,
    attempt: u32,
) {
    state
        .update_automation_v2_run(run_id, |row| {
            row.status = status;
            row.checkpoint
                .node_outputs
                .insert(node_id.to_string(), output.clone());
            row.checkpoint
                .node_attempts
                .insert(node_id.to_string(), attempt);
        })
        .await
        .expect("persist validated output");
}

#[tokio::test]
async fn local_research_flow_completes_with_read_and_write_artifact() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-local-research-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("docs")).expect("create workspace");
    std::fs::write(
        workspace_root.join("docs/source.md"),
        "# Source\n\nWorkspace evidence for the local brief.\n",
    )
    .expect("seed source file");

    let state = ready_test_state().await;
    let node = brief_research_node("research_local", ".tandem/artifacts/local-brief.md", false);
    let automation = automation_with_single_node(
        "automation-local-research",
        node.clone(),
        &workspace_root,
        vec!["read".to_string()],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let output_path = automation_node_required_output_path_for_run(&node, Some(&run.run_id))
        .expect("required output path");
    let workspace_snapshot_before = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let artifact_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n## Campaign goal\nClarify positioning.\n\n## Target audience\n- Operators.\n\n## Core pain points\n- Coordination overhead.\n\n## Positioning angle\nTandem centralizes orchestration.\n\n## Competitor context\nLocal-only comparison for this run.\n\n## Proof points with citations\n1. Supported from docs/source.md. Source note: https://example.com/reference\n\n## Likely objections\n- Proof depth.\n\n## Channel considerations\n- Landing page.\n\n## Recommended message hierarchy\n1. Problem\n2. Promise\n\n## Files reviewed\n- docs/source.md\n\n## Files not reviewed\n- docs/extra.md: not needed for this first pass.\n"
        .to_string();

    let artifact_dir = workspace_root
        .join(".tandem/runs")
        .join(&run.run_id)
        .join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::write(artifact_dir.join("local-brief.md"), &artifact_text).expect("write artifact");

    let session = assistant_session_with_tool_invocations(
        "local-research-validation",
        &workspace_root,
        vec![
            (
                "glob",
                json!({"pattern":"docs/**/*.md"}),
                json!({
                    "output": workspace_root
                        .join("docs/source.md")
                        .display()
                        .to_string()
                }),
                None,
            ),
            (
                "read",
                json!({"path":"docs/source.md"}),
                json!({"output":"Workspace evidence for the local brief."}),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":artifact_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let requested_tools = vec!["glob".to_string(), "read".to_string(), "write".to_string()];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    assert_eq!(
        tool_telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["glob", "read", "write"])
    );
    assert_eq!(
        tool_telemetry
            .get("workspace_inspection_used")
            .and_then(Value::as_bool),
        Some(true)
    );

    let session_text = "Done\n\n{\"status\":\"completed\"}";
    let (accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        session_text,
        &tool_telemetry,
        None,
        Some((output_path.clone(), artifact_text.clone())),
        &workspace_snapshot_before,
    );
    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("semantic_block_reason")
            .and_then(Value::as_str),
        None
    );

    let status = detect_automation_node_status(
        &node,
        session_text,
        accepted_output.as_ref(),
        &tool_telemetry,
        Some(&artifact_validation),
    );
    assert_eq!(status.0, "completed");

    let output = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        &session.id,
        Some(&run.run_id),
        session_text,
        accepted_output.clone(),
        Some(artifact_validation.clone()),
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output.clone(),
        AutomationRunStatus::Completed,
        1,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Completed);
    assert_eq!(
        persisted.checkpoint.node_attempts.get("research_local"),
        Some(&1)
    );

    let output = persisted
        .checkpoint
        .node_outputs
        .get("research_local")
        .expect("node output");
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["glob", "read", "write"])
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/workspace_inspection_used")
            .and_then(Value::as_bool),
        Some(true)
    );

    let written = std::fs::read_to_string(
        workspace_root
            .join(".tandem/runs")
            .join(&run.run_id)
            .join("artifacts")
            .join("local-brief.md"),
    )
    .expect("written artifact");
    assert_eq!(written, artifact_text);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn mcp_grounded_research_flow_completes_with_mcp_tool_usage() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-mcp-research-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let state = ready_test_state().await;
    let node = citations_research_node("research_mcp", ".tandem/artifacts/research-sources.json");
    let automation = automation_with_single_node(
        "automation-mcp-research",
        node.clone(),
        &workspace_root,
        vec!["mcp.tandem_mcp.search_docs".to_string()],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let output_path = automation_node_required_output_path_for_run(&node, Some(&run.run_id))
        .expect("required output path");
    let workspace_snapshot_before = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let artifact_text = "# Research Sources\n\n## Summary\nCollected current Tandem MCP documentation references.\n\n## Citations\n1. Tandem MCP Guide. Source note: tandem-mcp://docs/guide\n2. Tandem MCP API Reference. Source note: tandem-mcp://docs/api-reference\n"
        .to_string();

    let artifact_dir = workspace_root
        .join(".tandem/runs")
        .join(&run.run_id)
        .join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::write(artifact_dir.join("research-sources.json"), &artifact_text)
        .expect("write artifact");

    let session = assistant_session_with_tool_invocations(
        "mcp-research-validation",
        &workspace_root,
        vec![
            (
                "mcp.tandem_mcp.search_docs",
                json!({
                    "query": "research sources artifact contract"
                }),
                json!({
                    "output": "Matched Tandem MCP docs",
                    "metadata": {"count": 2}
                }),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":artifact_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let requested_tools = vec![
        "mcp.tandem_mcp.search_docs".to_string(),
        "write".to_string(),
    ];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    assert_eq!(
        tool_telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["mcp.tandem_mcp.search_docs", "write"])
    );
    assert_eq!(
        tool_telemetry
            .get("web_research_used")
            .and_then(Value::as_bool),
        Some(false)
    );

    let session_text = "Done\n\n{\"status\":\"completed\"}";
    let (accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        session_text,
        &tool_telemetry,
        None,
        Some((output_path.clone(), artifact_text.clone())),
        &workspace_snapshot_before,
    );
    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );

    let status = detect_automation_node_status(
        &node,
        session_text,
        accepted_output.as_ref(),
        &tool_telemetry,
        Some(&artifact_validation),
    );
    assert_eq!(status.0, "completed");

    let output = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        &session.id,
        Some(&run.run_id),
        session_text,
        accepted_output.clone(),
        Some(artifact_validation.clone()),
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output.clone(),
        AutomationRunStatus::Completed,
        1,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Completed);
    assert_eq!(
        persisted.checkpoint.node_attempts.get("research_mcp"),
        Some(&1)
    );

    let output = persisted
        .checkpoint
        .node_outputs
        .get("research_mcp")
        .expect("node output");
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["mcp.tandem_mcp.search_docs", "write"])
    );

    let written = std::fs::read_to_string(
        workspace_root
            .join(".tandem/runs")
            .join(&run.run_id)
            .join("artifacts")
            .join("research-sources.json"),
    )
    .expect("written artifact");
    assert_eq!(written, artifact_text);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn external_web_research_flow_completes_with_websearch_and_write() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-web-research-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("docs")).expect("create workspace");
    std::fs::write(
        workspace_root.join("docs/source.md"),
        "# Source\n\nWorkspace evidence for the web-backed brief.\n",
    )
    .expect("seed source file");

    let state = ready_test_state().await;

    let node = brief_research_node("research_web", ".tandem/artifacts/web-brief.md", true);
    let automation = automation_with_single_node(
        "automation-web-research",
        node.clone(),
        &workspace_root,
        vec!["read".to_string()],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let output_path = automation_node_required_output_path_for_run(&node, Some(&run.run_id))
        .expect("required output path");
    let workspace_snapshot_before = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let artifact_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n### Files Reviewed\n| Local Path | Evidence Summary |\n|---|---|\n| `docs/source.md` | Core source reviewed |\n\n### Files Not Reviewed\n| Local Path | Reason |\n|---|---|\n| `docs/extra.md` | Out of scope for this run |\n\n### Web Sources Reviewed\n| URL | Status | Notes |\n|---|---|---|\n| https://example.com | Fetched | Confirmed live |\n\n## Campaign goal\nClarify positioning.\n\n## Target audience\n- Operators.\n\n## Core pain points\n- Coordination overhead.\n\n## Positioning angle\nTandem centralizes orchestration.\n\n## Competitor context\nExternal web comparison for this run.\n\n## Proof points with citations\n1. Supported from docs/source.md. Source note: https://example.com/reference\n\n## Likely objections\n- Proof depth.\n\n## Channel considerations\n- Landing page.\n\n## Recommended message hierarchy\n1. Problem\n2. Promise\n"
        .to_string();

    let artifact_dir = workspace_root
        .join(".tandem/runs")
        .join(&run.run_id)
        .join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::write(artifact_dir.join("web-brief.md"), &artifact_text).expect("write artifact");

    let session = assistant_session_with_tool_invocations(
        "web-research-validation",
        &workspace_root,
        vec![
            (
                "glob",
                json!({"pattern":"docs/**/*.md"}),
                json!({
                    "output": workspace_root
                        .join("docs/source.md")
                        .display()
                        .to_string()
                }),
                None,
            ),
            (
                "read",
                json!({"path":"docs/source.md"}),
                json!({"output":"Workspace evidence for the web-backed brief."}),
                None,
            ),
            (
                "websearch",
                json!({"query":"tandem competitor landscape"}),
                json!({
                    "output": "Matched Tandem web research",
                    "metadata": {"count": 2}
                }),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":artifact_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let requested_tools = vec![
        "glob".to_string(),
        "read".to_string(),
        "websearch".to_string(),
        "write".to_string(),
    ];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    assert_eq!(
        tool_telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["glob", "read", "websearch", "write"])
    );
    assert_eq!(
        tool_telemetry
            .get("web_research_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        tool_telemetry
            .get("web_research_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Queued);
    assert_eq!(persisted.checkpoint.node_attempts.get("research_web"), None);

    let session_text = "Done\n\n{\"status\":\"completed\"}";
    let (accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &session,
        workspace_root.to_str().expect("workspace root string"),
        session_text,
        &tool_telemetry,
        None,
        Some((output_path.clone(), artifact_text.clone())),
        &workspace_snapshot_before,
    );
    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("web_sources_reviewed_present")
            .and_then(Value::as_bool),
        Some(true)
    );

    let status = detect_automation_node_status(
        &node,
        session_text,
        accepted_output.as_ref(),
        &tool_telemetry,
        Some(&artifact_validation),
    );
    assert_eq!(status.0, "completed");

    let output = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        &session.id,
        Some(&run.run_id),
        session_text,
        accepted_output.clone(),
        Some(artifact_validation.clone()),
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output.clone(),
        AutomationRunStatus::Completed,
        1,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Completed);
    assert_eq!(
        persisted.checkpoint.node_attempts.get("research_web"),
        Some(&1)
    );

    let output = persisted
        .checkpoint
        .node_outputs
        .get("research_web")
        .expect("node output");
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/web_research_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/web_research_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );
    let output_tools = output
        .pointer("/tool_telemetry/executed_tools")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .expect("output tools");
    assert!(output_tools.iter().any(|tool| *tool == "glob"));
    assert!(output_tools.iter().any(|tool| *tool == "read"));
    assert!(output_tools.iter().any(|tool| *tool == "websearch"));
    assert!(output_tools.iter().any(|tool| *tool == "write"));
    assert_eq!(
        output
            .pointer("/artifact_validation/web_sources_reviewed_present")
            .and_then(Value::as_bool),
        Some(true)
    );

    let written = std::fs::read_to_string(
        workspace_root
            .join(".tandem/runs")
            .join(&run.run_id)
            .join("artifacts")
            .join("web-brief.md"),
    )
    .expect("written artifact");
    assert_eq!(written, artifact_text);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn analyze_findings_dual_write_flow_completes_with_artifact_and_workspace_file() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-analyze-findings-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("inputs")).expect("create workspace");
    std::fs::write(
        workspace_root.join("inputs/clustered-findings.md"),
        "# Clustered findings\n\n- Repair loops block release confidence.\n- Missing artifacts break downstream synthesis.\n",
    )
    .expect("seed clustered findings");

    let state = ready_test_state().await;
    let workspace_file = "reports/pain-points-analysis.md";
    let node = analyze_findings_node(
        "analyze_findings",
        ".tandem/artifacts/analyze-findings.json",
        workspace_file,
    );
    let automation = automation_with_single_node(
        "automation-analyze-findings",
        node.clone(),
        &workspace_root,
        vec!["glob".to_string(), "read".to_string(), "write".to_string()],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let output_path = automation_node_required_output_path_for_run(&node, Some(&run.run_id))
        .expect("required output path");
    let workspace_snapshot_before = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let artifact_text = serde_json::to_string_pretty(&json!({
        "status": "completed",
        "pain_points": [
            "Repair loops reduce operator confidence.",
            "Artifact contract misses block downstream steps."
        ],
        "recommended_actions": [
            "Add replay regressions for escaped workflow bugs.",
            "Tighten required output enforcement for synthesis nodes."
        ],
        "summary": "Structured analysis generated from clustered workflow findings."
    }))
    .expect("artifact json");
    let workspace_file_text = "# Pain Points Analysis\n\n## Key Patterns\n- Repair loops reduce operator confidence.\n- Missing artifacts block downstream synthesis.\n\n## Recommended Actions\n1. Add replay regressions for escaped workflow bugs.\n2. Tighten required output enforcement for synthesis nodes.\n";

    let artifact_dir = workspace_root
        .join(".tandem/runs")
        .join(&run.run_id)
        .join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::create_dir_all(workspace_root.join("reports")).expect("create reports dir");
    std::fs::write(artifact_dir.join("analyze-findings.json"), &artifact_text)
        .expect("write artifact");
    std::fs::write(workspace_root.join(workspace_file), workspace_file_text)
        .expect("write workspace file");

    let session = assistant_session_with_tool_invocations(
        "analyze-findings-validation",
        &workspace_root,
        vec![
            (
                "glob",
                json!({"pattern":"inputs/**/*.md"}),
                json!({
                    "output": workspace_root
                        .join("inputs/clustered-findings.md")
                        .display()
                        .to_string()
                }),
                None,
            ),
            (
                "read",
                json!({"path":"inputs/clustered-findings.md"}),
                json!({"output":"Repair loops block release confidence."}),
                None,
            ),
            (
                "write",
                json!({"path":workspace_file,"content":workspace_file_text}),
                json!({"ok": true}),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":artifact_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let requested_tools = vec!["glob".to_string(), "read".to_string(), "write".to_string()];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    assert_eq!(
        tool_telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["glob", "read", "write"])
    );
    assert_eq!(
        tool_telemetry
            .pointer("/tool_call_counts/write")
            .and_then(Value::as_u64),
        Some(2)
    );

    let session_text = format!("{artifact_text}\n\n{{\"status\":\"completed\"}}");
    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root string"),
            Some(&run.run_id),
            &session_text,
            &tool_telemetry,
            None,
            Some((output_path.clone(), artifact_text.clone())),
            &workspace_snapshot_before,
            None,
        );
    assert!(rejected.is_none());
    let validation_outcome = artifact_validation
        .get("validation_outcome")
        .and_then(Value::as_str);
    assert!(
        validation_outcome == Some("passed"),
        "artifact_validation={}",
        serde_json::to_string_pretty(&artifact_validation).expect("artifact validation json")
    );
    assert!(artifact_validation
        .pointer("/validation_basis/must_write_file_statuses")
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|value| {
            value.get("path").and_then(Value::as_str) == Some(workspace_file)
                && value
                    .get("touched_by_current_attempt")
                    .and_then(Value::as_bool)
                    == Some(true)
                && value
                    .get("materialized_by_current_attempt")
                    .and_then(Value::as_bool)
                    == Some(true)
        })));

    let status = detect_automation_node_status(
        &node,
        &session_text,
        accepted_output.as_ref(),
        &tool_telemetry,
        Some(&artifact_validation),
    );
    assert_eq!(status.0, "completed");

    let output = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        &session.id,
        Some(&run.run_id),
        &session_text,
        accepted_output.clone(),
        Some(artifact_validation.clone()),
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output,
        AutomationRunStatus::Completed,
        1,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    let output = persisted
        .checkpoint
        .node_outputs
        .get("analyze_findings")
        .expect("node output");
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_basis/must_write_files")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        vec![Value::String(workspace_file.to_string())]
    );

    let written_artifact = std::fs::read_to_string(artifact_dir.join("analyze-findings.json"))
        .expect("written artifact");
    assert_eq!(written_artifact, artifact_text);
    let written_workspace_file =
        std::fs::read_to_string(workspace_root.join(workspace_file)).expect("workspace file");
    assert_eq!(written_workspace_file, workspace_file_text);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn compare_results_synthesis_flow_completes_with_upstream_evidence() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-compare-results-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("content/blog-memory")).expect("create workspace");
    std::fs::write(
        workspace_root.join("content/blog-memory/used-themes.md"),
        "# Used Themes\n\n- workflow repair loops\n- release confidence\n",
    )
    .expect("seed memory file");

    let state = ready_test_state().await;
    let node = compare_results_node("compare_results", ".tandem/artifacts/compare-results.md");
    let automation = automation_with_single_node(
        "automation-compare-results",
        node.clone(),
        &workspace_root,
        vec![
            "glob".to_string(),
            "read".to_string(),
            "mcp_list".to_string(),
            "mcp.blog_mcp.list_blog_drafts".to_string(),
            "write".to_string(),
        ],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let output_path = automation_node_required_output_path_for_run(&node, Some(&run.run_id))
        .expect("required output path");
    let workspace_snapshot_before = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let artifact_text = "# Recent Blog Review\n\n## Memory Themes\n\nPersistent memory in `content/blog-memory/used-themes.md` shows that workflow repair loops and release confidence are already well-trodden editorial territory. That means a fresh post should treat those ideas as context, not as the entire hook.\n\n## Upstream Grounding\n\nThe upstream handoffs in `.tandem/runs/run-compare/artifacts/collect-inputs.json` and `.tandem/runs/run-compare/artifacts/research-sources.json` already establish the run context, the approved Tandem terminology, and the tool inventory around tandem-mcp. A successful follow-on piece should preserve that terminology and use it as proof, rather than resetting to vague \"AI workflow\" language.\n\n## Recent Blog History\n\nThe `mcp.blog_mcp.list_blog_drafts` inspection shows that recent Tandem drafts emphasize orchestration reliability, faster recovery loops, and operator trust. Those drafts tend to open from a concrete operator pain point and then connect that pain to product truth, which is working well but is now close to becoming repetitive.\n\n## Repeated Framing To Avoid\n\nWe should avoid another opener that says repair loops are frustrating without adding new evidence. We should also avoid generic workflow-quality language that does not tie back to concrete artifacts, because the upstream evidence already gives us stronger anchors than that.\n\n## Unexplored Angles\n\nA stronger next angle is release-safety testing as a differentiator: how deterministic workflow contracts, replay coverage, and repair guidance reduce the operational cost of running agent systems in production. Another viable angle is contrasting structured workflow contracts with ad hoc orchestration, using the upstream Tandem grounding as the product-proof section instead of as a vague capabilities summary.\n\n## Recommended Direction\n\nThe best follow-up post should combine the memory evidence from `content/blog-memory/used-themes.md`, the Tandem grounding from `.tandem/runs/run-compare/artifacts/research-sources.json`, and the recent blog-pattern scan from `mcp.blog_mcp.list_blog_drafts`. That gives us a post with a new claim, grounded proof points, and a clear explanation of what not to repeat.\n";

    let artifact_dir = workspace_root
        .join(".tandem/runs")
        .join(&run.run_id)
        .join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::write(artifact_dir.join("compare-results.md"), artifact_text).expect("write artifact");

    let session = assistant_session_with_tool_invocations(
        "compare-results-validation",
        &workspace_root,
        vec![
            (
                "mcp_list",
                json!({}),
                json!({
                    "output": {
                        "connected_server_names": ["blog-mcp"],
                        "registered_tools": ["mcp.blog_mcp.list_blog_drafts"]
                    }
                }),
                None,
            ),
            (
                "glob",
                json!({"pattern":"content/blog-memory/*.md"}),
                json!({
                    "output": workspace_root
                        .join("content/blog-memory/used-themes.md")
                        .display()
                        .to_string()
                }),
                None,
            ),
            (
                "read",
                json!({"path":"content/blog-memory/used-themes.md"}),
                json!({"output":"workflow repair loops\nrelease confidence"}),
                None,
            ),
            (
                "mcp.blog_mcp.list_blog_drafts",
                json!({"limit": 3}),
                json!({
                    "output": "Recent drafts emphasize orchestration reliability and recovery loops."
                }),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":artifact_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let requested_tools = vec![
        "mcp_list".to_string(),
        "glob".to_string(),
        "read".to_string(),
        "mcp.blog_mcp.list_blog_drafts".to_string(),
        "write".to_string(),
    ];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    let upstream_evidence = AutomationUpstreamEvidence {
        read_paths: vec![
            ".tandem/runs/run-compare/artifacts/collect-inputs.json".to_string(),
            ".tandem/runs/run-compare/artifacts/research-sources.json".to_string(),
        ],
        discovered_relevant_paths: vec![
            ".tandem/runs/run-compare/artifacts/research-sources.json".to_string()
        ],
        web_research_attempted: false,
        web_research_succeeded: false,
        citation_count: 2,
        citations: vec![
            "Tandem MCP Guide".to_string(),
            "Blog history inspection from blog-mcp".to_string(),
        ],
    };

    let session_text = "Standup-like synthesis complete.\n\n{\"status\":\"completed\"}";
    let (accepted_output, artifact_validation, rejected) =
        validate_automation_artifact_output_with_upstream(
            &node,
            &session,
            workspace_root.to_str().expect("workspace root string"),
            Some(&run.run_id),
            session_text,
            &tool_telemetry,
            None,
            Some((output_path.clone(), artifact_text.to_string())),
            &workspace_snapshot_before,
            Some(&upstream_evidence),
        );
    assert!(rejected.is_none());
    let validation_outcome = artifact_validation
        .get("validation_outcome")
        .and_then(Value::as_str);
    assert!(
        validation_outcome == Some("passed"),
        "artifact_validation={}",
        serde_json::to_string_pretty(&artifact_validation).expect("artifact validation json")
    );
    assert_eq!(
        artifact_validation
            .pointer("/validation_basis/upstream_evidence_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(artifact_validation
        .get("read_paths")
        .and_then(Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| { value.as_str() == Some("content/blog-memory/used-themes.md") })));

    let status = detect_automation_node_status(
        &node,
        session_text,
        accepted_output.as_ref(),
        &tool_telemetry,
        Some(&artifact_validation),
    );
    assert_eq!(status.0, "completed");

    let output = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        &session.id,
        Some(&run.run_id),
        session_text,
        accepted_output.clone(),
        Some(artifact_validation.clone()),
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output,
        AutomationRunStatus::Completed,
        1,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    let output = persisted
        .checkpoint
        .node_outputs
        .get("compare_results")
        .expect("node output");
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_basis/upstream_evidence_used")
            .and_then(Value::as_bool),
        Some(true)
    );

    let written =
        std::fs::read_to_string(artifact_dir.join("compare-results.md")).expect("written artifact");
    assert_eq!(written, artifact_text);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn delivery_flow_completes_with_validated_artifact_body_and_email_send() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-delivery-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("reports")).expect("create workspace");
    let report_path = "reports/final-report.md";
    let report_text = "# Final Report\n\n## Highlights\n- Deterministic workflow contracts reduced repair churn.\n- Replay coverage caught escaped bugs before release.\n\n## Recommendation\nShip the gated workflow coverage bundle.\n";
    std::fs::write(workspace_root.join(report_path), report_text).expect("seed report");

    let state = ready_test_state().await;
    let node = delivery_node("notify_release_owner", "release-owner@example.com");
    let automation = automation_with_single_node(
        "automation-delivery",
        node.clone(),
        &workspace_root,
        vec![
            "read".to_string(),
            "mcp.composio_1.gmail_send_email".to_string(),
        ],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");

    let session = assistant_session_with_tool_invocations(
        "delivery-validation",
        &workspace_root,
        vec![
            (
                "read",
                json!({"path": report_path}),
                json!({"output": report_text}),
                None,
            ),
            (
                "mcp.composio_1.gmail_send_email",
                json!({
                    "to": "release-owner@example.com",
                    "subject": "Workflow release candidate",
                    "html_body": "<h1>Final Report</h1><p>Deterministic workflow contracts reduced repair churn.</p>"
                }),
                json!({
                    "output": "Email sent",
                    "metadata": {
                        "delivery_status": "sent",
                        "message_id": "msg_123"
                    }
                }),
                None,
            ),
        ],
    );
    let requested_tools = vec![
        "read".to_string(),
        "mcp.composio_1.gmail_send_email".to_string(),
    ];
    let tool_telemetry = summarize_automation_tool_activity(&node, &session, &requested_tools);
    assert_eq!(
        tool_telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["read", "mcp.composio_1.gmail_send_email"])
    );
    assert_eq!(
        tool_telemetry
            .get("workspace_inspection_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        tool_telemetry
            .get("email_delivery_attempted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        tool_telemetry
            .get("email_delivery_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );

    let session_text = format!(
        "Sent the validated report to release-owner@example.com.\n\n{}",
        serde_json::to_string(&json!({
            "status": "completed",
            "approved": true,
            "report_path": report_path
        }))
        .expect("status json")
    );
    let status = detect_automation_node_status(&node, &session_text, None, &tool_telemetry, None);
    assert_eq!(status.0, "completed");
    assert_eq!(status.1, None);
    assert_eq!(status.2, Some(true));

    let output = wrap_automation_node_output(
        &node,
        &session,
        &requested_tools,
        &session.id,
        Some(&run.run_id),
        &session_text,
        None,
        None,
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output,
        AutomationRunStatus::Completed,
        1,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    let output = persisted
        .checkpoint
        .node_outputs
        .get("notify_release_owner")
        .expect("node output");
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(output.get("approved").and_then(Value::as_bool), Some(true));
    assert_eq!(
        output
            .pointer("/tool_telemetry/email_delivery_attempted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/email_delivery_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["read", "mcp.composio_1.gmail_send_email"])
    );

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn code_loop_flow_repairs_after_missing_verification_and_completes() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-code-loop-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("src")).expect("create workspace");
    std::fs::write(
        workspace_root.join("src/lib.rs"),
        "pub fn release_note_title() -> &'static str {\n    \"old title\"\n}\n",
    )
    .expect("seed source");

    let state = ready_test_state().await;
    let node = code_loop_node("implement_release_fix", ".tandem/artifacts/code-loop.md");
    let automation = automation_with_single_node(
        "automation-code-loop",
        node.clone(),
        &workspace_root,
        vec![
            "read".to_string(),
            "apply_patch".to_string(),
            "write".to_string(),
            "bash".to_string(),
        ],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let output_path = automation_node_required_output_path_for_run(&node, Some(&run.run_id))
        .expect("required output path");
    let workspace_snapshot_before = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let handoff_text = "# Implementation Handoff\n\n## Files changed\n- `src/lib.rs`\n\n## Summary\nUpdated the release note title helper to use the repaired title string.\n\n## Verification\n- `cargo test`\n";

    let artifact_dir = workspace_root
        .join(".tandem/runs")
        .join(&run.run_id)
        .join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::write(artifact_dir.join("code-loop.md"), handoff_text).expect("write artifact");
    std::fs::write(
        workspace_root.join("src/lib.rs"),
        "pub fn release_note_title() -> &'static str {\n    \"repaired title\"\n}\n",
    )
    .expect("write patched source");

    let first_session = assistant_session_with_tool_invocations(
        "code-loop-attempt-1",
        &workspace_root,
        vec![
            (
                "read",
                json!({"path":"src/lib.rs"}),
                json!({"output":"pub fn release_note_title() -> &'static str { \"old title\" }\n"}),
                None,
            ),
            (
                "apply_patch",
                json!({"patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn release_note_title() -> &'static str {\n-    \"old title\"\n-}\n+pub fn release_note_title() -> &'static str {\n+    \"repaired title\"\n+}\n*** End Patch\n"}),
                json!({"ok": true}),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":handoff_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let requested_tools = vec![
        "read".to_string(),
        "apply_patch".to_string(),
        "write".to_string(),
        "bash".to_string(),
    ];
    let first_telemetry =
        summarize_automation_tool_activity(&node, &first_session, &requested_tools);
    assert_eq!(
        first_telemetry
            .get("verification_expected")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        first_telemetry
            .get("verification_ran")
            .and_then(Value::as_bool),
        Some(false)
    );

    let first_session_text =
        "Patched the code and wrote the handoff.\n\n{\"status\":\"completed\"}";
    let (first_accepted_output, first_artifact_validation, first_rejected) =
        validate_automation_artifact_output(
            &node,
            &first_session,
            workspace_root.to_str().expect("workspace root string"),
            first_session_text,
            &first_telemetry,
            None,
            Some((output_path.clone(), handoff_text.to_string())),
            &workspace_snapshot_before,
        );
    assert!(first_rejected.is_none());
    assert_eq!(
        first_artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    let first_status = detect_automation_node_status(
        &node,
        first_session_text,
        first_accepted_output.as_ref(),
        &first_telemetry,
        Some(&first_artifact_validation),
    );
    assert_eq!(first_status.0, "needs_repair");
    assert_eq!(
        first_status.1.as_deref(),
        Some("coding task completed without running the declared verification command")
    );

    let second_session = assistant_session_with_tool_invocations(
        "code-loop-attempt-2",
        &workspace_root,
        vec![
            (
                "read",
                json!({"path":"src/lib.rs"}),
                json!({"output":"pub fn release_note_title() -> &'static str { \"repaired title\" }\n"}),
                None,
            ),
            (
                "apply_patch",
                json!({"patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-pub fn release_note_title() -> &'static str {\n-    \"repaired title\"\n-}\n+pub fn release_note_title() -> &'static str {\n+    \"repaired title\"\n+}\n*** End Patch\n"}),
                json!({"ok": true}),
                None,
            ),
            (
                "bash",
                json!({"command":"cargo test"}),
                json!({
                    "output": "test result: ok. 1 passed; 0 failed;",
                    "metadata": {
                        "exit_code": 0
                    }
                }),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":handoff_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let second_telemetry =
        summarize_automation_tool_activity(&node, &second_session, &requested_tools);
    assert_eq!(
        second_telemetry
            .get("verification_ran")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        second_telemetry
            .get("verification_failed")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        second_telemetry
            .get("latest_verification_command")
            .and_then(Value::as_str),
        Some("cargo test")
    );

    let second_session_text =
        "Patched the code, reran verification, and finalized the handoff.\n\n{\"status\":\"completed\"}";
    let (accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &second_session,
        workspace_root.to_str().expect("workspace root string"),
        second_session_text,
        &second_telemetry,
        Some(handoff_text),
        Some((output_path.clone(), handoff_text.to_string())),
        &workspace_snapshot_before,
    );
    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    let status = detect_automation_node_status(
        &node,
        second_session_text,
        accepted_output.as_ref(),
        &second_telemetry,
        Some(&artifact_validation),
    );
    assert_eq!(status.0, "done");

    let output = wrap_automation_node_output(
        &node,
        &second_session,
        &requested_tools,
        &second_session.id,
        Some(&run.run_id),
        second_session_text,
        accepted_output.clone(),
        Some(artifact_validation.clone()),
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output,
        AutomationRunStatus::Completed,
        2,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Completed);
    assert_eq!(
        persisted
            .checkpoint
            .node_attempts
            .get("implement_release_fix"),
        Some(&2)
    );
    let output = persisted
        .checkpoint
        .node_outputs
        .get("implement_release_fix")
        .expect("node output");
    assert_eq!(output.get("status").and_then(Value::as_str), Some("done"));
    assert_eq!(
        output
            .pointer("/tool_telemetry/verification_ran")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/latest_verification_command")
            .and_then(Value::as_str),
        Some("cargo test")
    );

    let written_handoff =
        std::fs::read_to_string(artifact_dir.join("code-loop.md")).expect("written artifact");
    assert_eq!(written_handoff, handoff_text);
    let patched_source =
        std::fs::read_to_string(workspace_root.join("src/lib.rs")).expect("patched source");
    assert!(patched_source.contains("repaired title"));

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn repair_retry_after_needs_repair_completes_on_second_attempt() {
    let workspace_root = std::env::temp_dir().join(format!(
        "tandem-repair-retry-integration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(workspace_root.join("docs")).expect("create workspace");
    std::fs::write(
        workspace_root.join("docs/source.md"),
        "# Source\n\nWorkspace evidence for the retry brief.\n",
    )
    .expect("seed source file");

    let state = ready_test_state().await;

    let mut node = brief_research_node("research_retry", ".tandem/artifacts/retry-brief.md", true);
    node.retry_policy = Some(json!({
        "max_attempts": 2
    }));
    let automation = automation_with_single_node(
        "automation-retry-research",
        node.clone(),
        &workspace_root,
        vec!["read".to_string()],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");
    let output_path = automation_node_required_output_path_for_run(&node, Some(&run.run_id))
        .expect("required output path");
    let workspace_snapshot_before = automation_workspace_root_file_snapshot(
        workspace_root.to_str().expect("workspace root string"),
    );
    let local_brief_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n## Campaign goal\nClarify positioning.\n\n## Target audience\n- Operators.\n\n## Core pain points\n- Coordination overhead.\n\n## Positioning angle\nTandem centralizes orchestration.\n\n## Competitor context\nLocal-only comparison for this first pass.\n\n## Proof points with citations\n1. Supported from docs/source.md. Source note: https://example.com/reference\n\n## Likely objections\n- Proof depth.\n\n## Channel considerations\n- Landing page.\n\n## Recommended message hierarchy\n1. Problem\n2. Promise\n\n## Files reviewed\n- docs/source.md\n\n## Files not reviewed\n- docs/extra.md: not needed for this first pass.\n"
        .to_string();
    let web_brief_text = "# Marketing Brief\n\n## Workspace source audit\nPrepared from workspace sources.\n\n### Files Reviewed\n| Local Path | Evidence Summary |\n|---|---|\n| `docs/source.md` | Core source reviewed |\n\n### Files Not Reviewed\n| Local Path | Reason |\n|---|---|\n| `docs/extra.md` | Out of scope for this run |\n\n### Web Sources Reviewed\n| URL | Status | Notes |\n|---|---|---|\n| https://example.com | Fetched | Confirmed live |\n\n## Campaign goal\nClarify positioning.\n\n## Target audience\n- Operators.\n\n## Core pain points\n- Coordination overhead.\n\n## Positioning angle\nTandem centralizes orchestration.\n\n## Competitor context\nExternal web comparison for the retry run.\n\n## Proof points with citations\n1. Supported from docs/source.md. Source note: https://example.com/reference\n\n## Likely objections\n- Proof depth.\n\n## Channel considerations\n- Landing page.\n\n## Recommended message hierarchy\n1. Problem\n2. Promise\n"
        .to_string();

    let artifact_dir = workspace_root
        .join(".tandem/runs")
        .join(&run.run_id)
        .join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");
    std::fs::write(artifact_dir.join("retry-brief.md"), &local_brief_text)
        .expect("write first artifact");

    let first_session = assistant_session_with_tool_invocations(
        "repair-retry-attempt-1",
        &workspace_root,
        vec![
            (
                "glob",
                json!({"pattern":"docs/**/*.md"}),
                json!({
                    "output": workspace_root
                        .join("docs/source.md")
                        .display()
                        .to_string()
                }),
                None,
            ),
            (
                "read",
                json!({"path":"docs/source.md"}),
                json!({"output":"Workspace evidence for the retry brief."}),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":local_brief_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let requested_tools = vec![
        "glob".to_string(),
        "read".to_string(),
        "websearch".to_string(),
        "write".to_string(),
    ];
    let first_telemetry =
        summarize_automation_tool_activity(&node, &first_session, &requested_tools);
    assert_eq!(
        first_telemetry
            .get("executed_tools")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
        Some(vec!["glob", "read", "write"])
    );
    assert_eq!(
        first_telemetry
            .get("web_research_used")
            .and_then(Value::as_bool),
        Some(false)
    );

    let first_session_text = "Done\n\n{\"status\":\"completed\"}";
    let (first_accepted_output, first_artifact_validation, first_rejected) =
        validate_automation_artifact_output(
            &node,
            &first_session,
            workspace_root.to_str().expect("workspace root string"),
            first_session_text,
            &first_telemetry,
            None,
            Some((output_path.clone(), local_brief_text.clone())),
            &workspace_snapshot_before,
        );
    assert_eq!(
        first_artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("needs_repair")
    );
    let first_status = detect_automation_node_status(
        &node,
        first_session_text,
        first_accepted_output.as_ref(),
        &first_telemetry,
        Some(&first_artifact_validation),
    );
    assert_eq!(first_status.0, "needs_repair");
    assert!(first_rejected.is_some());
    assert!(first_artifact_validation
        .get("semantic_block_reason")
        .and_then(Value::as_str)
        .is_some());

    std::fs::write(artifact_dir.join("retry-brief.md"), &web_brief_text)
        .expect("write repaired artifact");

    let second_session = assistant_session_with_tool_invocations(
        "repair-retry-attempt-2",
        &workspace_root,
        vec![
            (
                "glob",
                json!({"pattern":"docs/**/*.md"}),
                json!({
                    "output": workspace_root
                        .join("docs/source.md")
                        .display()
                        .to_string()
                }),
                None,
            ),
            (
                "read",
                json!({"path":"docs/source.md"}),
                json!({"output":"Workspace evidence for the retry brief."}),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":local_brief_text}),
                json!({"ok": true}),
                None,
            ),
            (
                "websearch",
                json!({"query":"tandem competitor landscape"}),
                json!({
                    "output": "Matched Tandem web research",
                    "metadata": {"count": 2}
                }),
                None,
            ),
            (
                "write",
                json!({"path":output_path,"content":web_brief_text}),
                json!({"ok": true}),
                None,
            ),
        ],
    );
    let second_telemetry =
        summarize_automation_tool_activity(&node, &second_session, &requested_tools);
    let second_executed_tools = second_telemetry
        .get("executed_tools")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .expect("executed tools");
    assert!(second_executed_tools.iter().any(|tool| *tool == "glob"));
    assert!(second_executed_tools.iter().any(|tool| *tool == "read"));
    assert!(second_executed_tools
        .iter()
        .any(|tool| *tool == "websearch"));
    assert!(second_executed_tools.iter().any(|tool| *tool == "write"));
    assert_eq!(
        second_telemetry
            .pointer("/tool_call_counts/write")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        second_telemetry
            .get("web_research_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        second_telemetry
            .get("web_research_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );

    let second_session_text = "Done\n\n{\"status\":\"completed\"}";
    let (accepted_output, artifact_validation, rejected) = validate_automation_artifact_output(
        &node,
        &second_session,
        workspace_root.to_str().expect("workspace root string"),
        second_session_text,
        &second_telemetry,
        Some(&local_brief_text),
        Some((output_path.clone(), web_brief_text.clone())),
        &workspace_snapshot_before,
    );
    assert!(rejected.is_none());
    assert_eq!(
        artifact_validation
            .get("validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        artifact_validation
            .get("repair_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );

    let status = detect_automation_node_status(
        &node,
        second_session_text,
        accepted_output.as_ref(),
        &second_telemetry,
        Some(&artifact_validation),
    );
    assert_eq!(status.0, "completed");

    let output = wrap_automation_node_output(
        &node,
        &second_session,
        &requested_tools,
        &second_session.id,
        Some(&run.run_id),
        second_session_text,
        accepted_output.clone(),
        Some(artifact_validation.clone()),
    );
    persist_validated_output(
        &state,
        &run.run_id,
        &node.node_id,
        output.clone(),
        AutomationRunStatus::Completed,
        2,
    )
    .await;

    let persisted = state
        .get_automation_v2_run(&run.run_id)
        .await
        .expect("persisted run");
    assert_eq!(persisted.status, AutomationRunStatus::Completed);
    assert_eq!(
        persisted.checkpoint.node_attempts.get("research_retry"),
        Some(&2)
    );

    let output = persisted
        .checkpoint
        .node_outputs
        .get("research_retry")
        .expect("node output");
    assert_eq!(
        output.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output
            .pointer("/artifact_validation/validation_outcome")
            .and_then(Value::as_str),
        Some("passed")
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/web_research_used")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output
            .pointer("/tool_telemetry/web_research_succeeded")
            .and_then(Value::as_bool),
        Some(true)
    );
    let output_tools = output
        .pointer("/tool_telemetry/executed_tools")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .expect("output tools");
    assert!(output_tools.iter().any(|tool| *tool == "glob"));
    assert!(output_tools.iter().any(|tool| *tool == "read"));
    assert!(output_tools.iter().any(|tool| *tool == "websearch"));
    assert!(output_tools.iter().any(|tool| *tool == "write"));

    let written = std::fs::read_to_string(
        workspace_root
            .join(".tandem/runs")
            .join(&run.run_id)
            .join("artifacts")
            .join("retry-brief.md"),
    )
    .expect("written artifact");
    assert_eq!(written, web_brief_text);

    let _ = std::fs::remove_dir_all(&workspace_root);
}

#[tokio::test]
async fn restart_recovery_preserves_queued_and_paused_runs() {
    let paused_workspace =
        std::env::temp_dir().join(format!("tandem-recovery-paused-{}", uuid::Uuid::new_v4()));
    let queued_workspace =
        std::env::temp_dir().join(format!("tandem-recovery-queued-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&paused_workspace).expect("create paused workspace");
    std::fs::create_dir_all(&queued_workspace).expect("create queued workspace");

    let state = ready_test_state().await;
    let paused_automation = automation_with_single_node(
        "automation-paused-recovery",
        brief_research_node("paused_node", ".tandem/artifacts/paused.md", false),
        &paused_workspace,
        vec!["read".to_string()],
    );
    let queued_automation = automation_with_single_node(
        "automation-queued-recovery",
        brief_research_node("queued_node", ".tandem/artifacts/queued.md", false),
        &queued_workspace,
        vec!["read".to_string()],
    );

    let paused_run = state
        .create_automation_v2_run(&paused_automation, "manual")
        .await
        .expect("create paused run");
    let queued_run = state
        .create_automation_v2_run(&queued_automation, "manual")
        .await
        .expect("create queued run");

    state
        .update_automation_v2_run(&paused_run.run_id, |row| {
            row.status = AutomationRunStatus::Paused;
            row.pause_reason = Some("paused for recovery test".to_string());
            row.detail = Some("paused for recovery test".to_string());
            row.active_session_ids.clear();
            row.active_instance_ids.clear();
        })
        .await
        .expect("mark paused");

    let recovered = state.recover_in_flight_runs().await;
    assert_eq!(recovered, 0);

    let scheduler = state.automation_scheduler.read().await;
    assert!(!scheduler
        .locked_workspaces
        .contains_key(&paused_workspace.to_string_lossy().to_string()));
    assert!(!scheduler
        .locked_workspaces
        .contains_key(&queued_workspace.to_string_lossy().to_string()));
    drop(scheduler);

    let paused_persisted = state
        .get_automation_v2_run(&paused_run.run_id)
        .await
        .expect("paused run");
    let queued_persisted = state
        .get_automation_v2_run(&queued_run.run_id)
        .await
        .expect("queued run");
    assert_eq!(paused_persisted.status, AutomationRunStatus::Paused);
    assert_eq!(queued_persisted.status, AutomationRunStatus::Queued);

    let _ = std::fs::remove_dir_all(&paused_workspace);
    let _ = std::fs::remove_dir_all(&queued_workspace);
}

#[tokio::test]
async fn provider_usage_is_attributed_from_correlation_id_without_session_mapping() {
    let workspace_root =
        std::env::temp_dir().join(format!("tandem-usage-correlation-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace_root).expect("create workspace");

    let mut state = ready_test_state().await;
    state.token_cost_per_1k_usd = 12.5;

    let usage_aggregator = tokio::spawn(run_usage_aggregator(state.clone()));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let automation = automation_with_single_node(
        "automation-usage-correlation",
        brief_research_node("usage_node", ".tandem/artifacts/usage.md", false),
        &workspace_root,
        vec!["read".to_string()],
    );
    let run = state
        .create_automation_v2_run(&automation, "manual")
        .await
        .expect("create run");

    state.event_bus.publish(EngineEvent::new(
        "provider.usage",
        json!({
            "sessionID": "session-unused",
            "correlationID": format!("automation-v2:{}", run.run_id),
            "messageID": "message-usage",
            "promptTokens": 11,
            "completionTokens": 19,
            "totalTokens": 30,
        }),
    ));

    let updated = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Some(run) = state.get_automation_v2_run(&run.run_id).await {
                if run.total_tokens == 30 {
                    return run;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("usage attribution timeout");

    assert_eq!(updated.prompt_tokens, 11);
    assert_eq!(updated.completion_tokens, 19);
    assert_eq!(updated.total_tokens, 30);
    assert!(updated.estimated_cost_usd > 0.0);
    assert!(
        (updated.estimated_cost_usd - 0.375).abs() < 0.000_001,
        "expected estimated cost to be derived from usage"
    );

    usage_aggregator.abort();
    let _ = usage_aggregator.await;
    let _ = std::fs::remove_dir_all(&workspace_root);
}
