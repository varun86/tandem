import { useState } from "preact/hooks";
import { motion } from "motion/react";
import { ProviderModelSelector } from "../../components/ProviderModelSelector";
import { ScheduleBuilder } from "./ScheduleBuilder";
import { ScopeInspector } from "./ScopeInspector";
import { WatchConditionEditor } from "./WatchConditionEditor";
import { ScopePolicyEditor } from "./ScopePolicyEditor";
import { HandoffConfigEditor } from "./HandoffConfigEditor";
import { HandoffPanel } from "./HandoffPanel";

export function LegacyAutomationEditDialog({
  editDraft,
  setEditDraft,
  updateAutomationMutation,
}: any) {
  if (!editDraft) return null;

  return (
    <motion.div
      className="tcp-confirm-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={() => setEditDraft(null)}
    >
      <motion.div
        className="tcp-confirm-dialog w-[min(40rem,96vw)]"
        initial={{ opacity: 0, y: 8, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: 6, scale: 0.98 }}
        onClick={(event) => event.stopPropagation()}
      >
        <h3 className="tcp-confirm-title">Edit automation</h3>
        <div className="grid gap-3">
          <div className="grid gap-1">
            <label className="text-xs text-slate-400">Name</label>
            <input
              className="tcp-input"
              value={editDraft.name}
              onInput={(e) =>
                setEditDraft((current: any) =>
                  current ? { ...current, name: (e.target as HTMLInputElement).value } : current
                )
              }
            />
          </div>
          <div className="grid gap-1">
            <label className="text-xs text-slate-400">Objective</label>
            <textarea
              className="tcp-input min-h-[96px]"
              value={editDraft.objective}
              onInput={(e) =>
                setEditDraft((current: any) =>
                  current
                    ? { ...current, objective: (e.target as HTMLTextAreaElement).value }
                    : current
                )
              }
            />
          </div>
          <div className="grid gap-1 sm:grid-cols-2 sm:gap-2">
            <div className="grid gap-1">
              <label className="text-xs text-slate-400">Mode</label>
              <select
                className="tcp-input"
                value={editDraft.mode}
                onInput={(e) =>
                  setEditDraft((current: any) =>
                    current
                      ? {
                          ...current,
                          mode: (e.target as HTMLSelectElement).value as
                            | "standalone"
                            | "orchestrated",
                        }
                      : current
                  )
                }
              >
                <option value="standalone">standalone</option>
                <option value="orchestrated">orchestrated</option>
              </select>
            </div>
            <div className="grid gap-1">
              <label className="text-xs text-slate-400">Approval policy</label>
              <button
                className={`tcp-input flex h-10 items-center justify-between px-3 text-xs ${
                  editDraft.requiresApproval ? "border-amber-400/60 bg-amber-400/10" : ""
                }`}
                role="switch"
                aria-checked={editDraft.requiresApproval}
                onClick={() =>
                  setEditDraft((current: any) =>
                    current ? { ...current, requiresApproval: !current.requiresApproval } : current
                  )
                }
              >
                <span className="flex items-center gap-2">
                  <i data-lucide={editDraft.requiresApproval ? "shield-alert" : "shield-check"}></i>
                  {editDraft.requiresApproval
                    ? "Manual approvals enabled"
                    : "Fully automated enabled"}
                </span>
                <span
                  className={`relative h-5 w-9 rounded-full transition ${
                    editDraft.requiresApproval ? "bg-amber-500/40" : "bg-emerald-500/30"
                  }`}
                >
                  <span
                    className={`absolute left-0.5 top-0.5 h-4 w-4 rounded-full bg-slate-100 transition ${
                      editDraft.requiresApproval ? "" : "translate-x-4"
                    }`}
                  />
                </span>
              </button>
            </div>
          </div>
          <div className="grid gap-1 sm:grid-cols-2 sm:gap-2">
            <div className="grid gap-1">
              <label className="text-xs text-slate-400">Schedule type</label>
              <select
                className="tcp-input"
                value={editDraft.scheduleKind}
                onInput={(e) =>
                  setEditDraft((current: any) =>
                    current
                      ? {
                          ...current,
                          scheduleKind: (e.target as HTMLSelectElement).value as
                            | "cron"
                            | "interval",
                        }
                      : current
                  )
                }
              >
                <option value="interval">interval</option>
                <option value="cron">cron</option>
              </select>
            </div>
          </div>
          {editDraft.scheduleKind === "cron" ? (
            <div className="grid gap-1">
              <label className="text-xs text-slate-400">Cron expression</label>
              <input
                className="tcp-input font-mono"
                value={editDraft.cronExpression}
                onInput={(e) =>
                  setEditDraft((current: any) =>
                    current
                      ? { ...current, cronExpression: (e.target as HTMLInputElement).value }
                      : current
                  )
                }
                placeholder="0 9 * * *"
              />
            </div>
          ) : (
            <div className="grid gap-1">
              <label className="text-xs text-slate-400">Interval seconds</label>
              <input
                type="number"
                min="1"
                className="tcp-input"
                value={editDraft.intervalSeconds}
                onInput={(e) =>
                  setEditDraft((current: any) =>
                    current
                      ? { ...current, intervalSeconds: (e.target as HTMLInputElement).value }
                      : current
                  )
                }
              />
            </div>
          )}
        </div>
        <div className="tcp-confirm-actions mt-3">
          <button className="tcp-btn" onClick={() => setEditDraft(null)}>
            <i data-lucide="x-circle"></i>
            Cancel
          </button>
          <button
            className="tcp-btn-primary"
            onClick={() => editDraft && updateAutomationMutation.mutate(editDraft)}
            disabled={updateAutomationMutation.isPending}
          >
            <i data-lucide="check"></i>
            Save
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

export function AccordionSection({
  title,
  defaultOpen = false,
  children,
  description = "",
  headerStyle = "",
  icon = "",
}: {
  title: string;
  defaultOpen?: boolean;
  children: any;
  description?: string;
  headerStyle?: string;
  icon?: string;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div
      className={`rounded-xl border border-slate-700/50 bg-slate-900/30 overflow-hidden ${headerStyle === "violet" ? "border-violet-500/20 bg-violet-900/5" : ""}`}
    >
      <button
        type="button"
        className="w-full flex items-center justify-between px-4 py-3 text-left hover:bg-slate-800/40 transition-colors focus:outline-none"
        onClick={() => setOpen(!open)}
      >
        <div className="flex items-center gap-2">
          {icon && (
            <i
              data-lucide={icon}
              className={`h-4 w-4 ${headerStyle === "violet" ? "text-violet-400" : "text-slate-400"}`}
            ></i>
          )}
          <div>
            <div
              className={`text-xs uppercase tracking-wide font-medium ${headerStyle === "violet" ? "text-violet-300" : "text-slate-500"}`}
            >
              {title}
            </div>
            {description && open && (
              <div
                className={`text-xs mt-0.5 ${headerStyle === "violet" ? "text-slate-500" : "text-slate-400"}`}
              >
                {description}
              </div>
            )}
          </div>
        </div>
        <i
          data-lucide={open ? "chevron-up" : "chevron-down"}
          className="h-4 w-4 text-slate-500 shrink-0 ml-4"
        ></i>
      </button>
      {open && (
        <div
          className={`px-4 pb-4 border-t ${headerStyle === "violet" ? "border-violet-900/40" : "border-slate-800/60"} pt-3`}
        >
          <div className="grid gap-3">{children}</div>
        </div>
      )}
    </div>
  );
}

export function WorkflowAutomationEditDialog({
  workflowEditDraft,
  setWorkflowEditDraft,
  validateWorkspaceRootInput,
  validateModelInput,
  validatePlannerModelInput,
  automationWizardConfig,
  providerOptions,
  mcpServers,
  overlapHistoryEntries,
  runNowV2Mutation,
  updateWorkflowAutomationMutation,
  automationsV2List = [],
  client,
}: any) {
  if (!workflowEditDraft) return null;

  return (
    <motion.div
      className="tcp-confirm-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={() => setWorkflowEditDraft(null)}
    >
      <motion.div
        className="tcp-confirm-dialog tcp-workflow-editor-modal"
        initial={{ opacity: 0, y: 8, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: 6, scale: 0.98 }}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-3 border-b border-slate-800/70 px-4 py-4">
          <div>
            <h3 className="tcp-confirm-title">Edit workflow automation</h3>
            <div className="mt-1 text-sm text-slate-400">
              Update scheduling, model routing, MCP access, and the actual step prompts.
            </div>
          </div>
          <button className="tcp-btn h-9 w-9 px-0" onClick={() => setWorkflowEditDraft(null)}>
            <i data-lucide="x"></i>
          </button>
        </div>
        <div className="grid flex-1 gap-4 overflow-y-auto px-4 py-4 xl:grid-cols-[minmax(22rem,0.92fr)_minmax(0,1.35fr)]">
          <div className="grid content-start gap-4 min-w-0">
            <AccordionSection title="General setup" defaultOpen={true}>
              <div className="grid gap-1">
                <label className="text-xs text-slate-400">Automation name</label>
                <input
                  className="tcp-input"
                  value={workflowEditDraft.name}
                  onInput={(e) =>
                    setWorkflowEditDraft((current: any) =>
                      current ? { ...current, name: (e.target as HTMLInputElement).value } : current
                    )
                  }
                />
              </div>
              <div className="grid gap-1">
                <label className="text-xs text-slate-400">Notes / description</label>
                <textarea
                  className="tcp-input min-h-[120px]"
                  value={workflowEditDraft.description}
                  onInput={(e) =>
                    setWorkflowEditDraft((current: any) =>
                      current
                        ? {
                            ...current,
                            description: (e.target as HTMLTextAreaElement).value,
                          }
                        : current
                    )
                  }
                  placeholder="Add notes, delivery expectations, or operator guidance."
                />
              </div>
              <div className="grid gap-1">
                <label className="text-xs text-slate-400">Workspace root</label>
                <input
                  className={`tcp-input ${
                    validateWorkspaceRootInput(workflowEditDraft.workspaceRoot)
                      ? "border-red-500/70 text-red-100"
                      : ""
                  }`}
                  value={workflowEditDraft.workspaceRoot}
                  onInput={(e) =>
                    setWorkflowEditDraft((current: any) =>
                      current
                        ? { ...current, workspaceRoot: (e.target as HTMLInputElement).value }
                        : current
                    )
                  }
                />
                {validateWorkspaceRootInput(workflowEditDraft.workspaceRoot) ? (
                  <div className="text-xs text-red-300">
                    {validateWorkspaceRootInput(workflowEditDraft.workspaceRoot)}
                  </div>
                ) : null}
              </div>
            </AccordionSection>

            <AccordionSection title="Execution" defaultOpen={true}>
              <div className="grid gap-3">
                <div className="grid gap-1">
                  <label className="text-xs text-slate-400">Schedule</label>
                  <ScheduleBuilder
                    value={{
                      scheduleKind: workflowEditDraft.scheduleKind,
                      cronExpression: workflowEditDraft.cronExpression,
                      intervalSeconds: workflowEditDraft.intervalSeconds,
                    }}
                    onChange={(value) =>
                      setWorkflowEditDraft((current: any) =>
                        current
                          ? {
                              ...current,
                              scheduleKind: value.scheduleKind,
                              cronExpression: value.cronExpression,
                              intervalSeconds: value.intervalSeconds,
                            }
                          : current
                      )
                    }
                  />
                </div>
              </div>
              <div className="grid gap-2 sm:grid-cols-2">
                <div className="grid gap-1">
                  <label className="text-xs text-slate-400">Execution mode</label>
                  <select
                    className="tcp-select"
                    value={workflowEditDraft.executionMode}
                    onInput={(e) =>
                      setWorkflowEditDraft((current: any) =>
                        current
                          ? {
                              ...current,
                              executionMode: (e.target as HTMLSelectElement).value as any,
                            }
                          : current
                      )
                    }
                  >
                    {automationWizardConfig.executionModes.map((mode: any) => (
                      <option key={mode.id} value={mode.id}>
                        {mode.label}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="grid gap-1">
                  <label className="text-xs text-slate-400">Max parallel agents</label>
                  <input
                    type="number"
                    min="1"
                    max="16"
                    className="tcp-input"
                    value={workflowEditDraft.maxParallelAgents}
                    onInput={(e) =>
                      setWorkflowEditDraft((current: any) =>
                        current
                          ? {
                              ...current,
                              maxParallelAgents: (e.target as HTMLInputElement).value,
                            }
                          : current
                      )
                    }
                    disabled={workflowEditDraft.executionMode !== "swarm"}
                  />
                </div>
              </div>
            </AccordionSection>

            <div id="workflow-model-selection">
              <AccordionSection title="Model Selection">
                <ProviderModelSelector
                  providerLabel="Model provider"
                  modelLabel="Model"
                  draft={{
                    provider: workflowEditDraft.modelProvider,
                    model: workflowEditDraft.modelId,
                  }}
                  providers={providerOptions}
                  onChange={(draft) =>
                    setWorkflowEditDraft((current: any) =>
                      current
                        ? {
                            ...current,
                            modelProvider: draft.provider,
                            modelId: draft.model,
                          }
                        : current
                    )
                  }
                  inheritLabel="Workspace default"
                />
                {validateModelInput(workflowEditDraft.modelProvider, workflowEditDraft.modelId) ? (
                  <div className="text-xs text-red-300">
                    {validateModelInput(workflowEditDraft.modelProvider, workflowEditDraft.modelId)}
                  </div>
                ) : null}
                <div className="grid gap-2 rounded-lg border border-slate-800/70 bg-slate-950/30 p-3">
                  <div className="text-xs uppercase tracking-wide text-slate-500">
                    Planner fallback model
                  </div>
                  <div className="text-xs text-slate-400">
                    Optional. Leave blank to use the workflow default model for planning and
                    revisions.
                  </div>
                  <ProviderModelSelector
                    providerLabel="Planner provider"
                    modelLabel="Planner model"
                    draft={{
                      provider: workflowEditDraft.plannerModelProvider,
                      model: workflowEditDraft.plannerModelId,
                    }}
                    providers={providerOptions}
                    onChange={(draft) =>
                      setWorkflowEditDraft((current: any) =>
                        current
                          ? {
                              ...current,
                              plannerModelProvider: draft.provider,
                              plannerModelId: draft.model,
                            }
                          : current
                      )
                    }
                    inheritLabel="Use workflow model"
                  />
                  {validatePlannerModelInput(
                    workflowEditDraft.plannerModelProvider,
                    workflowEditDraft.plannerModelId
                  ) ? (
                    <div className="text-xs text-red-300">
                      {validatePlannerModelInput(
                        workflowEditDraft.plannerModelProvider,
                        workflowEditDraft.plannerModelId
                      )}
                    </div>
                  ) : null}
                </div>
              </AccordionSection>
            </div>

            <AccordionSection title="Tool Access">
              <div className="grid gap-2 sm:grid-cols-2">
                <button
                  type="button"
                  className={`tcp-list-item text-left ${workflowEditDraft.toolAccessMode === "all" ? "border-amber-400/60 bg-amber-400/10" : ""}`}
                  onClick={() =>
                    setWorkflowEditDraft((current: any) =>
                      current ? { ...current, toolAccessMode: "all" } : current
                    )
                  }
                >
                  <div className="font-medium">All tools</div>
                  <div className="tcp-subtle text-xs">
                    Grant full built-in tool access to workflow agents.
                  </div>
                </button>
                <button
                  type="button"
                  className={`tcp-list-item text-left ${workflowEditDraft.toolAccessMode === "custom" ? "border-amber-400/60 bg-amber-400/10" : ""}`}
                  onClick={() =>
                    setWorkflowEditDraft((current: any) =>
                      current ? { ...current, toolAccessMode: "custom" } : current
                    )
                  }
                >
                  <div className="font-medium">Custom allowlist</div>
                  <div className="tcp-subtle text-xs">
                    Restrict built-in tools manually. MCP tools still follow the selected servers.
                  </div>
                </button>
              </div>
              {workflowEditDraft.toolAccessMode === "custom" ? (
                <div className="grid gap-1">
                  <label className="text-xs text-slate-400">Allowed built-in tools</label>
                  <textarea
                    className="tcp-input min-h-[96px] font-mono text-xs"
                    value={workflowEditDraft.customToolsText}
                    onInput={(e) =>
                      setWorkflowEditDraft((current: any) =>
                        current
                          ? {
                              ...current,
                              customToolsText: (e.target as HTMLTextAreaElement).value,
                            }
                          : current
                      )
                    }
                    placeholder={`read\nwrite\nedit\nbash\nls\nglob\nwebsearch`}
                  />
                </div>
              ) : (
                <div className="text-xs text-slate-500">
                  All built-in tools are allowed for this automation.
                </div>
              )}
            </AccordionSection>

            <div id="workflow-connector-bindings">
              <AccordionSection
                title="Connector bindings"
                description="Edit the connector binding snapshot that the scope inspector reads back. Save will persist the new binding set into the automation metadata. Each binding must include an explicit status (mapped, unresolved_required, or unresolved_optional)."
              >
                <textarea
                  className="tcp-input min-h-[220px] font-mono text-xs leading-5"
                  value={workflowEditDraft.connectorBindingsJson}
                  onInput={(e) =>
                    setWorkflowEditDraft((current: any) =>
                      current
                        ? {
                            ...current,
                            connectorBindingsJson: (e.target as HTMLTextAreaElement).value,
                          }
                        : current
                    )
                  }
                  placeholder={`[\n  {\n    "capability": "github",\n    "binding_type": "oauth",\n    "binding_id": "github-primary",\n    "allowlist_pattern": "github.com/*",\n    "status": "mapped"\n  },\n  {\n    "capability": "slack",\n    "binding_type": null,\n    "binding_id": null,\n    "allowlist_pattern": null,\n    "status": "unresolved_required"\n  }\n]`}
                />
                <div className="text-xs text-slate-500">
                  Keep this as a JSON array of binding objects with capability, binding_type,
                  binding_id, allowlist_pattern, and an explicit status: mapped,
                  unresolved_required, or unresolved_optional.
                </div>
              </AccordionSection>
            </div>

            <AccordionSection
              title="Shared workflow context"
              description="Bind approved shared workflow contexts here, one context id per line. The ids are validated against this workflow's workspace and kept on the saved automation metadata so later runs can reuse the same approved context."
            >
              <textarea
                className="tcp-input min-h-[120px] font-mono text-xs leading-5"
                value={workflowEditDraft.sharedContextPackIdsText}
                onInput={(e) =>
                  setWorkflowEditDraft((current: any) =>
                    current
                      ? {
                          ...current,
                          sharedContextPackIdsText: (e.target as HTMLTextAreaElement).value,
                        }
                      : current
                  )
                }
                placeholder={`context-context-123\ncontext-context-456`}
              />
              <div className="text-xs text-slate-500">
                Use the copy context id button in the Shared workflow context panel to paste ids
                quickly.
              </div>
            </AccordionSection>

            <AccordionSection title="MCP Servers">
              {mcpServers.length ? (
                <div className="flex flex-wrap gap-2">
                  {mcpServers.map((server: any) => {
                    const isSelected = workflowEditDraft.selectedMcpServers.includes(server.name);
                    return (
                      <button
                        key={server.name}
                        className={`tcp-btn h-7 px-2 text-xs ${
                          isSelected ? "border-amber-400/60 bg-amber-400/10 text-amber-300" : ""
                        }`}
                        onClick={() =>
                          setWorkflowEditDraft((current: any) =>
                            current
                              ? {
                                  ...current,
                                  selectedMcpServers: isSelected
                                    ? current.selectedMcpServers.filter(
                                        (name: string) => name !== server.name
                                      )
                                    : [...current.selectedMcpServers, server.name].sort(),
                                }
                              : current
                          )
                        }
                      >
                        {server.name} {server.connected ? "• connected" : "• disconnected"}
                      </button>
                    );
                  })}
                </div>
              ) : (
                <div className="text-xs text-slate-400">No MCP servers configured yet.</div>
              )}
            </AccordionSection>

            {/* ─── Connected Agent Handoffs ─────────────────────────────── */}
            <AccordionSection
              title="Connected agents"
              icon="network"
              headerStyle="violet"
              description="Handoff configuration, watch conditions, and scope policy for agent-to-agent messaging."
            >
              <HandoffConfigEditor
                value={workflowEditDraft.handoffConfig}
                onChange={(next: any) =>
                  setWorkflowEditDraft((current: any) =>
                    current ? { ...current, handoffConfig: next } : current
                  )
                }
              />

              <WatchConditionEditor
                value={workflowEditDraft.watchConditions ?? []}
                automations={automationsV2List}
                onChange={(next: any) =>
                  setWorkflowEditDraft((current: any) =>
                    current ? { ...current, watchConditions: next } : current
                  )
                }
              />

              <ScopePolicyEditor
                value={workflowEditDraft.scopePolicy}
                onChange={(next: any) =>
                  setWorkflowEditDraft((current: any) =>
                    current ? { ...current, scopePolicy: next } : current
                  )
                }
              />

              {workflowEditDraft.automationId && client?.automationsV2?.listHandoffs && (
                <HandoffPanel automationId={workflowEditDraft.automationId} client={client} />
              )}
            </AccordionSection>

            <AccordionSection title="Scope Inspector">
              <ScopeInspector
                title=""
                planPackage={workflowEditDraft.scopeSnapshot}
                planPackageBundle={workflowEditDraft.planPackageBundle}
                planPackageReplay={workflowEditDraft.planPackageReplay}
                validationReport={workflowEditDraft.scopeValidation}
                runtimeContext={workflowEditDraft.runtimeContext}
                approvedPlanMaterialization={workflowEditDraft.approvedPlanMaterialization}
                overlapHistoryEntries={overlapHistoryEntries}
                onOpenPromptEditor={() => {
                  document
                    .getElementById("workflow-prompt-editor")
                    ?.scrollIntoView({ behavior: "smooth", block: "start" });
                }}
                onOpenModelRoutingEditor={() => {
                  document
                    .getElementById("workflow-model-selection")
                    ?.scrollIntoView({ behavior: "smooth", block: "start" });
                }}
                onOpenConnectorBindingsEditor={() => {
                  document
                    .getElementById("workflow-connector-bindings")
                    ?.scrollIntoView({ behavior: "smooth", block: "start" });
                }}
                onReplaceSharedContextPack={(fromPackId: string, toPackId: string) => {
                  if (!fromPackId || !toPackId) return;
                  setWorkflowEditDraft((current: any) =>
                    current
                      ? {
                          ...current,
                          sharedContextPackIdsText: String(current.sharedContextPackIdsText || "")
                            .split(/[\n,]/g)
                            .map((value: string) => String(value || "").trim())
                            .filter(Boolean)
                            .map((value: string) => (value === fromPackId ? toPackId : value))
                            .filter(
                              (value: string, index: number, values: string[]) =>
                                values.indexOf(value) === index
                            )
                            .join("\n"),
                        }
                      : current
                  );
                }}
                onDryRun={
                  workflowEditDraft.automationId
                    ? () =>
                        runNowV2Mutation.mutate({
                          id: workflowEditDraft.automationId,
                          dryRun: true,
                        })
                    : undefined
                }
                dryRunDisabled={!workflowEditDraft.automationId || runNowV2Mutation.isPending}
              />
            </AccordionSection>
          </div>

          <div className="grid content-start gap-4 min-w-0">
            <div id="workflow-prompt-editor">
              <AccordionSection
                title="Prompt Editor"
                description="Edit the actual prompts Tandem sends for each workflow step. These objectives control what every node does at runtime."
                defaultOpen={true}
              >
                {workflowEditDraft.nodes.length ? (
                  <div className="grid gap-3">
                    {workflowEditDraft.nodes.map((node: any, index: number) => (
                      <div
                        key={node.nodeId || index}
                        className="rounded-lg border border-slate-700/60 bg-slate-950/30 p-3"
                      >
                        <div className="mb-2 flex flex-wrap items-center gap-2">
                          <strong className="text-sm text-slate-100">
                            {node.nodeId || node.title || `Step ${index + 1}`}
                          </strong>
                          {node.agentId ? (
                            <span className="tcp-badge-info">agent: {node.agentId}</span>
                          ) : null}
                        </div>
                        <textarea
                          className="tcp-input min-h-[180px] text-sm leading-6"
                          value={node.objective}
                          onInput={(e) =>
                            setWorkflowEditDraft((current: any) =>
                              current
                                ? {
                                    ...current,
                                    nodes: current.nodes.map((row: any) =>
                                      row.nodeId === node.nodeId
                                        ? {
                                            ...row,
                                            objective: (e.target as HTMLTextAreaElement).value,
                                          }
                                        : row
                                    ),
                                  }
                                : current
                            )
                          }
                          placeholder="Describe exactly what this step should do."
                        />
                        <div className="mt-3 grid gap-2 rounded-lg border border-slate-800/70 bg-slate-950/30 p-3">
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <div className="text-xs uppercase tracking-wide text-slate-500">
                              Step routing
                            </div>
                            {node.modelProvider || node.modelId ? (
                              <span className="tcp-badge-info">overrides workflow model</span>
                            ) : (
                              <span className="tcp-badge-info">inherits workflow model</span>
                            )}
                          </div>
                          <ProviderModelSelector
                            providerLabel="Step model provider"
                            modelLabel="Step model"
                            draft={{
                              provider: node.modelProvider,
                              model: node.modelId,
                            }}
                            providers={providerOptions}
                            onChange={(draftModel) =>
                              setWorkflowEditDraft((current: any) =>
                                current
                                  ? {
                                      ...current,
                                      nodes: current.nodes.map((row: any) =>
                                        row.nodeId === node.nodeId
                                          ? {
                                              ...row,
                                              modelProvider: draftModel.provider,
                                              modelId: draftModel.model,
                                            }
                                          : row
                                      ),
                                    }
                                  : current
                              )
                            }
                            inheritLabel="Use workflow model"
                          />
                          {validateModelInput(node.modelProvider, node.modelId) ? (
                            <div className="text-xs text-red-300">
                              {validateModelInput(node.modelProvider, node.modelId)}
                            </div>
                          ) : (
                            <div className="text-xs text-slate-500">
                              Leave both fields blank to inherit the workflow model.
                            </div>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-xs text-slate-400">
                    This workflow does not currently expose editable node objectives.
                  </div>
                )}
              </AccordionSection>
            </div>
          </div>
        </div>
        <div className="tcp-confirm-actions border-t border-slate-800/70 px-4 py-3">
          <button className="tcp-btn" onClick={() => setWorkflowEditDraft(null)}>
            <i data-lucide="x-circle"></i>
            Cancel
          </button>
          <button
            className="tcp-btn"
            onClick={() =>
              workflowEditDraft &&
              workflowEditDraft.automationId &&
              runNowV2Mutation.mutate({
                id: workflowEditDraft.automationId,
              })
            }
            disabled={!workflowEditDraft?.automationId || runNowV2Mutation.isPending}
          >
            <i data-lucide="play"></i>
            {runNowV2Mutation.isPending ? "Starting..." : "Run now"}
          </button>
          <button
            className="tcp-btn-primary"
            onClick={() =>
              workflowEditDraft && updateWorkflowAutomationMutation.mutate(workflowEditDraft)
            }
            disabled={updateWorkflowAutomationMutation.isPending}
          >
            <i data-lucide="check"></i>
            {updateWorkflowAutomationMutation.isPending ? "Saving..." : "Save"}
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

export function DeleteAutomationDialog({
  deleteConfirm,
  setDeleteConfirm,
  automationActionMutation,
}: any) {
  if (!deleteConfirm) return null;

  return (
    <motion.div
      className="tcp-confirm-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={() => setDeleteConfirm(null)}
    >
      <motion.div
        className="tcp-confirm-dialog w-[min(34rem,96vw)]"
        initial={{ opacity: 0, y: 8, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: 6, scale: 0.98 }}
        onClick={(event) => event.stopPropagation()}
      >
        <h3 className="tcp-confirm-title">Delete automation</h3>
        <p className="tcp-confirm-message">
          This will permanently remove <strong>{deleteConfirm.title}</strong>.
        </p>
        <div className="tcp-confirm-actions mt-3">
          <button className="tcp-btn" onClick={() => setDeleteConfirm(null)}>
            <i data-lucide="x"></i>
            Cancel
          </button>
          <button
            className="tcp-btn-danger"
            disabled={automationActionMutation.isPending}
            onClick={() =>
              automationActionMutation.mutate(
                {
                  action: "delete",
                  automationId: deleteConfirm.automationId,
                  family: deleteConfirm.family,
                },
                {
                  onSettled: () => setDeleteConfirm(null),
                }
              )
            }
          >
            <i data-lucide="trash-2"></i>
            {automationActionMutation.isPending ? "Deleting..." : "Delete automation"}
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}
