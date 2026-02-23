const TOOL_CAPABLE_PATTERNS: RegExp[] = [
  /gpt/i,
  /claude/i,
  /gemini/i,
  /llama/i,
  /qwen/i,
  /deepseek/i,
  /mistral/i,
  /command/i,
  /o1/i,
  /o3/i,
  /o4/i,
  /glm/i,
  /grok/i,
];

const KNOWN_TOOL_MODEL_PREFIXES: Record<string, string[]> = {
  openrouter: [
    "openai/",
    "anthropic/",
    "google/",
    "x-ai/",
    "meta-llama/",
    "qwen/",
    "mistralai/",
    "deepseek/",
  ],
  openai: ["gpt-", "o1", "o3", "o4"],
  anthropic: ["claude-"],
  google: ["gemini-"],
  groq: ["llama", "mixtral", "qwen", "deepseek"],
};

const normalizeProvider = (providerId?: string | null): string => {
  return (providerId || "").trim().toLowerCase();
};

export const isLikelyToolCapableModel = (modelId: string, providerId?: string | null): boolean => {
  const normalized = modelId.trim();
  if (!normalized) return false;

  const provider = normalizeProvider(providerId);
  const providerPrefixes = KNOWN_TOOL_MODEL_PREFIXES[provider] || [];
  if (providerPrefixes.length > 0) {
    const lowerModel = normalized.toLowerCase();
    if (providerPrefixes.some((prefix) => lowerModel.startsWith(prefix))) {
      return true;
    }
  }

  return TOOL_CAPABLE_PATTERNS.some((pattern) => pattern.test(normalized));
};

export const toolCapablePolicyReason = (modelId: string): string => {
  return `Model '${modelId}' is blocked by portal tool-capable policy. Pick a model known to support tool calls.`;
};
