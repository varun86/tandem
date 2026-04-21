import { type StudioTemplateDefinition, type StudioWorkflowDraft } from "./schema";
import YAML from "yaml";

const STUDIO_TEMPLATE_SOURCES = import.meta.glob("./templates/*.yaml", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

function parseStudioTemplate(source: string, sourcePath: string): StudioTemplateDefinition {
  const parsed = YAML.parse(source) as unknown;
  if (!parsed || typeof parsed !== "object") {
    throw new Error("Invalid studio template at " + sourcePath + ": expected a YAML object.");
  }

  const template = parsed as Partial<StudioTemplateDefinition>;
  const id = String(template.id || "").trim();
  const name = String(template.name || "").trim();
  const icon = String(template.icon || "").trim();
  const summary = String(template.summary || "").trim();
  const description = String(template.description || "").trim();
  const order = Number(template.order);
  const suggestedOutputs = template.suggestedOutputs;
  const agents = template.agents;
  const nodes = template.nodes;

  if (!id) throw new Error("Invalid studio template at " + sourcePath + ": missing id.");
  if (!name) throw new Error("Invalid studio template at " + sourcePath + ": missing name.");
  if (!icon) throw new Error("Invalid studio template at " + sourcePath + ": missing icon.");
  if (!summary) {
    throw new Error("Invalid studio template at " + sourcePath + ": missing summary.");
  }
  if (!description) {
    throw new Error("Invalid studio template at " + sourcePath + ": missing description.");
  }
  if (Number.isNaN(order)) {
    throw new Error("Invalid studio template at " + sourcePath + ": order must be a number.");
  }
  if (!Array.isArray(suggestedOutputs)) {
    throw new Error(
      "Invalid studio template at " + sourcePath + ": suggestedOutputs must be an array."
    );
  }
  if (!Array.isArray(agents)) {
    throw new Error("Invalid studio template at " + sourcePath + ": agents must be an array.");
  }
  if (!Array.isArray(nodes)) {
    throw new Error("Invalid studio template at " + sourcePath + ": nodes must be an array.");
  }

  return {
    id,
    name,
    icon,
    summary,
    description,
    order,
    suggestedOutputs,
    agents,
    nodes,
  };
}

export const STUDIO_TEMPLATE_CATALOG: StudioTemplateDefinition[] = Object.entries(
  STUDIO_TEMPLATE_SOURCES
)
  .map(([sourcePath, source]) => parseStudioTemplate(source, sourcePath))
  .sort((left, right) => {
    const leftOrder = Number.isFinite(left.order) ? Number(left.order) : Number.MAX_SAFE_INTEGER;
    const rightOrder = Number.isFinite(right.order) ? Number(right.order) : Number.MAX_SAFE_INTEGER;
    if (leftOrder !== rightOrder) return leftOrder - rightOrder;
    return left.name.localeCompare(right.name, undefined, { sensitivity: "base" });
  });

export function createWorkflowDraftFromTemplate(
  template: StudioTemplateDefinition,
  workspaceRoot = ""
): StudioWorkflowDraft {
  return {
    automationId: "",
    starterTemplateId: template.id,
    name: template.name,
    description: template.description,
    summary: template.summary,
    icon: template.icon,
    workspaceRoot,
    status: "draft",
    scheduleType: "manual",
    cronExpression: "",
    intervalSeconds: "3600",
    maxParallelAgents: "1",
    useSharedModel: false,
    sharedModelProvider: "",
    sharedModelId: "",
    outputTargets: [...template.suggestedOutputs],
    agents: template.agents.map((entry) => ({
      ...entry,
      skills: [...entry.skills],
      toolAllowlist: [...entry.toolAllowlist],
      toolDenylist: [...entry.toolDenylist],
      mcpAllowedServers: [...entry.mcpAllowedServers],
      mcpAllowedTools: Array.isArray(entry.mcpAllowedTools) ? [...entry.mcpAllowedTools] : null,
      mcpOtherAllowedTools: Array.isArray(entry.mcpOtherAllowedTools)
        ? [...entry.mcpOtherAllowedTools]
        : [],
      prompt: { ...entry.prompt },
    })),
    nodes: template.nodes.map((entry) => ({
      ...entry,
      dependsOn: [...entry.dependsOn],
      inputRefs: entry.inputRefs.map((ref) => ({ ...ref })),
    })),
  };
}
