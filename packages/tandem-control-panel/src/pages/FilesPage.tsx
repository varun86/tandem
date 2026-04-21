import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { renderIcons } from "../app/icons.js";
import { renderMarkdownSafe } from "../lib/markdown";
import { useCapabilities } from "../features/system/queries.ts";
import { KnowledgebaseUploadPanel } from "../features/knowledgebase/KnowledgebaseUploadPanel";
import { PromptDialog } from "../components/ControlPanelDialogs";
import { AnimatedPage, PanelCard, Toolbar, Badge } from "../ui/index.tsx";
import { EmptyState } from "./ui";
import type { AppPageProps } from "./pageTypes";
import {
  consumeFilesExplorerHandoff,
  normalizeManagedFilesExplorerPath,
  parentManagedFilesExplorerDir,
} from "../features/files/explorerHandoff";

const EXPLORER_BUCKETS = ["uploads", "artifacts", "exports"];
const TEXT_PREVIEW_KINDS = new Set(["text", "markdown", "json", "yaml"]);

type FileRow = {
  name: string;
  path: string;
  size?: number;
  updatedAt?: number;
  mime?: string;
  previewKind?: string;
  downloadUrl?: string;
};

function formatBytes(bytes: number) {
  const value = Number(bytes || 0);
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function formatDateTime(ms: number) {
  const value = Number(ms || 0);
  if (!value) return "n/a";
  return new Date(value).toLocaleString();
}

function toArray(input: any, key: string) {
  if (Array.isArray(input)) return input;
  if (Array.isArray(input?.[key])) return input[key];
  return [];
}

function pathDepth(path: string) {
  const clean = String(path || "").trim();
  if (!clean) return 0;
  return clean.split("/").filter(Boolean).length;
}

export function FilesPage({ api, toast }: AppPageProps) {
  const queryClient = useQueryClient();
  const capabilities = useCapabilities();
  const rootRef = useRef<HTMLDivElement | null>(null);
  const uploadInputRef = useRef<HTMLInputElement | null>(null);
  const [dir, setDir] = useState("");
  const [selectedPath, setSelectedPath] = useState("");
  const [createDirectoryDialog, setCreateDirectoryDialog] = useState<{
    baseDir: string;
    value: string;
  } | null>(null);
  const [uploadRows, setUploadRows] = useState<
    Array<{ id: string; name: string; progress: number; error: string }>
  >([]);

  useEffect(() => {
    const handoff = consumeFilesExplorerHandoff();
    if (!handoff) return;
    if (handoff.dir) setDir(handoff.dir);
    else if (handoff.path) setDir(parentManagedFilesExplorerDir(handoff.path));
    setSelectedPath(handoff.path || handoff.dir || "");
  }, []);

  const filesQuery = useQuery({
    queryKey: ["files", dir],
    queryFn: async () =>
      api(`/api/files/list?dir=${encodeURIComponent(dir)}`).catch(() => ({
        dir,
        parent: null,
        directories: [],
        files: [],
      })),
    refetchInterval: 15000,
  });

  const rootDir = String(filesQuery.data?.dir || "").trim();
  const parentDir = String(filesQuery.data?.parent || "").trim();
  const directories = toArray(filesQuery.data, "directories");
  const files = toArray(filesQuery.data, "files") as FileRow[];

  const previewCandidate = useMemo(() => {
    return (
      files.find((entry: any) => String(entry?.path || "") === selectedPath) ||
      directories.find((entry: any) => String(entry?.path || "") === selectedPath) ||
      null
    );
  }, [filesQuery.data, selectedPath]);

  const selectedDirectory =
    (!!previewCandidate && String(previewCandidate?.previewKind || "") === "directory") ||
    (!!rootDir && selectedPath === rootDir);
  const selectedFile =
    !!previewCandidate && !selectedDirectory ? (previewCandidate as FileRow) : null;
  const selectedMime = String(selectedFile?.mime || "").trim();
  const selectedPreviewKind = String(selectedFile?.previewKind || "").trim();
  const selectedDownloadUrl =
    selectedFile?.downloadUrl || `/api/files/download?path=${encodeURIComponent(selectedPath)}`;
  const selectedTextPreview = useQuery({
    queryKey: ["files", "read", selectedPath],
    enabled: !!selectedFile && TEXT_PREVIEW_KINDS.has(selectedPreviewKind),
    queryFn: async () =>
      api(`/api/files/read?path=${encodeURIComponent(selectedPath)}`).catch((error) => ({
        ok: false,
        previewable: false,
        reason: "unavailable",
        error: error instanceof Error ? error.message : String(error),
      })),
  });
  const currentCount = directories.length + files.length;
  const currentDepth = pathDepth(rootDir);
  const selectedPreviewable =
    !!selectedFile &&
    TEXT_PREVIEW_KINDS.has(selectedPreviewKind) &&
    selectedTextPreview.data?.previewable !== false;
  const selectedPreviewLoading = selectedTextPreview.isFetching && selectedPreviewable;

  useEffect(() => {
    if (rootRef.current) renderIcons(rootRef.current);
  }, [
    currentCount,
    directories.length,
    files.length,
    selectedPath,
    selectedPreviewKind,
    selectedDirectory,
    uploadRows.length,
    rootDir,
    selectedTextPreview.data?.previewable,
  ]);

  const uploadOne = useMutation({
    mutationFn: ({ file, targetDir }: { file: File; targetDir: string }) =>
      new Promise<any>((resolve, reject) => {
        const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
        setUploadRows((prev) => [...prev, { id, name: file.name, progress: 0, error: "" }]);

        const xhr = new XMLHttpRequest();
        xhr.open("POST", `/api/files/upload?dir=${encodeURIComponent(targetDir)}`);
        xhr.withCredentials = true;
        xhr.responseType = "json";
        xhr.setRequestHeader("x-file-name", encodeURIComponent(file.name));

        xhr.upload.onprogress = (event) => {
          if (!event.lengthComputable) return;
          const pct = (event.loaded / event.total) * 100;
          setUploadRows((prev) =>
            prev.map((row) => (row.id === id ? { ...row, progress: pct } : row))
          );
        };

        xhr.onerror = () => {
          setUploadRows((prev) =>
            prev.map((row) => (row.id === id ? { ...row, error: "Network error" } : row))
          );
          window.setTimeout(() => {
            setUploadRows((prev) => prev.filter((row) => row.id !== id));
          }, 1200);
          reject(new Error(`Upload failed: ${file.name}`));
        };

        xhr.onload = () => {
          const payload = xhr.response || {};
          if (xhr.status < 200 || xhr.status >= 300 || payload?.ok === false) {
            const message = String(payload?.error || `Upload failed (${xhr.status})`);
            setUploadRows((prev) =>
              prev.map((row) => (row.id === id ? { ...row, error: message } : row))
            );
            window.setTimeout(() => {
              setUploadRows((prev) => prev.filter((row) => row.id !== id));
            }, 1600);
            reject(new Error(message));
            return;
          }

          setUploadRows((prev) => prev.filter((row) => row.id !== id));
          resolve(payload);
        };

        xhr.send(file);
      }),
    onSuccess: async (payload: any, vars) => {
      const nextPath = String(payload?.path || "").trim();
      if (nextPath) {
        const nextDir =
          String(vars.targetDir || "").trim() || parentManagedFilesExplorerDir(nextPath);
        if (!dir && nextDir === "uploads") setDir("uploads");
        setSelectedPath(nextPath);
      }
      await queryClient.invalidateQueries({ queryKey: ["files"] });
      toast(
        "ok",
        `Uploaded ${String(vars.file?.name || "file")} into ${vars.targetDir || "uploads"}.`
      );
    },
    onError: (error) => toast("err", error instanceof Error ? error.message : String(error)),
  });

  const createDirectory = useMutation({
    mutationFn: async (path: string) =>
      api("/api/files/mkdir", {
        method: "POST",
        body: JSON.stringify({ path }),
      }),
    onSuccess: async (payload: any) => {
      const nextPath = String(payload?.path || "").trim();
      if (nextPath) {
        setDir(nextPath);
        setSelectedPath(nextPath);
      }
      await queryClient.invalidateQueries({ queryKey: ["files"] });
      toast("ok", `Created folder ${nextPath || "folder"}.`);
    },
    onError: (error) => toast("err", error instanceof Error ? error.message : String(error)),
  });

  const uploadFiles = async (fileList: FileList | null) => {
    const filesToUpload = [...(fileList || [])];
    if (!filesToUpload.length) return;
    const targetDir = dir || "uploads";
    for (const file of filesToUpload) {
      await uploadOne.mutateAsync({ file, targetDir }).catch(() => undefined);
    }
    if (!dir) setDir("uploads");
  };

  const handleCreateDirectory = () => {
    const baseDir = String(dir || "").trim();
    if (!baseDir) {
      toast("warn", "Choose a bucket before creating folders.");
      return;
    }
    setCreateDirectoryDialog({
      baseDir,
      value: "knowledgebooks/new-collection",
    });
  };

  const submitCreateDirectory = async () => {
    const dialog = createDirectoryDialog;
    if (!dialog) return;
    const cleaned = String(dialog.value || "")
      .trim()
      .replace(/^\/+|\/+$/g, "")
      .replace(/\/{2,}/g, "/");
    if (!cleaned) {
      toast("warn", "Enter a folder name.");
      return;
    }
    setCreateDirectoryDialog(null);
    await createDirectory.mutateAsync(`${dialog.baseDir}/${cleaned}`);
  };

  const openDirectory = (path: string) => {
    const next = normalizeManagedFilesExplorerPath(path);
    if (!next) return;
    setDir(next);
    setSelectedPath(next);
  };

  const openParent = () => {
    if (!parentDir) {
      setDir("");
      setSelectedPath("");
      return;
    }
    setDir(parentDir);
    setSelectedPath(parentDir);
  };

  const openRoot = () => {
    setDir("");
    setSelectedPath("");
  };

  const currentLabel = !rootDir ? "Root" : rootDir;
  const currentPreviewReason = String(selectedTextPreview.data?.reason || "").trim();
  const selectedPreviewText = String(selectedTextPreview.data?.text || "").trim();
  const bucketCount = EXPLORER_BUCKETS.length;
  const hostedManaged = capabilities.data?.hosted_managed === true;
  const defaultKnowledgebaseCollectionId = String(
    capabilities.data?.hosted_deployment_slug || capabilities.data?.hosted_hostname || ""
  ).trim();

  return (
    <AnimatedPage className="grid h-full min-h-0 gap-4">
      <KnowledgebaseUploadPanel
        api={api}
        toast={toast}
        hostedManaged={hostedManaged}
        defaultCollectionId={defaultKnowledgebaseCollectionId}
      />
      <PanelCard
        fullHeight
        title="Files"
        subtitle="Browse managed uploads, published artifacts, and exports."
        actions={
          <Toolbar className="justify-start">
            <button
              type="button"
              className="tcp-btn"
              onClick={() => uploadInputRef.current?.click()}
              disabled={uploadOne.isPending}
            >
              <i data-lucide="upload"></i>
              Upload
            </button>
            <button
              type="button"
              className="tcp-btn"
              onClick={handleCreateDirectory}
              disabled={createDirectory.isPending || !dir}
              title={!dir ? "Choose a bucket first" : "Create a folder inside the current path"}
            >
              <i data-lucide="folder-plus"></i>
              New folder
            </button>
            <button type="button" className="tcp-btn" onClick={() => void filesQuery.refetch()}>
              <i data-lucide="refresh-cw"></i>
              Refresh
            </button>
            <button type="button" className="tcp-btn" onClick={openParent} disabled={!rootDir}>
              <i data-lucide="corner-up-left"></i>
              Up
            </button>
          </Toolbar>
        }
      >
        <div ref={rootRef} className="grid min-h-0 gap-4 xl:grid-cols-[280px_minmax(0,1fr)_360px]">
          <PanelCard
            fullHeight
            className="overflow-hidden"
            title="Buckets"
            subtitle="Top-level customer-visible folders."
            actions={<Badge tone="ghost">{bucketCount}</Badge>}
          >
            <div className="grid min-h-0 gap-3 p-4">
              <div className="grid gap-1">
                <button
                  type="button"
                  className={`tcp-list-item w-full text-left ${!rootDir ? "border-sky-500/40 bg-sky-950/20" : ""}`}
                  onClick={openRoot}
                >
                  <i data-lucide="hard-drive"></i>
                  Root
                </button>
                {EXPLORER_BUCKETS.map((bucket) => {
                  const active = rootDir === bucket;
                  return (
                    <button
                      key={bucket}
                      type="button"
                      className={`tcp-list-item w-full text-left ${active ? "border-sky-500/40 bg-sky-950/20" : ""}`}
                      onClick={() => openDirectory(bucket)}
                    >
                      <i data-lucide="folder-open"></i>
                      <span className="flex min-w-0 flex-1 items-center justify-between gap-2">
                        <span className="truncate">{bucket}</span>
                        <span className="tcp-subtle text-[11px]">
                          {bucket === "uploads" ? "managed" : bucket}
                        </span>
                      </span>
                    </button>
                  );
                })}
              </div>

              <div className="rounded-xl border border-white/10 bg-black/20 p-3">
                <div className="tcp-subtle text-xs uppercase tracking-wide">Path</div>
                <div className="mt-2 flex flex-wrap gap-2">
                  <button type="button" className="tcp-btn h-7 px-2 text-xs" onClick={openRoot}>
                    Root
                  </button>
                  {rootDir
                    .split("/")
                    .filter(Boolean)
                    .reduce<Array<string>>((acc, segment) => {
                      const next = acc.length ? `${acc[acc.length - 1]}/${segment}` : segment;
                      acc.push(next);
                      return acc;
                    }, [])
                    .map((segment) => (
                      <button
                        key={segment}
                        type="button"
                        className={`tcp-btn h-7 px-2 text-xs ${segment === rootDir ? "border-sky-500/40 bg-sky-950/20" : ""}`.trim()}
                        onClick={() => openDirectory(segment)}
                      >
                        {segment.split("/").pop() || segment}
                      </button>
                    ))}
                </div>
              </div>

              {uploadRows.length ? (
                <div className="grid gap-2">
                  <div className="tcp-subtle text-xs uppercase tracking-wide">Uploads</div>
                  <div className="grid gap-2">
                    {uploadRows.map((row) => (
                      <div
                        key={row.id}
                        className="rounded-xl border border-white/10 bg-black/20 p-2 text-xs"
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="truncate">{row.name}</span>
                          <span className="tcp-subtle">{Math.round(row.progress)}%</span>
                        </div>
                        <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-white/10">
                          <div
                            className={`h-full rounded-full ${row.error ? "bg-rose-400" : "bg-sky-400"}`}
                            style={{ width: `${Math.max(4, Math.min(100, row.progress || 0))}%` }}
                          ></div>
                        </div>
                        {row.error ? <div className="mt-1 text-rose-200">{row.error}</div> : null}
                      </div>
                    ))}
                  </div>
                </div>
              ) : null}
            </div>
          </PanelCard>

          <PanelCard
            fullHeight
            className="overflow-hidden"
            title={currentLabel}
            subtitle={
              currentDepth === 0
                ? "Select a bucket to browse its files."
                : `${directories.length} folder${directories.length === 1 ? "" : "s"} and ${files.length} file${files.length === 1 ? "" : "s"}`
            }
            actions={
              <div className="flex flex-wrap items-center justify-end gap-2">
                <span className="tcp-badge tcp-badge-ghost">{currentLabel}</span>
                {selectedPreviewKind ? <Badge tone="info">{selectedPreviewKind}</Badge> : null}
                {selectedDirectory ? <Badge tone="ghost">folder</Badge> : null}
              </div>
            }
          >
            <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
              <div className="tcp-toolbar">
                <span className="tcp-badge-info">
                  {directories.length} folder{directories.length === 1 ? "" : "s"}
                </span>
                <span className="tcp-badge-info">
                  {files.length} file{files.length === 1 ? "" : "s"}
                </span>
                <span className={dir ? "tcp-badge-ok" : "tcp-badge tcp-badge-ghost"}>
                  {dir ? "browsing" : "root"}
                </span>
              </div>

              <div className="grid min-h-0 gap-2 overflow-auto pr-1">
                {currentDepth === 0 ? (
                  <EmptyState
                    text="Pick a bucket on the left, or upload a file to start browsing."
                    title="No folder selected"
                  />
                ) : directories.length || files.length ? (
                  <>
                    {directories.map((entry: any) => {
                      const path = String(entry?.path || "");
                      const active = path === selectedPath;
                      return (
                        <button
                          key={`dir-${path}`}
                          type="button"
                          className={`tcp-list-item w-full text-left ${active ? "border-sky-500/40 bg-sky-950/20" : ""}`}
                          onClick={() => openDirectory(path)}
                        >
                          <i data-lucide="folder-open"></i>
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center justify-between gap-2">
                              <strong className="truncate">{String(entry?.name || path)}</strong>
                              <span className="tcp-subtle text-xs">folder</span>
                            </div>
                            <div className="mt-1 flex items-center justify-between gap-2 text-xs tcp-subtle">
                              <span className="truncate">{path}</span>
                              <span>{formatDateTime(Number(entry?.updatedAt || 0))}</span>
                            </div>
                          </div>
                        </button>
                      );
                    })}

                    {files.map((entry: FileRow) => {
                      const path = String(entry?.path || "");
                      const active = path === selectedPath;
                      const kind = String(entry?.previewKind || "file");
                      return (
                        <button
                          key={`file-${path}`}
                          type="button"
                          className={`tcp-list-item w-full text-left ${active ? "border-sky-500/40 bg-sky-950/20" : ""}`}
                          onClick={() => setSelectedPath(path)}
                        >
                          <i data-lucide="file-text"></i>
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center justify-between gap-2">
                              <strong className="truncate">{entry.name || path}</strong>
                              <span className="tcp-subtle text-xs">{kind}</span>
                            </div>
                            <div className="mt-1 flex flex-wrap items-center gap-2 text-xs tcp-subtle">
                              <span className="truncate">{path}</span>
                              <span>{formatBytes(Number(entry?.size || 0))}</span>
                              <span>{formatDateTime(Number(entry?.updatedAt || 0))}</span>
                            </div>
                          </div>
                        </button>
                      );
                    })}
                  </>
                ) : (
                  <EmptyState text="This folder does not contain any files yet." />
                )}
              </div>
            </div>
          </PanelCard>

          <PanelCard
            fullHeight
            className="overflow-hidden"
            title="Preview"
            subtitle={
              selectedFile ? selectedFile.path : selectedDirectory ? selectedPath : "Select a file"
            }
            actions={
              selectedFile ? (
                <a
                  className="tcp-btn h-8 px-3 text-xs"
                  href={selectedDownloadUrl}
                  target="_blank"
                  rel="noreferrer"
                >
                  <i data-lucide="download"></i>
                  Download
                </a>
              ) : selectedDirectory ? (
                <button
                  type="button"
                  className="tcp-btn h-8 px-3 text-xs"
                  onClick={() => openDirectory(selectedPath)}
                >
                  <i data-lucide="folder-open"></i>
                  Open
                </button>
              ) : null
            }
          >
            <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
              {selectedFile ? (
                <>
                  <div className="rounded-xl border border-white/10 bg-black/20 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="truncate text-sm font-semibold">{selectedFile.name}</div>
                        <div className="tcp-subtle mt-1 text-xs">{selectedFile.path}</div>
                      </div>
                      <div className="flex flex-col items-end gap-1">
                        <Badge tone="info">{selectedPreviewKind || "file"}</Badge>
                        <span className="tcp-subtle text-xs">
                          {selectedMime || "application/octet-stream"}
                        </span>
                      </div>
                    </div>
                    <div className="mt-3 grid grid-cols-2 gap-2 text-xs">
                      <div className="rounded-lg border border-white/10 bg-black/10 p-2">
                        <div className="tcp-subtle">Size</div>
                        <div className="mt-1 font-medium">
                          {formatBytes(Number(selectedFile.size || 0))}
                        </div>
                      </div>
                      <div className="rounded-lg border border-white/10 bg-black/10 p-2">
                        <div className="tcp-subtle">Updated</div>
                        <div className="mt-1 font-medium">
                          {formatDateTime(Number(selectedFile.updatedAt || 0))}
                        </div>
                      </div>
                    </div>
                  </div>

                  {selectedPreviewKind === "image" ? (
                    <div className="min-h-0 flex-1 overflow-auto rounded-xl border border-white/10 bg-black/20 p-3">
                      <img
                        src={selectedDownloadUrl}
                        alt={selectedFile.name}
                        className="max-h-full max-w-full rounded-lg object-contain"
                      />
                    </div>
                  ) : selectedPreviewKind === "pdf" ? (
                    <div className="min-h-0 flex-1 overflow-hidden rounded-xl border border-white/10 bg-black/20">
                      <iframe
                        title={selectedFile.name}
                        src={selectedDownloadUrl}
                        className="h-full w-full"
                      />
                    </div>
                  ) : selectedPreviewable ? (
                    <div className="min-h-0 flex-1 overflow-auto rounded-xl border border-white/10 bg-black/20 p-3">
                      {selectedPreviewLoading ? (
                        <div className="tcp-subtle text-sm">Loading preview...</div>
                      ) : selectedPreviewKind === "markdown" ? (
                        <div
                          className="tcp-markdown tcp-markdown-ai"
                          dangerouslySetInnerHTML={{
                            __html: renderMarkdownSafe(selectedPreviewText || " "),
                          }}
                        />
                      ) : (
                        <pre className="tcp-code whitespace-pre-wrap break-words">
                          {selectedPreviewText || " "}
                        </pre>
                      )}
                    </div>
                  ) : (
                    <div className="grid min-h-0 flex-1 gap-3">
                      <EmptyState
                        title="Preview unavailable"
                        text={
                          currentPreviewReason === "too_large"
                            ? `This file is larger than ${formatBytes(2 * 1024 * 1024)} for inline preview.`
                            : currentPreviewReason === "unavailable"
                              ? "The preview request failed or the file is no longer available."
                              : "This file type is not previewed inline."
                        }
                      />
                      <div className="rounded-xl border border-white/10 bg-black/20 p-3 text-xs">
                        <div className="tcp-subtle">Download URL</div>
                        <a className="mt-1 block truncate text-sky-200" href={selectedDownloadUrl}>
                          {selectedDownloadUrl}
                        </a>
                      </div>
                    </div>
                  )}
                </>
              ) : selectedDirectory ? (
                <div className="grid gap-3">
                  <div className="rounded-xl border border-white/10 bg-black/20 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <div className="text-sm font-semibold">{selectedPath || "Folder"}</div>
                        <div className="tcp-subtle mt-1 text-xs">Folder preview</div>
                      </div>
                      <Badge tone="ghost">folder</Badge>
                    </div>
                    <div className="mt-3 grid grid-cols-2 gap-2 text-xs">
                      <div className="rounded-lg border border-white/10 bg-black/10 p-2">
                        <div className="tcp-subtle">Folders</div>
                        <div className="mt-1 font-medium">{directories.length}</div>
                      </div>
                      <div className="rounded-lg border border-white/10 bg-black/10 p-2">
                        <div className="tcp-subtle">Files</div>
                        <div className="mt-1 font-medium">{files.length}</div>
                      </div>
                    </div>
                  </div>
                  <div className="rounded-xl border border-white/10 bg-black/20 p-3 text-xs">
                    <div className="tcp-subtle">Folder path</div>
                    <div className="mt-1 break-all">{selectedPath}</div>
                  </div>
                </div>
              ) : (
                <EmptyState
                  title="No selection"
                  text="Pick a file to preview it here, or select a folder to see its details."
                />
              )}
            </div>
          </PanelCard>
        </div>
      </PanelCard>

      <input
        ref={uploadInputRef}
        type="file"
        className="hidden"
        multiple
        onChange={(event) => {
          void uploadFiles((event.target as HTMLInputElement).files);
          (event.target as HTMLInputElement).value = "";
        }}
      />

      <PromptDialog
        open={!!createDirectoryDialog}
        title="Create folder"
        message={
          <span>
            Create a new folder inside{" "}
            <strong>{createDirectoryDialog?.baseDir || "current path"}</strong>.
          </span>
        }
        label="Folder path"
        value={createDirectoryDialog?.value || ""}
        placeholder="knowledgebooks/new-collection"
        confirmLabel="Create folder"
        confirmIcon="folder-plus"
        confirmDisabled={!String(createDirectoryDialog?.value || "").trim()}
        onCancel={() => setCreateDirectoryDialog(null)}
        onChange={(value) =>
          setCreateDirectoryDialog((current) => (current ? { ...current, value } : current))
        }
        onConfirm={() => void submitCreateDirectory()}
      />
    </AnimatedPage>
  );
}
