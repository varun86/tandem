use super::{
    ChatMessage, ContentBlock, MessageRole, PlanFeedbackWizardState, QuestionDraft, Task,
    TaskStatus,
};

pub(super) fn question_drafts_from_permission_args(
    args: Option<&serde_json::Value>,
    fallback_query: Option<&str>,
) -> Vec<QuestionDraft> {
    let Some(raw_args) = args else {
        if let Some(query) = fallback_query.map(str::trim).filter(|q| !q.is_empty()) {
            return vec![QuestionDraft {
                header: "Question".to_string(),
                question: query.to_string(),
                options: Vec::new(),
                multiple: false,
                custom: true,
                selected_options: Vec::new(),
                custom_input: String::new(),
                option_cursor: 0,
            }];
        }
        return Vec::new();
    };

    let parsed_args;
    let args = if let Some(raw) = raw_args.as_str() {
        if let Ok(decoded) = serde_json::from_str::<serde_json::Value>(raw) {
            parsed_args = decoded;
            &parsed_args
        } else {
            raw_args
        }
    } else {
        raw_args
    };

    let parse_choice = |opt: &serde_json::Value| {
        if let Some(label) = opt.as_str() {
            return Some(crate::net::client::QuestionChoice {
                label: label.to_string(),
                description: String::new(),
            });
        }
        let label = opt
            .get("label")
            .or_else(|| opt.get("title"))
            .or_else(|| opt.get("name"))
            .or_else(|| opt.get("value"))
            .or_else(|| opt.get("text"))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_u64().map(|n| n.to_string()))
                }
            })?;
        let description = opt
            .get("description")
            .or_else(|| opt.get("hint"))
            .or_else(|| opt.get("subtitle"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Some(crate::net::client::QuestionChoice { label, description })
    };

    if let Some(items) = args.get("questions").and_then(|v| v.as_array()) {
        let parsed = items
            .iter()
            .filter_map(|item| {
                if let Some(question) = item.as_str() {
                    let text = question.trim();
                    if text.is_empty() {
                        return None;
                    }
                    return Some(QuestionDraft {
                        header: "Question".to_string(),
                        question: text.to_string(),
                        options: Vec::new(),
                        multiple: false,
                        custom: true,
                        selected_options: Vec::new(),
                        custom_input: String::new(),
                        option_cursor: 0,
                    });
                }
                let question = item
                    .get("question")
                    .or_else(|| item.get("prompt"))
                    .or_else(|| item.get("query"))
                    .or_else(|| item.get("text"))
                    .and_then(|v| v.as_str())?;
                let header = item
                    .get("header")
                    .or_else(|| item.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Question")
                    .to_string();
                let options = item
                    .get("options")
                    .or_else(|| item.get("choices"))
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(parse_choice).collect::<Vec<_>>())
                    .unwrap_or_default();
                let has_options = !options.is_empty();
                let multiple = item
                    .get("multiple")
                    .or_else(|| item.get("multi_select"))
                    .or_else(|| item.get("multiSelect"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let custom = item
                    .get("custom")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(!has_options);

                Some(QuestionDraft {
                    header,
                    question: question.to_string(),
                    options,
                    multiple,
                    custom,
                    selected_options: Vec::new(),
                    custom_input: String::new(),
                    option_cursor: 0,
                })
            })
            .collect::<Vec<_>>();
        if !parsed.is_empty() {
            return parsed;
        }
    }

    let options = args
        .get("options")
        .or_else(|| args.get("choices"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_choice).collect::<Vec<_>>())
        .unwrap_or_default();
    let has_options = !options.is_empty();
    let question = args
        .get("question")
        .or_else(|| args.get("prompt"))
        .or_else(|| args.get("text"))
        .or_else(|| args.get("title"))
        .or_else(|| args.get("query"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| fallback_query.map(str::trim).filter(|s| !s.is_empty()));
    if let Some(question) = question {
        return vec![QuestionDraft {
            header: args
                .get("header")
                .and_then(|v| v.as_str())
                .unwrap_or("Question")
                .to_string(),
            question: question.to_string(),
            options,
            multiple: args
                .get("multiple")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            custom: args
                .get("custom")
                .and_then(|v| v.as_bool())
                .unwrap_or(!has_options),
            selected_options: Vec::new(),
            custom_input: String::new(),
            option_cursor: 0,
        }];
    }
    Vec::new()
}

pub(super) fn is_task_tool_name(tool: &str) -> bool {
    matches!(
        canonical_tool_name(tool).as_str(),
        "task" | "todo_write" | "todowrite" | "update_todo_list" | "new_task"
    )
}

pub(super) fn is_todo_write_tool_name(tool: &str) -> bool {
    matches!(
        canonical_tool_name(tool).as_str(),
        "todo_write" | "todowrite" | "update_todo_list"
    )
}

pub(super) fn is_question_tool_name(tool: &str) -> bool {
    let canonical = canonical_tool_name(tool);
    canonical == "question"
        || canonical.starts_with("question_")
        || canonical.starts_with("question")
        || canonical.contains("question")
}

pub(super) fn task_payload_all_pending(args: Option<&serde_json::Value>) -> bool {
    let items = extract_task_payload_items(args);
    !items.is_empty()
        && items
            .iter()
            .all(|(_, status)| matches!(status, TaskStatus::Pending))
}

pub(super) fn apply_task_payload(
    tasks: &mut Vec<Task>,
    active_task_id: &mut Option<String>,
    tool: &str,
    args: Option<&serde_json::Value>,
) {
    let incoming = extract_task_payload_items(args);
    if incoming.is_empty() {
        return;
    }

    if is_todo_write_tool_name(tool) {
        let mut normalized: Vec<(String, TaskStatus)> = Vec::new();
        for (description, status) in incoming {
            if let Some(existing) = normalized
                .iter_mut()
                .find(|(d, _)| d.eq_ignore_ascii_case(description.as_str()))
            {
                existing.1 = status;
            } else {
                normalized.push((description, status));
            }
        }

        let pinned_by_description = tasks
            .iter()
            .map(|t| (t.description.to_ascii_lowercase(), t.pinned))
            .collect::<std::collections::HashMap<_, _>>();

        tasks.clear();
        for (idx, (description, status)) in normalized.into_iter().enumerate() {
            let pinned = pinned_by_description
                .get(&description.to_ascii_lowercase())
                .copied()
                .unwrap_or(false);
            tasks.push(Task {
                id: format!("task-{}", idx + 1),
                description,
                status,
                pinned,
            });
        }
    } else {
        for (description, status) in incoming {
            if let Some(existing) = tasks.iter_mut().find(|t| t.description == description) {
                existing.status = status.clone();
            } else {
                let id = format!("task-{}", tasks.len() + 1);
                tasks.push(Task {
                    id,
                    description,
                    status: status.clone(),
                    pinned: false,
                });
            }
        }
    }

    if let Some(working) = tasks
        .iter()
        .find(|t| matches!(t.status, TaskStatus::Working))
    {
        *active_task_id = Some(working.id.clone());
    } else {
        *active_task_id = None;
    }
}

pub(super) fn plan_fingerprint_from_args(args: Option<&serde_json::Value>) -> Vec<String> {
    let Some(args) = args else {
        return Vec::new();
    };
    let arrays = [
        args.get("todos").and_then(|v| v.as_array()),
        args.get("tasks").and_then(|v| v.as_array()),
        args.get("steps").and_then(|v| v.as_array()),
        args.get("items").and_then(|v| v.as_array()),
    ];

    let mut items: Vec<String> = Vec::new();
    for arr in arrays.into_iter().flatten() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                if let Some(content) = obj
                    .get("content")
                    .or_else(|| obj.get("description"))
                    .or_else(|| obj.get("title"))
                    .and_then(|v| v.as_str())
                {
                    let normalized = content.trim().to_lowercase();
                    if !normalized.is_empty() {
                        items.push(normalized);
                    }
                }
            }
        }
    }
    items.sort();
    items.dedup();
    items
}

