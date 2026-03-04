use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct PermissionReplyInput {
    pub reply: String,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct QuestionReplyInput {
    #[serde(default)]
    pub _answers: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct QuestionAnswerInput {
    pub answer: Option<String>,
}

pub(super) async fn list_permissions(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "requests": state.permissions.list().await,
        "rules": state.permissions.list_rules().await
    }))
}

pub(super) async fn reply_permission(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<PermissionReplyInput>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let accepted = matches!(
        input.reply.as_str(),
        "once" | "always" | "reject" | "allow" | "deny"
    );
    if !accepted {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorEnvelope {
                error: "reply must be one of once|always|reject|allow|deny".to_string(),
                code: Some("invalid_permission_reply".to_string()),
            }),
        ));
    }
    let ok = state.permissions.reply(&id, &input.reply).await;
    if !ok {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorEnvelope {
                error: "Permission request not found".to_string(),
                code: Some("permission_request_not_found".to_string()),
            }),
        ));
    }
    Ok(Json(json!({
        "ok": true,
        "requestID": id,
        "reply": input.reply,
        "status": "applied",
        "persistedRule": matches!(input.reply.as_str(), "always" | "allow")
    })))
}

pub(super) async fn approve_tool_by_call(
    State(state): State<AppState>,
    Path((_session_id, tool_call_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ok = state.permissions.reply(&tool_call_id, "allow").await;
    if !ok {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorEnvelope {
                error: "Permission request not found".to_string(),
                code: Some("permission_request_not_found".to_string()),
            }),
        ));
    }
    Ok(Json(json!({"ok": true})))
}

pub(super) async fn deny_tool_by_call(
    State(state): State<AppState>,
    Path((_session_id, tool_call_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ok = state.permissions.reply(&tool_call_id, "deny").await;
    if !ok {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorEnvelope {
                error: "Permission request not found".to_string(),
                code: Some("permission_request_not_found".to_string()),
            }),
        ));
    }
    Ok(Json(json!({"ok": true})))
}

pub(super) async fn list_questions(State(state): State<AppState>) -> Json<Value> {
    Json(json!(state.storage.list_question_requests().await))
}

pub(super) async fn reply_question(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(_input): Json<QuestionReplyInput>,
) -> Result<Json<Value>, StatusCode> {
    let ok = state
        .storage
        .reply_question(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ok {
        state.event_bus.publish(EngineEvent::new(
            "question.replied",
            json!({"id": id, "ok": true}),
        ));
    }
    Ok(Json(json!({"ok": ok})))
}

pub(super) async fn reject_question(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let ok = state
        .storage
        .reject_question(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ok {
        state.event_bus.publish(EngineEvent::new(
            "question.replied",
            json!({"id": id, "ok": false}),
        ));
    }
    Ok(Json(json!({"ok": ok})))
}

pub(super) async fn answer_question(
    State(state): State<AppState>,
    Path((_session_id, question_id)): Path<(String, String)>,
    Json(input): Json<QuestionAnswerInput>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ok = state
        .storage
        .reply_question(&question_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorEnvelope {
                    error: "Failed to answer question".to_string(),
                    code: Some("question_answer_failed".to_string()),
                }),
            )
        })?;
    if !ok {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorEnvelope {
                error: "Question request not found".to_string(),
                code: Some("question_not_found".to_string()),
            }),
        ));
    }
    if ok {
        state.event_bus.publish(EngineEvent::new(
            "question.replied",
            json!({"id": question_id, "ok": true, "answer": input.answer}),
        ));
    }
    Ok(Json(json!({"ok": true})))
}
