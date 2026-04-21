import { useMemo } from "react";
import { collapseMcpAllowedToolsSelection, normalizeMcpToolNames } from "../features/mcp/mcpTools";

type McpToolAllowlistEditorProps = {
  title: string;
  subtitle?: string;
  discoveredTools: string[];
  value: string[] | null;
  onChange: (next: string[] | null) => void;
  disabled?: boolean;
  emptyText?: string;
};

export function McpToolAllowlistEditor({
  title,
  subtitle,
  discoveredTools,
  value,
  onChange,
  disabled = false,
  emptyText = "No MCP tools have been discovered for this server yet.",
}: McpToolAllowlistEditorProps) {
  const discovered = useMemo(() => normalizeMcpToolNames(discoveredTools), [discoveredTools]);
  const selected = useMemo(() => {
    if (value === null) return [...discovered];
    return normalizeMcpToolNames(value);
  }, [value, discovered]);
  const selectedSet = useMemo(() => new Set(selected), [selected]);
  const discoveredSet = useMemo(() => new Set(discovered), [discovered]);
  const visibleSelectedCount = discovered.filter((tool) => selectedSet.has(tool)).length;
  const extraSelected = selected.filter((tool) => !discoveredSet.has(tool));
  const allVisibleSelected =
    discovered.length > 0 &&
    visibleSelectedCount === discovered.length &&
    extraSelected.length === 0;

  const setSelection = (next: string[] | null) => {
    if (disabled) return;
    onChange(next);
  };

  const toggleTool = (toolName: string) => {
    if (disabled) return;
    const next = new Set(value === null ? discovered : selected);
    if (next.has(toolName)) {
      next.delete(toolName);
    } else {
      next.add(toolName);
    }
    const nextSelected = Array.from(next);
    const collapsed = collapseMcpAllowedToolsSelection(discovered, nextSelected);
    onChange(collapsed === null ? null : nextSelected);
  };

  return (
    <div className="grid gap-3 rounded-xl border border-slate-700/70 bg-slate-950/30 p-3">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="grid gap-1">
          <div className="text-sm font-medium text-slate-100">{title}</div>
          {subtitle ? <div className="text-xs text-slate-400">{subtitle}</div> : null}
        </div>
        <div className="flex flex-wrap items-center gap-2 text-[11px] text-slate-400">
          <span className="rounded-full border border-slate-700 px-2 py-1">
            {value === null || allVisibleSelected
              ? "all discovered"
              : `${visibleSelectedCount}/${discovered.length || 0} selected`}
          </span>
          <button
            type="button"
            className="rounded-full border border-slate-700 px-2 py-1 text-slate-200 transition hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-50"
            disabled={disabled || !discovered.length}
            onClick={() => setSelection(null)}
          >
            Select all
          </button>
          <button
            type="button"
            className="rounded-full border border-slate-700 px-2 py-1 text-slate-200 transition hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-50"
            disabled={disabled || (!selected.length && !extraSelected.length)}
            onClick={() => setSelection([])}
          >
            Clear all
          </button>
        </div>
      </div>

      {!discovered.length && !extraSelected.length ? (
        <div className="text-xs text-slate-500">{emptyText}</div>
      ) : (
        <>
          {discovered.length ? (
            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
              {discovered.map((toolName) => {
                const checked = value === null || selectedSet.has(toolName);
                return (
                  <label
                    key={toolName}
                    className={`flex items-start gap-2 rounded-lg border px-3 py-2 text-sm transition ${
                      checked
                        ? "border-amber-400/40 bg-amber-400/10 text-amber-100"
                        : "border-slate-700/70 bg-slate-950/20 text-slate-300"
                    } ${disabled ? "opacity-60" : "cursor-pointer"}`}
                  >
                    <input
                      type="checkbox"
                      className="mt-1"
                      checked={checked}
                      disabled={disabled}
                      onChange={() => toggleTool(toolName)}
                    />
                    <span className="break-all font-mono text-[11px] leading-5">{toolName}</span>
                  </label>
                );
              })}
            </div>
          ) : null}

          {extraSelected.length ? (
            <div className="grid gap-2">
              <div className="text-[11px] uppercase tracking-wide text-amber-200">
                Saved but not currently discovered
              </div>
              <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
                {extraSelected.map((toolName) => (
                  <label
                    key={toolName}
                    className={`flex items-start gap-2 rounded-lg border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-sm text-amber-100 ${
                      disabled ? "opacity-60" : "cursor-pointer"
                    }`}
                  >
                    <input
                      type="checkbox"
                      className="mt-1"
                      checked
                      disabled={disabled}
                      onChange={() => toggleTool(toolName)}
                    />
                    <span className="break-all font-mono text-[11px] leading-5">{toolName}</span>
                  </label>
                ))}
              </div>
            </div>
          ) : null}
        </>
      )}
    </div>
  );
}