pub(super) fn plan_preview_from_args(args: Option<&serde_json::Value>) -> Vec<String> {
    extract_task_payload_items(args)
        .into_iter()
        .map(|(content, _)| content)
        .take(10)
        .collect()
}

pub(super) fn build_plan_feedback_markdown(wizard: &PlanFeedbackWizardState) -> String {
    let plan_name = if wizard.plan_name.trim().is_empty() {
        "Current plan".to_string()
    } else {
        wizard.plan_name.trim().to_string()
    };
    let scope = if wizard.scope.trim().is_empty() {
        "Use the proposed tasks as the working scope.".to_string()
    } else {
        wizard.scope.trim().to_string()
    };
    let constraints = if wizard.constraints.trim().is_empty() {
        "No additional constraints.".to_string()
    } else {
        wizard.constraints.trim().to_string()
    };
    let priorities = if wizard.priorities.trim().is_empty() {
        "Follow logical dependency order.".to_string()
    } else {
        wizard.priorities.trim().to_string()
    };
    let notes = if wizard.notes.trim().is_empty() {
        "No additional notes.".to_string()
    } else {
        wizard.notes.trim().to_string()
    };

    let mut task_lines = String::new();
    if wizard.task_preview.is_empty() {
        task_lines.push_str("- Use the current todo list from `todowrite`.\n");
    } else {
        for (idx, task) in wizard.task_preview.iter().enumerate() {
            task_lines.push_str(&format!("{}. {}\n", idx + 1, task));
        }
    }

    format!(
        "## Plan Feedback\n\
         \n\
         **Plan:** {}\n\
         \n\
         ### Approved Task Draft\n\
         {}\n\
         ### Scope\n\
         {}\n\
         \n\
         ### Constraints\n\
         {}\n\
         \n\
         ### Priority Order\n\
         {}\n\
         \n\
         ### Additional Notes\n\
         {}\n\
         \n\
         ### Next Action\n\
         Revise the plan using this feedback, update `todowrite` with refined tasks, and then ask for approval before execution.",
        plan_name, task_lines, scope, constraints, priorities, notes
    )
}

