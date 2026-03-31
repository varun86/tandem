use serde_json::Value;
use tandem_types::MessagePart;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessagePartReduceAction {
    AppendedNewPart,
    UpdatedPendingInvocation,
    ResolvedPendingInvocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolArgsReplaceReason {
    ExistingArgsEmpty,
    IncomingAddsExecutionContext,
    IncomingAddsTerminalFields,
    ExistingArgsMalformed,
    IncomingArgsMoreStructured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MessagePartReduceResult {
    pub action: MessagePartReduceAction,
    pub target_index: usize,
    pub args_replace_reason: Option<ToolArgsReplaceReason>,
}

pub(crate) fn reduce_message_parts(
    parts: &mut Vec<MessagePart>,
    incoming: MessagePart,
) -> MessagePartReduceResult {
    match incoming {
        MessagePart::ToolInvocation {
            tool,
            args,
            result,
            error,
        } => {
            let target_index = parts.iter().rposition(|existing| {
                matches!(
                    existing,
                    MessagePart::ToolInvocation {
                        tool: existing_tool,
                        result: None,
                        error: None,
                        ..
                    } if existing_tool == &tool
                )
            });

            if let Some(target_index) = target_index {
                if let Some(MessagePart::ToolInvocation {
                    args: existing_args,
                    result: existing_result,
                    error: existing_error,
                    ..
                }) = parts.get_mut(target_index)
                {
                    let args_replace_reason =
                        should_replace_tool_args(existing_args, &args).map(|reason| {
                            *existing_args = args.clone();
                            reason
                        });
                    if result.is_some() || error.is_some() {
                        *existing_result = result;
                        *existing_error = error;
                        return MessagePartReduceResult {
                            action: MessagePartReduceAction::ResolvedPendingInvocation,
                            target_index,
                            args_replace_reason,
                        };
                    }
                    return MessagePartReduceResult {
                        action: MessagePartReduceAction::UpdatedPendingInvocation,
                        target_index,
                        args_replace_reason,
                    };
                }
            }

            let target_index = parts.len();
            parts.push(MessagePart::ToolInvocation {
                tool,
                args,
                result,
                error,
            });
            MessagePartReduceResult {
                action: MessagePartReduceAction::AppendedNewPart,
                target_index,
                args_replace_reason: None,
            }
        }
        other => {
            let target_index = parts.len();
            parts.push(other);
            MessagePartReduceResult {
                action: MessagePartReduceAction::AppendedNewPart,
                target_index,
                args_replace_reason: None,
            }
        }
    }
}

fn tool_args_are_empty(args: &Value) -> bool {
    match args {
        Value::Null => true,
        Value::Object(values) => values.is_empty(),
        Value::Array(values) => values.is_empty(),
        Value::String(value) => value.trim().is_empty(),
        _ => false,
    }
}

fn tool_args_have_more_structure(existing: &Value, incoming: &Value) -> bool {
    match (existing, incoming) {
        (Value::String(current), Value::Object(values)) => {
            !current.trim().is_empty() && !values.is_empty()
        }
        (Value::Object(current), Value::Object(next)) => {
            next.len() > current.len()
                && current
                    .iter()
                    .all(|(key, value)| next.get(key) == Some(value))
        }
        _ => false,
    }
}

fn object_has_non_empty_string_field(obj: &serde_json::Map<String, Value>, key: &str) -> bool {
    obj.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

fn incoming_object_adds_terminal_args(existing: &Value, incoming: &Value) -> bool {
    let (Some(existing_obj), Some(incoming_obj)) = (existing.as_object(), incoming.as_object())
    else {
        return false;
    };

    let terminals = ["path", "content", "query", "url", "pattern", "old", "new"];
    terminals.iter().any(|key| {
        object_has_non_empty_string_field(incoming_obj, key)
            && !object_has_non_empty_string_field(existing_obj, key)
    })
}

fn incoming_object_adds_execution_context(existing: &Value, incoming: &Value) -> bool {
    let (Some(existing_obj), Some(incoming_obj)) = (existing.as_object(), incoming.as_object())
    else {
        return false;
    };

    let context_keys = ["__workspace_root", "__effective_cwd", "__session_id"];
    let incoming_has_context = context_keys
        .iter()
        .any(|key| incoming_obj.contains_key(*key));
    if !incoming_has_context {
        return false;
    }
    let existing_has_context = context_keys
        .iter()
        .any(|key| existing_obj.contains_key(*key));
    incoming_has_context && !existing_has_context
}

fn tool_args_object_looks_malformed(args: &Value) -> bool {
    let Some(obj) = args.as_object() else {
        return false;
    };
    if obj.is_empty() {
        return false;
    }
    obj.keys().all(|key| {
        !key.chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            || key.contains('{')
            || key.contains('}')
            || key.contains('"')
            || key.contains('\'')
    })
}

fn should_replace_tool_args(existing: &Value, incoming: &Value) -> Option<ToolArgsReplaceReason> {
    if tool_args_are_empty(incoming) {
        return tool_args_are_empty(existing).then_some(ToolArgsReplaceReason::ExistingArgsEmpty);
    }
    if incoming_object_adds_execution_context(existing, incoming) {
        return Some(ToolArgsReplaceReason::IncomingAddsExecutionContext);
    }
    if incoming_object_adds_terminal_args(existing, incoming) {
        return Some(ToolArgsReplaceReason::IncomingAddsTerminalFields);
    }
    if tool_args_object_looks_malformed(existing) && !tool_args_object_looks_malformed(incoming) {
        return Some(ToolArgsReplaceReason::ExistingArgsMalformed);
    }
    if tool_args_are_empty(existing) {
        return Some(ToolArgsReplaceReason::ExistingArgsEmpty);
    }
    if tool_args_have_more_structure(existing, incoming) {
        return Some(ToolArgsReplaceReason::IncomingArgsMoreStructured);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tandem_types::MessagePart;

    fn tool_part(
        tool: &str,
        args: Value,
        result: Option<Value>,
        error: Option<&str>,
    ) -> MessagePart {
        MessagePart::ToolInvocation {
            tool: tool.to_string(),
            args,
            result,
            error: error.map(str::to_string),
        }
    }

    fn text_part(text: &str) -> MessagePart {
        MessagePart::Text {
            text: text.to_string(),
        }
    }

    fn reducer_result(
        parts: &mut Vec<MessagePart>,
        incoming: MessagePart,
    ) -> (MessagePartReduceResult, Vec<MessagePart>) {
        let result = reduce_message_parts(parts, incoming);
        (result, parts.clone())
    }

    #[test]
    fn appends_tool_invocation_and_result_when_no_pending_same_tool_exists() {
        let mut parts = vec![text_part("hello")];
        let (result, parts) = reducer_result(
            &mut parts,
            tool_part(
                "write",
                json!({"path":"game.html"}),
                Some(json!("ok")),
                None,
            ),
        );

        assert_eq!(result.action, MessagePartReduceAction::AppendedNewPart);
        assert_eq!(result.target_index, 1);
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn updates_pending_tool_invocation_args_when_richer_args_arrive() {
        let mut parts = vec![
            text_part("hello"),
            tool_part("write", json!({"path":"game.html"}), None, None),
        ];

        let (result, parts) = reducer_result(
            &mut parts,
            tool_part(
                "write",
                json!({"path":"game.html","content":"<html></html>"}),
                None,
                None,
            ),
        );

        assert_eq!(
            result.action,
            MessagePartReduceAction::UpdatedPendingInvocation
        );
        match &parts[1] {
            MessagePart::ToolInvocation { args, .. } => {
                assert_eq!(args["content"], "<html></html>");
            }
            other => panic!("expected tool part, got {other:?}"),
        }
    }

    #[test]
    fn resolves_pending_tool_invocation_with_result() {
        let mut parts = vec![
            text_part("hello"),
            tool_part("write", json!({"path":"game.html"}), None, None),
        ];

        let (result, parts) = reducer_result(
            &mut parts,
            tool_part("write", json!({}), Some(json!("ok")), None),
        );

        assert_eq!(
            result.action,
            MessagePartReduceAction::ResolvedPendingInvocation
        );
        match &parts[1] {
            MessagePart::ToolInvocation { result, error, .. } => {
                assert_eq!(result.as_ref(), Some(&json!("ok")));
                assert_eq!(error.as_deref(), None);
            }
            other => panic!("expected tool part, got {other:?}"),
        }
    }

    #[test]
    fn upgrades_raw_string_args_to_structured_invocation_args() {
        let mut parts = vec![
            text_part("hello"),
            tool_part("write", json!("{\"path\":\"game.html\"}"), None, None),
        ];

        let (result, parts) = reducer_result(
            &mut parts,
            tool_part("write", json!({"path":"game.html"}), None, None),
        );

        assert_eq!(
            result.args_replace_reason,
            Some(ToolArgsReplaceReason::IncomingArgsMoreStructured)
        );
        match &parts[1] {
            MessagePart::ToolInvocation { args, .. } => {
                assert_eq!(args["path"], "game.html");
            }
            other => panic!("expected tool part, got {other:?}"),
        }
    }

    #[test]
    fn replaces_malformed_object_args_with_structured_result_args() {
        let mut parts = vec![
            text_part("hello"),
            tool_part("write", json!({"{\"allow_empty": null}), None, None),
        ];

        let (result, parts) = reducer_result(
            &mut parts,
            tool_part(
                "write",
                json!({"path":"game.html","content":"<html></html>"}),
                Some(json!("ok")),
                None,
            ),
        );

        assert_eq!(
            result.args_replace_reason,
            Some(ToolArgsReplaceReason::IncomingAddsTerminalFields)
        );
        match &parts[1] {
            MessagePart::ToolInvocation { args, result, .. } => {
                assert_eq!(args["path"], "game.html");
                assert_eq!(result.as_ref(), Some(&json!("ok")));
            }
            other => panic!("expected tool part, got {other:?}"),
        }
    }

    #[test]
    fn prefers_execution_context_over_pending_raw_args() {
        let mut parts = vec![
            text_part("hello"),
            tool_part("write", json!({"path":"."}), None, None),
        ];

        let (result, parts) = reducer_result(
            &mut parts,
            tool_part(
                "write",
                json!({
                    "path": ".tandem/runs/run-123/artifacts/research-sources.json",
                    "content": "draft",
                    "__workspace_root": "/home/evan/marketing-tandem",
                    "__effective_cwd": "/home/evan/marketing-tandem"
                }),
                Some(json!("ok")),
                None,
            ),
        );

        assert_eq!(
            result.args_replace_reason,
            Some(ToolArgsReplaceReason::IncomingAddsExecutionContext)
        );
        match &parts[1] {
            MessagePart::ToolInvocation { args, .. } => {
                assert_eq!(
                    args["path"],
                    ".tandem/runs/run-123/artifacts/research-sources.json"
                );
                assert_eq!(args["__workspace_root"], "/home/evan/marketing-tandem");
            }
            other => panic!("expected tool part, got {other:?}"),
        }
    }

    #[test]
    fn appends_non_tool_parts_without_reduction() {
        let mut parts = vec![text_part("hello")];

        let (result, parts) =
            reducer_result(&mut parts, MessagePart::Reasoning { text: "why".into() });

        assert_eq!(result.action, MessagePartReduceAction::AppendedNewPart);
        assert!(matches!(parts[1], MessagePart::Reasoning { .. }));
    }

    #[test]
    fn latest_unresolved_same_tool_part_is_the_merge_target() {
        let mut parts = vec![
            text_part("hello"),
            tool_part("write", json!({"path":"one"}), None, None),
            tool_part("write", json!({"path":"two"}), None, None),
        ];

        let (result, parts) = reducer_result(
            &mut parts,
            tool_part("write", json!({"path":"two","content":"draft"}), None, None),
        );

        assert_eq!(result.target_index, 2);
        match &parts[1] {
            MessagePart::ToolInvocation { args, .. } => assert_eq!(args["path"], "one"),
            other => panic!("expected tool part, got {other:?}"),
        }
        match &parts[2] {
            MessagePart::ToolInvocation { args, .. } => assert_eq!(args["content"], "draft"),
            other => panic!("expected tool part, got {other:?}"),
        }
    }

    #[test]
    fn does_not_merge_into_resolved_same_tool_parts() {
        let mut parts = vec![
            text_part("hello"),
            tool_part("write", json!({"path":"one"}), Some(json!("ok")), None),
        ];

        let (result, parts) = reducer_result(
            &mut parts,
            tool_part("write", json!({"path":"two"}), None, None),
        );

        assert_eq!(result.action, MessagePartReduceAction::AppendedNewPart);
        assert_eq!(parts.len(), 3);
    }
}
