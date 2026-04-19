const FILES_EXPLORER_HANDOFF_KEY = "tandem_control_panel_files_explorer_handoff";
const FILES_EXPLORER_BUCKETS = new Set(["uploads", "artifacts", "exports"]);
const LEGACY_UPLOAD_BUCKET = "control-panel";

export type FilesExplorerHandoff = {
  dir?: string;
  path?: string;
  atMs: number;
};

function normalizeExplorerPath(raw: string) {
  const text = String(raw || "")
    .trim()
    .replace(/\\/g, "/");
  if (!text) return "";

  const marker = "/channel_uploads/";
  const markerIndex = text.lastIndexOf(marker);
  const visible =
    markerIndex >= 0
      ? text.slice(markerIndex + marker.length)
      : text.startsWith("channel_uploads/")
        ? text.slice("channel_uploads/".length)
        : text.replace(/^\/+/, "");
  const normalized = visible.replace(/\/+$/, "");
  if (!normalized) return "";

  const parts = normalized.split("/").filter(Boolean);
  if (!parts.length || parts.some((part) => part === "." || part === "..")) return "";

  const [first, ...rest] = parts;
  if (first === LEGACY_UPLOAD_BUCKET) return ["uploads", ...rest].join("/");
  if (FILES_EXPLORER_BUCKETS.has(first)) return [first, ...rest].join("/");
  return "";
}

function normalizeExplorerDir(raw: string) {
  return normalizeExplorerPath(raw);
}

function explorerParentDir(raw: string) {
  const path = normalizeExplorerPath(raw);
  if (!path) return "";
  const idx = path.lastIndexOf("/");
  return idx < 0 ? "" : path.slice(0, idx);
}

function writeExplorerHandoff(handoff: FilesExplorerHandoff) {
  try {
    sessionStorage.setItem(FILES_EXPLORER_HANDOFF_KEY, JSON.stringify(handoff));
  } catch {
    // ignore storage failures
  }
}

export function isManagedFilesExplorerPath(raw: string) {
  return !!normalizeExplorerPath(raw);
}

export function normalizeManagedFilesExplorerPath(raw: string) {
  return normalizeExplorerPath(raw);
}

export function normalizeManagedFilesExplorerDir(raw: string) {
  return normalizeExplorerDir(raw);
}

export function parentManagedFilesExplorerDir(raw: string) {
  return explorerParentDir(raw);
}

export function saveFilesExplorerHandoff(target: { dir?: string; path?: string }) {
  const path = normalizeExplorerPath(target.path || "");
  const dir = normalizeExplorerDir(target.dir || "");
  const resolvedDir =
    dir || (path ? (FILES_EXPLORER_BUCKETS.has(path) ? path : explorerParentDir(path)) : "");
  writeExplorerHandoff({
    dir: resolvedDir || undefined,
    path: path || undefined,
    atMs: Date.now(),
  });
}

export function consumeFilesExplorerHandoff() {
  try {
    const raw = sessionStorage.getItem(FILES_EXPLORER_HANDOFF_KEY);
    if (!raw) return null;
    sessionStorage.removeItem(FILES_EXPLORER_HANDOFF_KEY);
    const parsed = JSON.parse(raw);
    const path = normalizeExplorerPath(parsed?.path || "");
    const dir = normalizeExplorerDir(parsed?.dir || "");
    return {
      dir: dir || undefined,
      path: path || undefined,
      atMs: Number(parsed?.atMs || 0) || Date.now(),
    } satisfies FilesExplorerHandoff;
  } catch {
    return null;
  }
}

export function openFilesExplorer(
  navigate: (route: string) => void,
  target: { dir?: string; path?: string }
) {
  const path = normalizeExplorerPath(target.path || "");
  const dir = normalizeExplorerDir(target.dir || "");
  const resolvedDir =
    dir || (path ? (FILES_EXPLORER_BUCKETS.has(path) ? path : explorerParentDir(path)) : "");
  if (!path && !resolvedDir) return false;
  saveFilesExplorerHandoff({ dir: resolvedDir || undefined, path: path || undefined });
  navigate("files");
  return true;
}
