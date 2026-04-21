export type McpToolRowLike = {
  name?: string;
  tool_name?: string;
  toolName?: string;
  namespaced_name?: string;
  namespacedName?: string;
  [key: string]: unknown;
};

export type McpServerToolSourceLike = {
  name?: string;
  toolCache?: unknown[];
  tool_cache?: unknown[];
  allowedTools?: string[] | null;
  allowed_tools?: string[] | null;
  [key: string]: unknown;
};

export function normalizeMcpNamespaceSegment(raw: string) {
  let out = "";
  let previousUnderscore = false;
  for (const ch of String(raw || "").trim()) {
    if (/^[a-z0-9]$/i.test(ch)) {
      out += ch.toLowerCase();
      previousUnderscore = false;
    } else if (!previousUnderscore) {
      out += "_";
      previousUnderscore = true;
    }
  }
  return out.replace(/^_+|_+$/g, "") || "mcp";
}

export function normalizeMcpToolName(value: unknown) {
  return String(value || "").trim();
}

export function isMcpToolName(value: unknown) {
  return normalizeMcpToolName(value).toLowerCase().startsWith("mcp.");
}

export function normalizeMcpToolNames(raw: unknown) {
  const rows = Array.isArray(raw)
    ? raw
    : Array.isArray((raw as any)?.toolCache)
      ? (raw as any).toolCache
      : Array.isArray((raw as any)?.tool_cache)
        ? (raw as any).tool_cache
        : [];
  const seen = new Set<string>();
  const names: string[] = [];
  for (const row of rows) {
    const toolName =
      typeof row === "string"
        ? normalizeMcpToolName(row)
        : row && typeof row === "object"
          ? normalizeMcpToolName(
              (row as McpToolRowLike).namespaced_name ||
                (row as McpToolRowLike).namespacedName ||
                (row as McpToolRowLike).tool_name ||
                (row as McpToolRowLike).toolName ||
                (row as McpToolRowLike).name
            )
          : "";
    if (!toolName || seen.has(toolName)) continue;
    seen.add(toolName);
    names.push(toolName);
  }
  return names;
}

export function normalizeMcpAllowedTools(raw: unknown) {
  if (raw == null) return null;
  if (!Array.isArray(raw)) return [];
  const seen = new Set<string>();
  const tools: string[] = [];
  for (const entry of raw) {
    const toolName = normalizeMcpToolName(entry);
    if (!toolName || seen.has(toolName)) continue;
    seen.add(toolName);
    tools.push(toolName);
  }
  return tools;
}

export function splitMcpAllowedTools(raw: unknown) {
  const values = Array.isArray(raw) ? raw : [];
  const mcpTools: string[] = [];
  const otherTools: string[] = [];
  const seenMcp = new Set<string>();
  const seenOther = new Set<string>();
  for (const entry of values) {
    const toolName = normalizeMcpToolName(entry);
    if (!toolName) continue;
    if (isMcpToolName(toolName)) {
      if (seenMcp.has(toolName)) continue;
      seenMcp.add(toolName);
      mcpTools.push(toolName);
      continue;
    }
    if (seenOther.has(toolName)) continue;
    seenOther.add(toolName);
    otherTools.push(toolName);
  }
  return { mcpTools, otherTools };
}

export function collapseMcpAllowedToolsSelection(
  discoveredTools: string[],
  selectedTools: string[]
): string[] | null {
  const normalizedDiscovered = normalizeMcpToolNames(discoveredTools);
  const normalizedSelected = normalizeMcpToolNames(selectedTools);
  if (
    normalizedDiscovered.length &&
    normalizedDiscovered.length === normalizedSelected.length &&
    normalizedDiscovered.every((tool) => normalizedSelected.includes(tool))
  ) {
    return null;
  }
  return normalizedSelected;
}
