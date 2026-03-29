// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::host::{PlannerLlmInvocation, PlannerLlmInvoker};
use crate::planner_types::PlannerInvocationFailure;
use crate::workflow_plan::truncate_text;

pub async fn invoke_planner_json<T, H>(
    host: &H,
    invocation: PlannerLlmInvocation,
) -> Result<T, PlannerInvocationFailure>
where
    T: DeserializeOwned,
    H: PlannerLlmInvoker,
{
    let payload = host.invoke_planner_llm(invocation).await?;
    parse_planner_json(payload)
}

pub fn parse_planner_json<T>(payload: Value) -> Result<T, PlannerInvocationFailure>
where
    T: DeserializeOwned,
{
    serde_json::from_value::<T>(payload).map_err(|error| PlannerInvocationFailure {
        reason: "invalid_json".to_string(),
        detail: Some(truncate_text(&error.to_string(), 500)),
    })
}
