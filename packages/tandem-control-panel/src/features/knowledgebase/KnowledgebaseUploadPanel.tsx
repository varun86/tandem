import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { renderIcons } from "../../app/icons.js";
import { Badge, PanelCard, Toolbar } from "../../ui/index.tsx";

type KnowledgebaseCollection = {
  collection_id: string;
  document_count?: number;
  updated_at?: number;
};

type UploadRow = {
  id: string;
  name: string;
  progress: number;
  status: "queued" | "uploading" | "done" | "error";
  error: string;
};

const MAX_CONCURRENCY = 3;
const UPLOAD_ACCEPT = ".md,.markdown,.txt,text/plain,text/markdown";

function makeId() {
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function basenameWithoutExtension(name: string) {
  const raw = String(name || "").trim();
  if (!raw) return "";
  const parts = raw.split("/");
  const file = parts[parts.length - 1] || raw;
  return file.replace(/\.[^.]+$/, "");
}

export function KnowledgebaseUploadPanel({
  api,
  toast,
  hostedManaged,
  defaultCollectionId,
}: {
  api: (path: string, init?: RequestInit) => Promise<any>;
  toast: (kind: "ok" | "info" | "warn" | "err", text: string) => void;
  hostedManaged: boolean;
  defaultCollectionId?: string;
}) {
  const queryClient = useQueryClient();
  const panelRef = useRef<HTMLDivElement | null>(null);
  const uploadInputRef = useRef<HTMLInputElement | null>(null);
  const [collectionId, setCollectionId] = useState("");
  const [collectionTouched, setCollectionTouched] = useState(false);
  const [rows, setRows] = useState<UploadRow[]>([]);
  const [isUploading, setIsUploading] = useState(false);

  const collectionsQuery = useQuery({
    queryKey: ["knowledgebase", "collections"],
    enabled: hostedManaged,
    queryFn: async () => api("/api/knowledgebase/collections").catch(() => ({ collections: [] })),
    staleTime: 30_000,
    refetchInterval: 60_000,
  });

  const collections = Array.isArray(collectionsQuery.data?.collections)
    ? (collectionsQuery.data.collections as KnowledgebaseCollection[])
    : [];

  useEffect(() => {
    if (collectionTouched) return;
    const candidate = String(defaultCollectionId || "").trim();
    if (!collectionId.trim() && candidate) setCollectionId(candidate);
  }, [collectionId, collectionTouched, defaultCollectionId]);

  useEffect(() => {
    if (collectionTouched) return;
    if (collectionId.trim()) return;
    const first = String(collections[0]?.collection_id || "").trim();
    if (first) setCollectionId(first);
  }, [collectionId, collectionTouched, collections]);

  const currentCollection = collectionId.trim();

  useEffect(() => {
    if (panelRef.current) renderIcons(panelRef.current);
  }, [collections.length, rows.length, currentCollection, isUploading, hostedManaged]);

  const uploadOne = useMutation({
    mutationFn: ({ file, targetCollection }: { file: File; targetCollection: string }) =>
      new Promise<any>((resolve, reject) => {
        const id = makeId();
        setRows((prev) => [
          ...prev,
          {
            id,
            name: file.name,
            progress: 0,
            status: "queued",
            error: "",
          },
        ]);

        const xhr = new XMLHttpRequest();
        xhr.open("POST", "/api/knowledgebase/documents");
        xhr.withCredentials = true;
        xhr.responseType = "json";

        const form = new FormData();
        form.set("collection_id", targetCollection);
        form.set("title", basenameWithoutExtension(file.name));
        form.set("file", file, file.name);

        xhr.upload.onprogress = (event) => {
          if (!event.lengthComputable) return;
          const pct = (event.loaded / event.total) * 100;
          setRows((prev) =>
            prev.map((row) =>
              row.id === id ? { ...row, status: "uploading", progress: pct } : row
            )
          );
        };

        xhr.onerror = () => {
          const message = "Network error";
          setRows((prev) =>
            prev.map((row) =>
              row.id === id
                ? { ...row, status: "error", error: message, progress: Math.max(row.progress, 4) }
                : row
            )
          );
          window.setTimeout(() => {
            setRows((prev) => prev.filter((row) => row.id !== id));
          }, 4000);
          reject(new Error(`KB upload failed: ${file.name}`));
        };

        xhr.onload = () => {
          const payload = xhr.response || {};
          if (xhr.status < 200 || xhr.status >= 300 || payload?.ok === false) {
            const message = String(payload?.error || `Upload failed (${xhr.status})`);
            setRows((prev) =>
              prev.map((row) =>
                row.id === id
                  ? { ...row, status: "error", error: message, progress: Math.max(row.progress, 4) }
                  : row
              )
            );
            window.setTimeout(() => {
              setRows((prev) => prev.filter((row) => row.id !== id));
            }, 5000);
            reject(new Error(message));
            return;
          }

          setRows((prev) =>
            prev.map((row) => (row.id === id ? { ...row, status: "done", progress: 100 } : row))
          );
          window.setTimeout(() => {
            setRows((prev) => prev.filter((row) => row.id !== id));
          }, 4000);
          resolve(payload);
        };

        setRows((prev) =>
          prev.map((row) => (row.id === id ? { ...row, status: "uploading" } : row))
        );
        xhr.send(form);
      }),
  });

  const uploadFiles = async (fileList: FileList | null) => {
    const files = [...(fileList || [])];
    const targetCollection = collectionId.trim();
    if (!hostedManaged || !files.length) return;
    if (!targetCollection) {
      toast("warn", "Choose a knowledgebase collection first.");
      return;
    }

    setIsUploading(true);
    let okCount = 0;
    let failCount = 0;
    let cursor = 0;

    try {
      const workers = Array.from({ length: Math.min(MAX_CONCURRENCY, files.length) }, async () => {
        while (true) {
          const index = cursor;
          cursor += 1;
          if (index >= files.length) return;
          const file = files[index];
          try {
            await uploadOne.mutateAsync({ file, targetCollection });
            okCount += 1;
          } catch {
            failCount += 1;
          }
        }
      });
      await Promise.all(workers);
      await queryClient.invalidateQueries({ queryKey: ["knowledgebase", "collections"] });
      if (okCount && failCount) {
        toast("warn", `Uploaded ${okCount} file(s) to ${targetCollection}; ${failCount} failed.`);
      } else if (okCount) {
        toast("ok", `Uploaded ${okCount} file(s) to ${targetCollection}.`);
      } else if (failCount) {
        toast("err", `All ${failCount} KB upload(s) failed.`);
      }
    } finally {
      setIsUploading(false);
    }
  };

  const reindex = async () => {
    try {
      await api("/api/knowledgebase/reindex", { method: "POST" });
      toast("ok", "Knowledgebase reindex requested.");
    } catch (error) {
      toast("err", error instanceof Error ? error.message : String(error));
    }
  };

  if (!hostedManaged) return null;

  return (
    <PanelCard
      className="overflow-hidden"
      title="Knowledgebase"
      subtitle="Provisioned-server docs are stored separately and searched by bots through MCP."
      actions={
        <Toolbar className="justify-start">
          <button
            type="button"
            className="tcp-btn"
            onClick={() => uploadInputRef.current?.click()}
            disabled={isUploading}
          >
            <i data-lucide="upload"></i>
            Upload docs
          </button>
          <button type="button" className="tcp-btn" onClick={() => void collectionsQuery.refetch()}>
            <i data-lucide="refresh-cw"></i>
            Refresh
          </button>
          <button type="button" className="tcp-btn" onClick={reindex}>
            <i data-lucide="sparkles"></i>
            Reindex
          </button>
        </Toolbar>
      }
    >
      <div ref={panelRef} className="grid gap-4 p-4">
        <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
          <div className="grid gap-2">
            <label
              className="tcp-subtle text-xs uppercase tracking-wide"
              htmlFor="kb-collection-id"
            >
              Collection
            </label>
            <input
              id="kb-collection-id"
              className="tcp-input"
              value={collectionId}
              onChange={(event) => {
                setCollectionTouched(true);
                setCollectionId(event.target.value);
              }}
              placeholder="customer-slug"
              spellCheck={false}
            />
          </div>
          <div className="flex items-end gap-2">
            <input
              ref={uploadInputRef}
              type="file"
              multiple
              accept={UPLOAD_ACCEPT}
              className="hidden"
              onChange={(event) => {
                void uploadFiles(event.target.files);
                event.currentTarget.value = "";
              }}
            />
            <button
              type="button"
              className="tcp-btn h-10"
              onClick={() => uploadInputRef.current?.click()}
              disabled={!currentCollection || isUploading}
            >
              <i data-lucide="files"></i>
              Select files
            </button>
          </div>
        </div>

        <div className="flex flex-wrap gap-2">
          {collections.length ? (
            collections.slice(0, 8).map((collection) => {
              const name = String(collection.collection_id || "").trim();
              const active = name === currentCollection;
              return (
                <button
                  key={name}
                  type="button"
                  className={`tcp-btn h-7 px-2 text-xs ${active ? "border-sky-500/40 bg-sky-950/20" : ""}`.trim()}
                  onClick={() => {
                    setCollectionTouched(true);
                    setCollectionId(name);
                  }}
                >
                  {name}
                  {typeof collection.document_count === "number" ? (
                    <span className="ml-2 text-[10px] text-slate-400">
                      {collection.document_count}
                    </span>
                  ) : null}
                </button>
              );
            })
          ) : (
            <span className="tcp-subtle text-sm">
              No collections yet. Create a collection by uploading a doc with a new collection id.
            </span>
          )}
        </div>

        {rows.length ? (
          <div className="grid gap-2">
            <div className="tcp-subtle text-xs uppercase tracking-wide">Upload progress</div>
            <div className="grid gap-2">
              {rows.map((row) => (
                <div
                  key={row.id}
                  className="rounded-xl border border-white/10 bg-black/20 p-3 text-xs"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <div className="truncate font-medium">{row.name}</div>
                      <div className="tcp-subtle mt-1 truncate">
                        {currentCollection || "collection"} • {row.status}
                      </div>
                    </div>
                    <Badge
                      tone={row.status === "done" ? "ok" : row.status === "error" ? "err" : "info"}
                    >
                      {Math.round(row.progress)}%
                    </Badge>
                  </div>
                  <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-white/10">
                    <div
                      className={`h-full rounded-full ${
                        row.status === "error"
                          ? "bg-rose-400"
                          : row.status === "done"
                            ? "bg-emerald-400"
                            : "bg-sky-400"
                      }`}
                      style={{ width: `${Math.max(4, Math.min(100, row.progress || 0))}%` }}
                    ></div>
                  </div>
                  {row.error ? <div className="mt-2 text-rose-200">{row.error}</div> : null}
                </div>
              ))}
            </div>
          </div>
        ) : null}
      </div>
    </PanelCard>
  );
}
