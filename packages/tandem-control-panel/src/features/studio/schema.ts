export type StudioRole =
  | "worker"
  | "reviewer"
  | "tester"
  | "watcher"
  | "delegator"
  | "committer"
  | "orchestrator";

export type StudioPromptSections = {
  role: string;
  mission: string;
  inputs: string;
  outputContract: string;
  guardrails: string;
};

export type StudioAgentDraft = {
  agentId: string;
  displayName: string;
  role: StudioRole;
  avatarUrl: string;
  templateId: string;
  linkedTemplateId: string;
  skills: string[];
  prompt: StudioPromptSections;
  modelProvider: string;
  modelId: string;
  toolAllowlist: string[];
  toolDenylist: string[];
  mcpAllowedServers: string[];
};

export type StudioNodeDraft = {
  nodeId: string;
  title: string;
  agentId: string;
  objective: string;
  dependsOn: string[];
  inputRefs: Array<{ fromStepId: string; alias: string }>;
  inputFiles: string[];
  stageKind?: string;
  outputKind: string;
  outputPath: string;
  outputFiles: string[];
  taskKind?: string;
  projectBacklogTasks?: boolean;
  backlogTaskId?: string;
  repoRoot?: string;
  writeScope?: string;
  acceptanceCriteria?: string;
  taskDependencies?: string;
  verificationState?: string;
  taskOwner?: string;
  verificationCommand?: string;
};

export type StudioWorkflowDraft = {
  automationId: string;
  starterTemplateId: string;
  name: string;
  description: string;
  summary: string;
  icon: string;
  workspaceRoot: string;
  status: "draft" | "active" | "paused";
  scheduleType: "manual" | "cron" | "interval";
  cronExpression: string;
  intervalSeconds: string;
  maxParallelAgents: string;
  useSharedModel: boolean;
  sharedModelProvider: string;
  sharedModelId: string;
  outputTargets: string[];
  agents: StudioAgentDraft[];
  nodes: StudioNodeDraft[];
};

export type StudioTemplateDefinition = {
  id: string;
  order?: number;
  name: string;
  icon: string;
  summary: string;
  description: string;
  agents: StudioAgentDraft[];
  nodes: StudioNodeDraft[];
  suggestedOutputs: string[];
};

export function emptyPromptSections(
  overrides: Partial<StudioPromptSections> = {}
): StudioPromptSections {
  return {
    role: "",
    mission: "",
    inputs: "",
    outputContract: "",
    guardrails: "",
    ...overrides,
  };
}

export function createEmptyAgentDraft(
  agentId: string,
  displayName: string,
  overrides: Partial<StudioAgentDraft> = {}
): StudioAgentDraft {
  return {
    agentId,
    displayName,
    role: "worker",
    avatarUrl: "",
    templateId: "",
    linkedTemplateId: "",
    skills: [],
    prompt: emptyPromptSections(),
    modelProvider: "",
    modelId: "",
    toolAllowlist: ["read", "write", "glob"],
    toolDenylist: [],
    mcpAllowedServers: [],
    ...overrides,
  };
}

export function createEmptyNodeDraft(
  nodeId: string,
  title: string,
  agentId: string,
  dependsOn: string[] = [],
  inputRefs: Array<{ fromStepId: string; alias: string }> = [],
  overrides: Partial<StudioNodeDraft> = {}
): StudioNodeDraft {
  return {
    nodeId,
    title,
    agentId,
    objective: "",
    dependsOn: [...dependsOn],
    inputRefs: inputRefs.map((ref) => ({ ...ref })),
    inputFiles: [],
    outputKind: "artifact",
    outputPath: "",
    outputFiles: [],
    taskKind: "",
    projectBacklogTasks: false,
    backlogTaskId: "",
    repoRoot: "",
    writeScope: "",
    acceptanceCriteria: "",
    taskDependencies: "",
    verificationState: "",
    taskOwner: "",
    verificationCommand: "",
    ...overrides,
  };
}
