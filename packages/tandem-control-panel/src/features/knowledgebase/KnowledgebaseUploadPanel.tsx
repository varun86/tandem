import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { renderIcons } from "../../app/icons.js";
import { renderMarkdownSafe } from "../../lib/markdown";
import { ConfirmDialog } from "../../components/ControlPanelDialogs";
import { Badge, EmptyState, PanelCard, Toolbar } from "../../ui/index.tsx";

type KnowledgebaseCollection = {
  collection_id: string;
  document_count?: number;
  updated_at?: number;
};

type KnowledgebaseDocument = {
  doc_id?: string;
  id?: string;
  collection_id?: string;
  title?: string;
  filename?: string;
  file_name?: string;
  updated_at?: number;
  created_at?: number;
  content?: string;
  excerpt?: string;
  content_type?: string;
  path?: string;
  size_bytes?: number;
};

type KnowledgebaseUploadResult = {
  docId: string;
  collectionId: string;
  title: string;
};

type UploadRow = {
  id: string;
  name: string;
  progress: number;
  status: "queued" | "uploading" | "done" | "error";
  error: string;
  result?: KnowledgebaseUploadResult;
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

function toDocumentList(payload: any): KnowledgebaseDocument[] {
  if (Array.isArray(payload)) return payload as KnowledgebaseDocument[];
  if (Array.isArray(payload?.documents)) return payload.documents as KnowledgebaseDocument[];
  if (Array.isArray(payload?.items)) return payload.items as KnowledgebaseDocument[];
  if (payload?.document && typeof payload.document === "object") {
    return [payload.document as KnowledgebaseDocument];
  }
  return [];
}

function toCollectionList(payload: any): KnowledgebaseCollection[] {
  if (Array.isArray(payload)) return payload as KnowledgebaseCollection[];
  if (Array.isArray(payload?.collections)) return payload.collections as KnowledgebaseCollection[];
  if (Array.isArray(payload?.items)) return payload.items as KnowledgebaseCollection[];
  return [];
}

function docCollectionId(document: KnowledgebaseDocument) {
  const explicit = String(document.collection_id || "").trim();
  if (explicit) return explicit;
  const docId = String(document.doc_id || document.id || "").trim();
  const slash = docId.indexOf("/");
  return slash > 0 ? docId.slice(0, slash) : "";
}

function documentIdentity(document: KnowledgebaseDocument) {
  const collection = docCollectionId(document);
  const docId = String(document.doc_id || document.id || "").trim();
  if (!collection || !docId) return "";
  return `${collection}|${docId}`;
}

function documentKey(document: KnowledgebaseDocument) {
  return [
    String(document.doc_id || document.id || "")
      .trim()
      .toLowerCase(),
    String(document.collection_id || "")
      .trim()
      .toLowerCase(),
    String(document.title || document.filename || document.file_name || "")
      .trim()
      .toLowerCase(),
  ]
    .filter(Boolean)
    .join("|");
}

function dedupeDocuments(documents: KnowledgebaseDocument[]) {
  const seen = new Set<string>();
  const next: KnowledgebaseDocument[] = [];
  for (const document of documents) {
    const key = documentKey(document);
    if (key && seen.has(key)) continue;
    if (key) seen.add(key);
    next.push(document);
  }
  return next;
}

function documentMatchesCollection(document: KnowledgebaseDocument, collectionId: string) {
  const target = String(collectionId || "")
    .trim()
    .toLowerCase();
  if (!target) return true;
  const explicit = String(document.collection_id || "")
    .trim()
    .toLowerCase();
  if (explicit && explicit === target) return true;
  const docId = String(document.doc_id || document.id || "")
    .trim()
    .toLowerCase();
  return docId.startsWith(`${target}/`);
}

function formatKbDate(value?: number) {
  const numeric = Number(value || 0);
  if (!numeric) return "n/a";
  return new Date(numeric).toLocaleString();
}

function formatKbBytes(value?: number) {
  const numeric = Number(value || 0);
  if (!numeric) return "n/a";
  if (numeric < 1024) return `${numeric} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = numeric / 1024;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[unit]}`;
}

function inferDocumentPreviewKind(document?: KnowledgebaseDocument | null) {
  const contentType = String(document?.content_type || "").toLowerCase();
  const title = String(
    document?.title || document?.filename || document?.file_name || ""
  ).toLowerCase();
  if (contentType.includes("markdown") || title.endsWith(".md") || title.endsWith(".markdown")) {
    return "markdown";
  }
  if (contentType.includes("json") || title.endsWith(".json")) {
    return "json";
  }
  return "text";
}

function getDocumentSlug(document?: KnowledgebaseDocument | null) {
  const collection = docCollectionId(document || {});
  const docId = String(document?.doc_id || document?.id || "").trim();
  if (!collection || !docId) return "";
  if (docId.startsWith(`${collection}/`)) return docId.slice(collection.length + 1);
  return docId;
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
  const folderInputRef = useRef<HTMLInputElement | null>(null);
  const [collectionId, setCollectionId] = useState("");
  const [collectionTouched, setCollectionTouched] = useState(false);
  const [documentSearch, setDocumentSearch] = useState("");
  const [selectedDocumentKey, setSelectedDocumentKey] = useState("");
  const [previewExpanded, setPreviewExpanded] = useState(false);
  const [editMode, setEditMode] = useState(false);
  const [editDraft, setEditDraft] = useState("");
  const [editError, setEditError] = useState("");
  const [deleteConfirm, setDeleteConfirm] = useState<{
    collection: string;
    slug: string;
    title: string;
  } | null>(null);
  const [rows, setRows] = useState<UploadRow[]>([]);
  const [isUploading, setIsUploading] = useState(false);

  const collectionsQuery = useQuery({
    queryKey: ["knowledgebase", "collections"],
    enabled: hostedManaged,
    queryFn: async () => api("/api/knowledgebase/collections").catch(() => ({ collections: [] })),
    staleTime: 30_000,
    refetchInterval: 60_000,
  });

  const queryCollectionsRaw = Array.isArray(collectionsQuery.data?.collections)
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
    const first = String(queryCollectionsRaw[0]?.collection_id || "").trim();
    if (first) setCollectionId(first);
  }, [collectionId, collectionTouched, queryCollectionsRaw]);

  useEffect(() => {
    const input = folderInputRef.current;
    if (!input) return;
    input.setAttribute("webkitdirectory", "");
    input.setAttribute("directory", "");
  }, []);

  const currentCollection = collectionId.trim();
  const documentsQuery = useQuery({
    queryKey: ["knowledgebase", "documents", currentCollection],
    enabled: hostedManaged && !!currentCollection,
    queryFn: async () => {
      const [allPayload, scopedPayload] = await Promise.all([
        api("/api/knowledgebase/documents").catch(() => ({ documents: [] })),
        api(
          `/api/knowledgebase/documents?collection_id=${encodeURIComponent(currentCollection)}`
        ).catch(() => ({ documents: [] })),
      ]);
      return {
        documents: [...toDocumentList(allPayload), ...toDocumentList(scopedPayload)],
      };
    },
    staleTime: 15_000,
    refetchInterval: currentCollection ? 30_000 : false,
  });
  const documents = useMemo(() => {
    const combined = dedupeDocuments(toDocumentList(documentsQuery.data));
    return currentCollection
      ? combined.filter((document) => documentMatchesCollection(document, currentCollection))
      : combined;
  }, [currentCollection, documentsQuery.data]);

  useEffect(() => {
    if (!documents.length) {
      if (selectedDocumentKey) setSelectedDocumentKey("");
      return;
    }
    const selectionExists = documents.some(
      (document) => documentIdentity(document) === selectedDocumentKey
    );
    if (selectedDocumentKey && !selectionExists) {
      setSelectedDocumentKey("");
      setPreviewExpanded(false);
      setEditMode(false);
      setEditError("");
    }
  }, [documents, selectedDocumentKey]);

  useEffect(() => {
    setPreviewExpanded(true);
    setEditMode(false);
    setEditError("");
  }, [selectedDocumentKey]);

  const queryCollections = queryCollectionsRaw;
  const derivedCollections = useMemo(() => {
    const map = new Map<string, KnowledgebaseCollection>();
    for (const document of documents) {
      const name = docCollectionId(document);
      if (!name) continue;
      const existing = map.get(name) || { collection_id: name, document_count: 0, updated_at: 0 };
      existing.document_count = Number(existing.document_count || 0) + 1;
      existing.updated_at = Math.max(
        Number(existing.updated_at || 0),
        Number(document.updated_at || document.created_at || 0)
      );
      map.set(name, existing);
    }
    return [...map.values()].sort((a, b) =>
      String(a.collection_id).localeCompare(String(b.collection_id))
    );
  }, [documents]);
  const collections = useMemo(() => {
    const map = new Map<string, KnowledgebaseCollection>();
    for (const collection of queryCollections) {
      const name = String(collection.collection_id || "").trim();
      if (!name) continue;
      map.set(name, collection);
    }
    for (const collection of derivedCollections) {
      const name = String(collection.collection_id || "").trim();
      if (!name) continue;
      const current = map.get(name) || collection;
      map.set(name, {
        ...current,
        document_count:
          Number(current.document_count || 0) ||
          Number(collection.document_count || 0) ||
          undefined,
        updated_at: Math.max(Number(current.updated_at || 0), Number(collection.updated_at || 0)),
      });
    }
    return [...map.values()].sort((a, b) =>
      String(a.collection_id).localeCompare(String(b.collection_id))
    );
  }, [derivedCollections, queryCollections]);
  const visibleDocuments = useMemo(() => {
    const term = documentSearch.trim().toLowerCase();
    if (!term) return documents;
    return documents.filter((document) => {
      const haystack = [
        document.title,
        document.filename,
        document.file_name,
        document.doc_id,
        document.id,
        document.collection_id,
        document.path,
        document.excerpt,
      ]
        .map((value) => String(value || "").toLowerCase())
        .join(" ");
      return haystack.includes(term);
    });
  }, [documents, documentSearch]);
  const selectedDocument = useMemo(
    () => documents.find((document) => documentIdentity(document) === selectedDocumentKey) || null,
    [documents, selectedDocumentKey]
  );
  const selectedDocumentSlug = getDocumentSlug(selectedDocument);
  const selectedDocumentCollection = String(
    docCollectionId(selectedDocument || {}) || currentCollection
  ).trim();
  const selectedDocumentQuery = useQuery({
    queryKey: ["knowledgebase", "document", selectedDocumentCollection, selectedDocumentSlug],
    enabled: hostedManaged && !!selectedDocumentCollection && !!selectedDocumentSlug,
    queryFn: async () =>
      api(
        `/api/knowledgebase/documents/${encodeURIComponent(selectedDocumentCollection)}/${encodeURIComponent(
          selectedDocumentSlug
        )}`
      ),
    staleTime: 30_000,
  });
  const selectedDocumentDetail =
    selectedDocumentQuery.data?.document || selectedDocumentQuery.data || null;
  const selectedPreviewKind = inferDocumentPreviewKind(selectedDocumentDetail || selectedDocument);
  const selectedPreviewTitle = String(
    selectedDocumentDetail?.title ||
      selectedDocumentDetail?.filename ||
      selectedDocumentDetail?.file_name ||
      selectedDocument?.title ||
      selectedDocument?.filename ||
      selectedDocument?.file_name ||
      selectedDocument?.doc_id ||
      "Document"
  ).trim();
  const selectedPreviewPath = String(
    selectedDocumentDetail?.path ||
      selectedDocumentDetail?.doc_id ||
      selectedDocument?.path ||
      selectedDocument?.doc_id ||
      ""
  ).trim();
  const selectedPreviewContent = String(
    selectedDocumentDetail?.content ||
      selectedDocumentDetail?.excerpt ||
      selectedDocument?.excerpt ||
      ""
  );
  const selectedDocumentError = selectedDocumentQuery.error
    ? selectedDocumentQuery.error instanceof Error
      ? selectedDocumentQuery.error.message
      : String(selectedDocumentQuery.error)
    : "";
  const selectedPreviewUpdatedAt = Number(
    selectedDocumentDetail?.updated_at ||
      selectedDocumentDetail?.created_at ||
      selectedDocument?.updated_at ||
      selectedDocument?.created_at ||
      0
  );
  const selectedPreviewSizeBytes = Number(
    selectedDocumentDetail?.size_bytes || selectedDocument?.size_bytes || 0
  );
  const selectedDocumentCanMutate = Boolean(selectedDocumentCollection && selectedDocumentSlug);
  const selectedDocumentPanel = selectedDocument ? (
    <div className="flex min-h-0 flex-col rounded-xl border border-white/10 bg-black/20 p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold text-slate-100">
            {selectedPreviewTitle}
          </div>
          <div className="tcp-subtle mt-1 truncate text-xs">
            {selectedPreviewPath || selectedDocumentSlug || currentCollection}
          </div>
        </div>
        <div className="flex flex-col items-end gap-2">
          <Badge tone="info">{selectedDocumentCollection || currentCollection}</Badge>
          <div className="flex flex-wrap justify-end gap-2">
            <button
              type="button"
              className="tcp-btn h-7 w-7 justify-center px-0"
              title="Refresh preview"
              aria-label="Refresh preview"
              onClick={() => void selectedDocumentQuery.refetch()}
              disabled={!selectedDocumentKey}
            >
              <i data-lucide="refresh-cw"></i>
              <span className="sr-only">Refresh preview</span>
            </button>
            <button
              type="button"
              className="tcp-btn h-7 w-7 justify-center px-0"
              title="Copy document content"
              aria-label="Copy document content"
              onClick={() => void copySelectedDocument()}
              disabled={!selectedPreviewContent.trim()}
            >
              <i data-lucide="copy"></i>
              <span className="sr-only">Copy document content</span>
            </button>
            <button
              type="button"
              className="tcp-btn h-7 w-7 justify-center border-rose-500/30 px-0 text-rose-200 hover:bg-rose-950/20"
              title="Delete document"
              aria-label="Delete document"
              onClick={() =>
                setDeleteConfirm({
                  collection: selectedDocumentCollection,
                  slug: selectedDocumentSlug,
                  title: selectedPreviewTitle,
                })
              }
              disabled={!selectedDocumentCanMutate}
            >
              <i data-lucide="trash-2"></i>
              <span className="sr-only">Delete document</span>
            </button>
          </div>
        </div>
      </div>

      <div className="mt-3 grid grid-cols-3 gap-2 text-xs">
        <div className="rounded-lg border border-white/10 bg-black/10 p-2">
          <div className="tcp-subtle">Type</div>
          <div className="mt-1 font-medium">
            {String(selectedDocumentDetail?.content_type || selectedPreviewKind || "text")}
          </div>
        </div>
        <div className="rounded-lg border border-white/10 bg-black/10 p-2">
          <div className="tcp-subtle">Updated</div>
          <div className="mt-1 font-medium">{formatKbDate(selectedPreviewUpdatedAt)}</div>
        </div>
        <div className="rounded-lg border border-white/10 bg-black/10 p-2">
          <div className="tcp-subtle">Size</div>
          <div className="mt-1 font-medium">{formatKbBytes(selectedPreviewSizeBytes)}</div>
        </div>
      </div>

      <div className="mt-3 rounded-xl border border-white/10 bg-black/10 p-3 text-xs">
        <div className="tcp-subtle uppercase tracking-wide">Excerpt</div>
        <div className="mt-1 whitespace-pre-wrap break-words text-slate-200">
          {String(
            selectedDocumentDetail?.excerpt ||
              selectedPreviewContent.slice(0, 600) ||
              "No excerpt returned."
          )}
        </div>
      </div>

      {selectedDocumentQuery.isLoading && !selectedDocumentDetail ? (
        <div className="mt-3 rounded-xl border border-white/10 bg-black/20 p-3 text-sm tcp-subtle">
          Loading preview...
        </div>
      ) : selectedDocumentError ? (
        <div className="mt-3">
          <EmptyState title="Preview unavailable" text={selectedDocumentError} />
        </div>
      ) : editMode ? (
        <div className="mt-3 flex min-h-0 flex-1 flex-col gap-3 rounded-xl border border-sky-500/30 bg-sky-950/10 p-3">
          <div className="flex items-center justify-between gap-3">
            <div className="tcp-subtle text-xs uppercase tracking-wide">Edit document in place</div>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                className="tcp-btn h-7 px-2 text-xs"
                onClick={() => {
                  setEditMode(false);
                  setEditError("");
                  setEditDraft(String(selectedPreviewContent || ""));
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                className="tcp-btn h-7 px-2 text-xs"
                onClick={() => void saveSelectedDocument()}
              >
                <i data-lucide="save"></i>
                Save
              </button>
            </div>
          </div>
          {editError ? (
            <div className="rounded-lg border border-rose-500/30 bg-rose-950/20 p-2 text-xs text-rose-200">
              {editError}
            </div>
          ) : null}
          <textarea
            className="tcp-input min-h-[340px] flex-1 resize-none font-mono text-xs leading-6"
            value={editDraft}
            onChange={(event) => setEditDraft(event.target.value)}
            spellCheck={false}
          />
        </div>
      ) : previewExpanded ? (
        <div className="mt-3 flex min-h-0 flex-1 flex-col gap-3 rounded-xl border border-white/10 bg-black/20 p-3">
          <div className="flex items-center justify-between gap-3">
            <div className="tcp-subtle text-xs uppercase tracking-wide">Document content</div>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                className="tcp-btn h-7 w-7 justify-center px-0"
                title="Edit document"
                aria-label="Edit document"
                onClick={() => {
                  setEditError("");
                  setEditMode(true);
                  setEditDraft(String(selectedPreviewContent || ""));
                }}
                disabled={!selectedPreviewContent.trim()}
              >
                <i data-lucide="square-pen"></i>
                <span className="sr-only">Edit document</span>
              </button>
              <button
                type="button"
                className="tcp-btn h-7 w-7 justify-center px-0"
                title="Collapse preview"
                aria-label="Collapse preview"
                onClick={() => setPreviewExpanded(false)}
              >
                <i data-lucide="chevron-up"></i>
                <span className="sr-only">Collapse preview</span>
              </button>
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-auto rounded-xl border border-white/10 bg-black/20 p-3">
            {selectedPreviewContent ? (
              selectedPreviewKind === "markdown" ? (
                <div
                  className="tcp-markdown tcp-markdown-ai"
                  dangerouslySetInnerHTML={{
                    __html: renderMarkdownSafe(selectedPreviewContent),
                  }}
                />
              ) : (
                <pre className="tcp-code whitespace-pre-wrap break-words">
                  {selectedPreviewContent}
                </pre>
              )
            ) : (
              <EmptyState
                title="No preview content"
                text="This document did not return preview text from the KB service."
              />
            )}
          </div>
        </div>
      ) : (
        <EmptyState
          title="Preview collapsed"
          text="Open the document content when you want to inspect or edit it. The list stays compact until you expand a file."
          action={
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                className="tcp-btn h-8 w-8 justify-center px-0"
                title="Open content"
                aria-label="Open content"
                onClick={() => setPreviewExpanded(true)}
              >
                <i data-lucide="chevron-down"></i>
                <span className="sr-only">Open content</span>
              </button>
              <button
                type="button"
                className="tcp-btn h-8 w-8 justify-center px-0"
                title="Edit in place"
                aria-label="Edit in place"
                onClick={() => {
                  setPreviewExpanded(true);
                  setEditError("");
                  setEditMode(true);
                  setEditDraft(String(selectedPreviewContent || ""));
                }}
                disabled={!selectedPreviewContent.trim()}
              >
                <i data-lucide="square-pen"></i>
                <span className="sr-only">Edit in place</span>
              </button>
            </div>
          }
        />
      )}
    </div>
  ) : null;

  useEffect(() => {
    if (editMode) return;
    setEditDraft(String(selectedPreviewContent || ""));
  }, [editMode, selectedPreviewContent, selectedDocumentKey]);

  const completedRows = rows.filter((row) => row.status === "done" || row.status === "error");

  useEffect(() => {
    if (panelRef.current) renderIcons(panelRef.current);
  }, [
    collections.length,
    rows.length,
    currentCollection,
    isUploading,
    hostedManaged,
    documents.length,
    visibleDocuments.length,
    selectedDocumentKey,
    documentSearch,
    selectedDocumentQuery.data,
    selectedDocumentQuery.isFetching,
    previewExpanded,
    editMode,
  ]);

  const clearFinishedUploads = () => {
    setRows((prev) => prev.filter((row) => row.status === "queued" || row.status === "uploading"));
  };

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
            reject(new Error(message));
            return;
          }

          const document = (
            payload?.document && typeof payload.document === "object"
              ? payload.document
              : payload?.doc || payload?.item || {}
          ) as Record<string, any>;
          const nextDocId = String(
            document?.doc_id || document?.id || payload?.doc_id || ""
          ).trim();
          const nextCollectionId = String(document?.collection_id || targetCollection).trim();
          const nextTitle = String(
            document?.title ||
              document?.filename ||
              document?.file_name ||
              basenameWithoutExtension(file.name) ||
              file.name
          ).trim();

          setRows((prev) =>
            prev.map((row) =>
              row.id === id
                ? {
                    ...row,
                    status: "done",
                    progress: 100,
                    error: "",
                    result: {
                      docId: nextDocId,
                      collectionId: nextCollectionId,
                      title: nextTitle,
                    },
                  }
                : row
            )
          );
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
      await queryClient.invalidateQueries({ queryKey: ["knowledgebase", "documents"] });
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

  const copySelectedDocument = async () => {
    const text = String(selectedPreviewContent || "").trim();
    if (!text) {
      toast("warn", "No document content is available to copy.");
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      toast("ok", "Document content copied to clipboard.");
    } catch {
      toast("warn", "Could not copy the document content.");
    }
  };

  const saveSelectedDocument = async () => {
    const collection = String(selectedDocumentCollection || "").trim();
    const slug = String(selectedDocumentSlug || "").trim();
    if (!collection || !slug) {
      toast("warn", "Pick a document before saving edits.");
      return;
    }
    setEditError("");
    try {
      const form = new FormData();
      form.set("title", String(selectedPreviewTitle || "").trim() || slug);
      form.set("content", editDraft);
      const response = await fetch(
        `/api/knowledgebase/documents/${encodeURIComponent(collection)}/${encodeURIComponent(slug)}`,
        {
          method: "PUT",
          credentials: "include",
          body: form,
        }
      );
      const text = await response.text();
      let payload: any = {};
      if (text) {
        try {
          payload = JSON.parse(text);
        } catch {
          payload = {};
        }
      }
      if (!response.ok || payload?.ok === false) {
        const message = String(payload?.error || text || `Update failed (${response.status})`);
        throw new Error(message);
      }
      setEditMode(false);
      setPreviewExpanded(true);
      toast("ok", "Document updated.");
      await queryClient.invalidateQueries({ queryKey: ["knowledgebase", "documents"] });
      await queryClient.invalidateQueries({ queryKey: ["knowledgebase", "document"] });
      await documentsQuery.refetch();
      await selectedDocumentQuery.refetch();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setEditError(message);
      toast("err", message);
    }
  };

  const confirmDeleteSelectedDocument = async () => {
    const collection = String(deleteConfirm?.collection || "").trim();
    const slug = String(deleteConfirm?.slug || "").trim();
    if (!collection || !slug) {
      toast("warn", "Pick a document before deleting it.");
      return;
    }
    try {
      const response = await fetch(
        `/api/knowledgebase/documents/${encodeURIComponent(collection)}/${encodeURIComponent(slug)}`,
        {
          method: "DELETE",
          credentials: "include",
        }
      );
      const text = await response.text();
      let payload: any = {};
      if (text) {
        try {
          payload = JSON.parse(text);
        } catch {
          payload = {};
        }
      }
      if (!response.ok || payload?.ok === false) {
        const message = String(payload?.error || text || `Delete failed (${response.status})`);
        throw new Error(message);
      }
      setSelectedDocumentKey("");
      setPreviewExpanded(false);
      setEditMode(false);
      setEditDraft("");
      setEditError("");
      setDeleteConfirm(null);
      toast("ok", "Document deleted.");
      await queryClient.invalidateQueries({ queryKey: ["knowledgebase", "collections"] });
      await queryClient.invalidateQueries({ queryKey: ["knowledgebase", "documents"] });
      await documentsQuery.refetch();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast("err", message);
    }
  };

  if (!hostedManaged) return null;

  return (
    <PanelCard
      className="overflow-hidden"
      title="Knowledgebase"
      subtitle="Provisioned-server docs live in KB collections searched through MCP, not in the file buckets below."
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
          <button
            type="button"
            className="tcp-btn"
            onClick={() => folderInputRef.current?.click()}
            disabled={isUploading}
          >
            <i data-lucide="folder-open"></i>
            Upload folder
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
              Collection ID
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
            <div className="tcp-subtle text-xs">
              This collection groups the docs the KB MCP searches. It is not a raw filesystem path.
            </div>
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
            <input
              ref={folderInputRef}
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
            <button
              type="button"
              className="tcp-btn h-10"
              onClick={() => folderInputRef.current?.click()}
              disabled={!currentCollection || isUploading}
              title="Select a local folder and upload all matching docs inside it"
            >
              <i data-lucide="folder-open"></i>
              Select folder
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
              No collections reported yet. Uploads can still succeed before the KB service lists the
              collection.
            </span>
          )}
        </div>

        <div className="rounded-xl border border-white/10 bg-black/20 p-3 text-sm">
          <div className="font-medium text-slate-100">How this works</div>
          <div className="tcp-subtle mt-1">
            The KB MCP reads from collections in the hosted knowledgebase service. It does not
            browse a raw server folder directly, so uploads here will not appear in the managed
            `Files` buckets below unless you upload them there separately.
          </div>
        </div>

        {rows.length ? (
          <div className="grid gap-2">
            <div className="flex items-center justify-between gap-3">
              <div className="tcp-subtle text-xs uppercase tracking-wide">Upload history</div>
              {completedRows.length ? (
                <button
                  type="button"
                  className="tcp-btn h-7 px-2 text-xs"
                  onClick={clearFinishedUploads}
                >
                  Clear finished
                </button>
              ) : null}
            </div>
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
                        {row.result?.collectionId || currentCollection || "collection"} •{" "}
                        {row.status}
                      </div>
                      {row.result?.docId ? (
                        <div className="tcp-subtle mt-1 truncate text-[11px]">
                          {row.result.docId}
                        </div>
                      ) : null}
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

        <div className="grid gap-2">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="tcp-subtle text-xs uppercase tracking-wide">Collection documents</div>
              <div className="tcp-subtle mt-1 text-xs">
                Showing what the selected KB collection currently contains.
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Badge tone="ghost">{documents.length}</Badge>
              <button
                type="button"
                className="tcp-btn h-8 px-3 text-xs"
                onClick={() => void documentsQuery.refetch()}
                disabled={!currentCollection}
              >
                <i data-lucide="refresh-cw"></i>
                Refresh docs
              </button>
            </div>
          </div>

          {!currentCollection ? (
            <div className="rounded-xl border border-white/10 bg-black/20 p-3 text-sm tcp-subtle">
              Pick or type a collection ID to inspect its documents.
            </div>
          ) : documentsQuery.isFetching && !documents.length ? (
            <div className="rounded-xl border border-white/10 bg-black/20 p-3 text-sm tcp-subtle">
              Loading documents from{" "}
              <span className="font-medium text-slate-200">{currentCollection}</span>.
            </div>
          ) : documents.length ? (
            <div className="grid gap-4">
              <div className="grid min-h-0 gap-3">
                <div className="flex flex-wrap items-center gap-2">
                  <input
                    className="tcp-input h-9 min-w-[220px] flex-1"
                    value={documentSearch}
                    onChange={(event) => setDocumentSearch(event.target.value)}
                    placeholder="Filter documents"
                    spellCheck={false}
                  />
                  <Badge tone="ghost">
                    {visibleDocuments.length}/{documents.length}
                  </Badge>
                </div>

                {visibleDocuments.length ? (
                  <div className="grid gap-2">
                    {visibleDocuments.map((document, index) => {
                      const docId = String(document.doc_id || document.id || "").trim();
                      const title = String(
                        document.title ||
                          document.filename ||
                          document.file_name ||
                          docId ||
                          `Document ${index + 1}`
                      ).trim();
                      const collection = String(document.collection_id || currentCollection).trim();
                      const updatedAt = Number(document.updated_at || document.created_at || 0);
                      const active = documentIdentity(document) === selectedDocumentKey;
                      return (
                        <motion.div key={docId || `${collection}-${title}-${index}`} layout>
                          <button
                            type="button"
                            className={`w-full rounded-xl border p-3 text-left text-sm transition ${
                              active
                                ? "border-sky-500/50 bg-sky-950/20"
                                : "border-white/10 bg-black/20 hover:border-white/20 hover:bg-black/30"
                            }`.trim()}
                            onClick={() => {
                              setSelectedDocumentKey(documentIdentity(document));
                              setPreviewExpanded(true);
                              setEditMode(false);
                              setEditError("");
                            }}
                            aria-expanded={active}
                          >
                            <div className="flex items-start justify-between gap-3">
                              <div className="min-w-0">
                                <div className="truncate font-medium text-slate-100">{title}</div>
                                <div className="tcp-subtle mt-1 truncate text-xs">
                                  {docId || `${collection}/${title}`}
                                </div>
                              </div>
                              <Badge tone={active ? "ok" : "info"}>{collection}</Badge>
                            </div>
                            <div className="mt-2 flex flex-wrap gap-2 text-xs tcp-subtle">
                              <span>Updated: {formatKbDate(updatedAt)}</span>
                              {document.content_type ? <span>{document.content_type}</span> : null}
                            </div>
                          </button>

                          <AnimatePresence initial={false}>
                            {active ? (
                              <motion.div
                                key={`${selectedDocumentKey}-expanded`}
                                initial={{ opacity: 0, height: 0, y: -8 }}
                                animate={{ opacity: 1, height: "auto", y: 0 }}
                                exit={{ opacity: 0, height: 0, y: -8 }}
                                transition={{ duration: 0.22, ease: "easeOut" }}
                                className="overflow-hidden"
                              >
                                <div className="mt-2">{selectedDocumentPanel}</div>
                              </motion.div>
                            ) : null}
                          </AnimatePresence>
                        </motion.div>
                      );
                    })}
                  </div>
                ) : (
                  <EmptyState
                    title="No documents match your filter"
                    text={`Try another search term or clear the filter to show the full collection.`}
                    action={
                      documentSearch ? (
                        <button
                          type="button"
                          className="tcp-btn h-8 px-3 text-xs"
                          onClick={() => setDocumentSearch("")}
                        >
                          Clear filter
                        </button>
                      ) : null
                    }
                  />
                )}
              </div>
            </div>
          ) : (
            <div className="rounded-xl border border-white/10 bg-black/20 p-3 text-sm tcp-subtle">
              No visible documents returned for{" "}
              <span className="font-medium text-slate-200">{currentCollection}</span>. If you just
              uploaded a file, refresh docs or reindex and confirm the KB admin service exposes
              document listing for that collection.
            </div>
          )}
        </div>

        <ConfirmDialog
          open={!!deleteConfirm}
          title="Delete document"
          message={
            <span>
              This will permanently remove{" "}
              <strong>{deleteConfirm?.title || "this document"}</strong> from{" "}
              <strong>
                {deleteConfirm?.collection || currentCollection || "the selected collection"}
              </strong>
              .
            </span>
          }
          confirmLabel="Delete document"
          confirmIcon="trash-2"
          confirmTone="danger"
          onCancel={() => setDeleteConfirm(null)}
          onConfirm={() => void confirmDeleteSelectedDocument()}
        />
      </div>
    </PanelCard>
  );
}
