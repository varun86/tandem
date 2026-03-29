// Copyright (c) 2026 Frumu LTD
// Licensed under the Business Source License 1.1

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerClarifierOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerClarifier {
    #[serde(default)]
    pub field: Option<String>,
    pub question: String,
    #[serde(default)]
    pub options: Vec<PlannerClarifierOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerInvocationFailure {
    pub reason: String,
    #[serde(default)]
    pub detail: Option<String>,
}
