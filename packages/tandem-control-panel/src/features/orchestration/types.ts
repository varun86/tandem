export type TaskState =
  | "created"
  | "pending"
  | "runnable"
  | "assigned"
  | "in_progress"
  | "blocked"
  | "done"
  | "failed"
  | "validated";

export interface OrchestrationTask {
  id: string;
  title: string;
  description?: string;
  dependencies: string[];
  state: TaskState;
  retry_count: number;
  error_message?: string;
  runtime_status?: string;
  runtime_detail?: string;
  assigned_role?: string;
  gate?: string;
  workflow_id?: string;
  session_id?: string;
}

export interface BudgetUsage {
  max_iterations: number;
  iterations_used: number;
  max_tokens: number;
  tokens_used: number;
  max_wall_time_secs: number;
  wall_time_secs: number;
  max_subagent_runs: number;
  subagent_runs_used: number;
  exceeded: boolean;
  exceeded_reason?: string;
  limits_enforced?: boolean;
  source?: "run" | "derived";
}