pub(super) fn rebuild_tasks_from_messages(messages: &[ChatMessage]) -> (Vec<Task>, Option<String>) {
    let mut tasks = Vec::new();
    let mut active_task_id = None;

    for message in messages {
        for block in &message.content {
            let ContentBlock::ToolCall(tool_call) = block else {
                continue;
            };
            if !is_task_tool_name(&tool_call.name) {
                continue;
            }
            if let Ok(args) = serde_json::from_str::<serde_json::Value>(&tool_call.args) {
                apply_task_payload(
                    &mut tasks,
                    &mut active_task_id,
                    &tool_call.name,
                    Some(&args),
                );
            }
        }
    }

    (tasks, active_task_id)
}

pub(super) fn task_status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Working => "working",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
    }
}

pub(super) fn context_todo_items_from_tasks(
    tasks: &[Task],
) -> Vec<crate::net::client::ContextTodoSyncItem> {
    tasks
        .iter()
        .map(|task| crate::net::client::ContextTodoSyncItem {
            id: Some(task.id.clone()),
            content: task.description.clone(),
            status: Some(context_todo_status_label(&task.status).to_string()),
        })
        .collect::<Vec<_>>()
}

pub(super) fn latest_assistant_text(messages: &[ChatMessage]) -> Option<String> {
    for message in messages.iter().rev() {
        if !matches!(message.role, MessageRole::Assistant) {
            continue;
        }
        let mut chunks = Vec::new();
        for block in &message.content {
            match block {
                ContentBlock::Text(text) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        chunks.push(trimmed.to_string());
                    }
                }
                ContentBlock::Code { language, code } => {
                    let lang = language.trim();
                    if lang.is_empty() {
                        chunks.push(format!("```\n{}\n```", code));
                    } else {
                        chunks.push(format!("```{}\n{}\n```", lang, code));
                    }
                }
                ContentBlock::ToolCall(tool) => {
                    chunks.push(format!("Tool call: {} {}", tool.name, tool.args));
                }
                ContentBlock::ToolResult(result) => {
                    chunks.push(format!("Tool result: {}", result));
                }
            }
        }
        if !chunks.is_empty() {
            return Some(chunks.join("\n\n"));
        }
    }
    None
}

pub(super) fn plan_task_context_block(
    tasks: &[Task],
    active_task_id: Option<&str>,
) -> Option<String> {
    if tasks.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    lines.push(format!("Total tasks: {}", tasks.len()));
    if let Some(active_id) = active_task_id {
        lines.push(format!("Active task id: {}", active_id));
    }
    for task in tasks.iter().take(12) {
        let active_marker = if active_task_id == Some(task.id.as_str()) {
            ">"
        } else {
            "-"
        };
        lines.push(format!(
            "{} [{}] {}",
            active_marker,
            task_status_label(&task.status),
            task.description
        ));
    }
    if tasks.len() > 12 {
        lines.push(format!("... and {} more", tasks.len() - 12));
    }
    Some(lines.join("\n"))
}

fn canonical_tool_name(tool: &str) -> String {
    let last = tool
        .rsplit('.')
        .next()
        .unwrap_or(tool)
        .trim()
        .to_lowercase();
    last.replace('-', "_")
}

