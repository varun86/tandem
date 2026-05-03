import { Badge, PanelCard } from "../../ui/index.tsx";
import { PlannerSubsection } from "./plannerPrimitives";

function safeString(value: unknown) {
  return String(value || "").trim();
}

function rows(value: unknown): string[] {
  return Array.isArray(value) ? value.map((entry) => safeString(entry)).filter(Boolean) : [];
}

export function PlannerDiagnosticsPanel({
  plannerDiagnostics,
  teachingLibrary,
  planningChangeSummary,
}: {
  plannerDiagnostics: any;
  teachingLibrary: any;
  planningChangeSummary: string[];
}) {
  const explanations = rows(teachingLibrary?.explanations || teachingLibrary?.explanation || []);
  const objections = rows(teachingLibrary?.objections || []);
  const proofPoints = rows(teachingLibrary?.proof_points || teachingLibrary?.proofPoints || []);
  const fallbackReason = safeString(
    plannerDiagnostics?.fallback_reason || plannerDiagnostics?.fallbackReason || ""
  );
  const summary = safeString(plannerDiagnostics?.summary || plannerDiagnostics?.detail || "");
  const taskBudget =
    plannerDiagnostics?.task_budget ||
    plannerDiagnostics?.taskBudget ||
    plannerDiagnostics?.decomposition_observation?.task_budget ||
    plannerDiagnostics?.decompositionObservation?.taskBudget ||
    plannerDiagnostics?.observation?.task_budget ||
    plannerDiagnostics?.observation?.taskBudget ||
    null;
  const taskBudgetStatus = safeString(taskBudget?.status || "");
  const originalStepCount = Number(
    taskBudget?.original_step_count ?? taskBudget?.originalStepCount
  );
  const generatedStepCount = Number(
    taskBudget?.generated_step_count ?? taskBudget?.generatedStepCount
  );
  const maxGeneratedSteps = Number(
    taskBudget?.max_generated_steps ?? taskBudget?.maxGeneratedSteps
  );
  const taskBudgetMessage =
    taskBudgetStatus === "compacted" &&
    Number.isFinite(originalStepCount) &&
    Number.isFinite(generatedStepCount)
      ? `Planner compacted ${originalStepCount} generated tasks into ${generatedStepCount} runnable workflow steps.`
      : taskBudgetStatus === "rejected" &&
          Number.isFinite(generatedStepCount) &&
          Number.isFinite(maxGeneratedSteps)
        ? `Generated plan has ${generatedStepCount} tasks, above the ${maxGeneratedSteps} task budget.`
        : taskBudgetStatus === "within_budget" &&
            Number.isFinite(generatedStepCount) &&
            Number.isFinite(maxGeneratedSteps)
          ? `Generated plan is within budget: ${generatedStepCount}/${maxGeneratedSteps} tasks.`
          : "";

  return (
    <PanelCard title="Diagnostics" subtitle="Planner guidance, change summary, and compiler notes.">
      <div className="grid gap-3 text-sm">
        <div className="flex flex-wrap gap-2">
          <Badge tone={plannerDiagnostics ? "ok" : "warn"}>
            {plannerDiagnostics ? "diagnostics ready" : "diagnostics pending"}
          </Badge>
          <Badge tone={explanations.length ? "ok" : "warn"}>
            {explanations.length ? "teaching library ready" : "teaching library pending"}
          </Badge>
          {fallbackReason ? <Badge tone="info">{fallbackReason.replace(/_/g, " ")}</Badge> : null}
          {taskBudgetStatus ? (
            <Badge tone={taskBudgetStatus === "rejected" ? "warn" : "ok"}>
              task budget {taskBudgetStatus.replace(/_/g, " ")}
            </Badge>
          ) : null}
        </div>

        <PlannerSubsection title="Planner diagnostics">
          <div className="text-slate-200">{summary || "No planner diagnostics yet."}</div>
          {taskBudgetMessage ? (
            <div className="mt-2 rounded-lg border border-lime-400/30 bg-lime-400/10 p-2 text-lime-100">
              {taskBudgetMessage}
            </div>
          ) : null}
        </PlannerSubsection>

        {planningChangeSummary.length ? (
          <PlannerSubsection title="Latest changes">
            <ul className="mt-2 grid gap-2 text-slate-200">
              {planningChangeSummary.map((entry) => (
                <li key={entry} className="rounded-lg border border-white/10 bg-black/20 p-2">
                  {entry}
                </li>
              ))}
            </ul>
          </PlannerSubsection>
        ) : null}

        {explanations.length || objections.length || proofPoints.length ? (
          <div className="grid gap-3 lg:grid-cols-3">
            <PlannerSubsection title="Explanations">
              <ul className="mt-2 grid gap-2 text-slate-200">
                {explanations.length ? (
                  explanations.map((entry) => (
                    <li key={entry} className="rounded-lg border border-white/10 bg-black/20 p-2">
                      {entry}
                    </li>
                  ))
                ) : (
                  <li className="tcp-subtle text-xs">No explanation guidance yet.</li>
                )}
              </ul>
            </PlannerSubsection>
            <PlannerSubsection title="Objections">
              <ul className="mt-2 grid gap-2 text-slate-200">
                {objections.length ? (
                  objections.map((entry) => (
                    <li key={entry} className="rounded-lg border border-white/10 bg-black/20 p-2">
                      {entry}
                    </li>
                  ))
                ) : (
                  <li className="tcp-subtle text-xs">No objection guidance yet.</li>
                )}
              </ul>
            </PlannerSubsection>
            <PlannerSubsection title="Proof points">
              <ul className="mt-2 grid gap-2 text-slate-200">
                {proofPoints.length ? (
                  proofPoints.map((entry) => (
                    <li key={entry} className="rounded-lg border border-white/10 bg-black/20 p-2">
                      {entry}
                    </li>
                  ))
                ) : (
                  <li className="tcp-subtle text-xs">No proof-point guidance yet.</li>
                )}
              </ul>
            </PlannerSubsection>
          </div>
        ) : null}
      </div>
    </PanelCard>
  );
}