fn task_status_from_text(status: &str) -> TaskStatus {
    match status.to_ascii_lowercase().as_str() {
        "done" | "completed" | "complete" => TaskStatus::Done,
        "working" | "in_progress" | "in-progress" | "active" => TaskStatus::Working,
        "failed" | "error" | "blocked" => TaskStatus::Failed,
        _ => TaskStatus::Pending,
    }
}

fn extract_task_payload_items(args: Option<&serde_json::Value>) -> Vec<(String, TaskStatus)> {
    let Some(args) = args else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let arrays = [
        args.get("todos").and_then(|v| v.as_array()),
        args.get("tasks").and_then(|v| v.as_array()),
        args.get("steps").and_then(|v| v.as_array()),
        args.get("items").and_then(|v| v.as_array()),
    ];
    for arr in arrays.into_iter().flatten() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                let content = obj
                    .get("content")
                    .or_else(|| obj.get("description"))
                    .or_else(|| obj.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();
                if content.is_empty() {
                    continue;
                }
                let status_text = obj
                    .get("status")
                    .or_else(|| obj.get("state"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("pending");
                out.push((content.to_string(), task_status_from_text(status_text)));
            }
        }
    }
    out
}

fn context_todo_status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Working => "in_progress",
        TaskStatus::Done => "completed",
        TaskStatus::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_drafts_parse_structured_questions() {
        let payload = serde_json::json!({
            "questions": [
                {
                    "header": "Pick",
                    "question": "Choose a lane",
                    "options": [
                        { "label": "A", "description": "First" },
                        { "label": "B", "description": "Second" }
                    ],
                    "multiple": true
                }
            ]
        });

        let drafts = question_drafts_from_permission_args(Some(&payload), None);
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].header, "Pick");
        assert_eq!(drafts[0].question, "Choose a lane");
        assert_eq!(drafts[0].options.len(), 2);
        assert!(drafts[0].multiple);
        assert!(!drafts[0].custom);
    }

    #[test]
    fn task_payload_helpers_apply_and_rebuild_tasks() {
        let payload = serde_json::json!({
            "todos": [
                { "content": "Inspect logs", "status": "pending" },
                { "content": "Verify rollback", "status": "in_progress" }
            ]
        });

        let mut tasks = vec![Task {
            id: "task-1".to_string(),
            description: "Inspect logs".to_string(),
            status: TaskStatus::Pending,
            pinned: true,
        }];
        let mut active_task_id = None;
        apply_task_payload(&mut tasks, &mut active_task_id, "todowrite", Some(&payload));

        assert_eq!(tasks.len(), 2);
        assert!(tasks[0].pinned);
        assert_eq!(active_task_id.as_deref(), Some("task-2"));

        let messages = vec![ChatMessage {
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolCall(crate::app::ToolCallInfo {
                id: "tool-1".to_string(),
                name: "todowrite".to_string(),
                args: payload.to_string(),
            })],
        }];
        let (rebuilt, rebuilt_active) = rebuild_tasks_from_messages(&messages);
        assert_eq!(rebuilt.len(), 2);
        assert_eq!(rebuilt_active.as_deref(), Some("task-2"));
    }

    #[test]
    fn plan_context_block_and_feedback_markdown_render_expected_text() {
        let tasks = vec![
            Task {
                id: "task-1".to_string(),
                description: "Inspect logs".to_string(),
                status: TaskStatus::Pending,
                pinned: false,
            },
            Task {
                id: "task-2".to_string(),
                description: "Verify rollback".to_string(),
                status: TaskStatus::Working,
                pinned: false,
            },
        ];
        let context = plan_task_context_block(&tasks, Some("task-2")).expect("context");
        assert!(context.contains("Total tasks: 2"));
        assert!(context.contains("Active task id: task-2"));
        assert!(context.contains("> [working] Verify rollback"));

        let markdown = build_plan_feedback_markdown(&PlanFeedbackWizardState {
            plan_name: "Rollback plan".to_string(),
            scope: "Stay inside tandem-tui".to_string(),
            constraints: "No API changes".to_string(),
            priorities: "Verify first".to_string(),
            notes: "Keep UX stable".to_string(),
            cursor_step: 0,
            source_request_id: None,
            task_preview: vec!["Inspect logs".to_string(), "Verify rollback".to_string()],
        });
        assert!(markdown.contains("## Plan Feedback"));
        assert!(markdown.contains("Rollback plan"));
        assert!(markdown.contains("1. Inspect logs"));
        assert!(markdown.contains("No API changes"));
    }
}
