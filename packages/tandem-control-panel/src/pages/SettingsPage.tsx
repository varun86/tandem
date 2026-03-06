import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import {
  AnimatedPage,
  Badge,
  DetailDrawer,
  PanelCard,
  SplitView,
  StaggerGroup,
  Toolbar,
} from "../ui/index.tsx";
import { ThemePicker } from "../ui/ThemePicker.tsx";
import { EmptyState } from "./ui";
import type { AppPageProps } from "./pageTypes";

type BrowserBlockingIssue = {
  code?: string;
  message?: string;
};

type BrowserBinaryStatus = {
  found?: boolean;
  path?: string | null;
  version?: string | null;
  channel?: string | null;
};

type BrowserStatusResponse = {
  enabled?: boolean;
  runnable?: boolean;
  headless_default?: boolean;
  sidecar?: BrowserBinaryStatus;
  browser?: BrowserBinaryStatus;
  blocking_issues?: BrowserBlockingIssue[];
  recommendations?: string[];
  install_hints?: string[];
  last_error?: string | null;
};

function providerCatalogBadge(provider: any, modelCount: number) {
  const source = String(provider?.catalog_source || "")
    .trim()
    .toLowerCase();
  if (source === "remote" && modelCount > 0) {
    return { tone: "ok" as const, text: `${modelCount} models` };
  }
  if (source === "config" && modelCount > 0) {
    return { tone: "info" as const, text: "configured models" };
  }
  return { tone: "warn" as const, text: "manual entry" };
}

function providerCatalogSubtitle(provider: any, defaultModel: string) {
  const catalogMessage = String(provider?.catalog_message || "").trim();
  if (catalogMessage) return catalogMessage;
  return `Default model: ${defaultModel || "none"}`;
}

export function SettingsPage({
  client,
  api,
  toast,
  identity,
  themes,
  setTheme,
  themeId,
  refreshProviderStatus,
  refreshIdentityStatus,
}: AppPageProps) {
  const queryClient = useQueryClient();
  const [modelSearchByProvider, setModelSearchByProvider] = useState<Record<string, string>>({});
  const [botName, setBotName] = useState(String(identity?.botName || "Tandem"));
  const [botAvatarUrl, setBotAvatarUrl] = useState(String(identity?.botAvatarUrl || ""));
  const [botControlPanelAlias, setBotControlPanelAlias] = useState("Control Center");
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [providerDefaultsOpen, setProviderDefaultsOpen] = useState(false);
  const avatarInputRef = useRef<HTMLInputElement | null>(null);

  const loadIdentityConfig = async () => {
    const identityApi = (client as any)?.identity;
    if (identityApi?.get) return identityApi.get();
    return api("/api/engine/config/identity", { method: "GET" });
  };

  const patchIdentityConfig = async (payload: any) => {
    const identityApi = (client as any)?.identity;
    if (identityApi?.patch) return identityApi.patch(payload);
    return api("/api/engine/config/identity", {
      method: "PATCH",
      body: JSON.stringify(payload),
    });
  };

  const identityConfig = useQuery({
    queryKey: ["settings", "identity", "config"],
    queryFn: () => loadIdentityConfig().catch(() => ({ identity: {} as any })),
  });

  useEffect(() => {
    const bot = (identityConfig.data as any)?.identity?.bot || {};
    const aliases = bot?.aliases || {};
    const canonical = String(
      bot?.canonicalName || bot?.canonical_name || identity?.botName || "Tandem"
    ).trim();
    const avatar = String(bot?.avatarUrl || bot?.avatar_url || identity?.botAvatarUrl || "").trim();
    const controlPanelAlias = String(aliases?.controlPanel || aliases?.control_panel || "").trim();
    setBotName(canonical || "Tandem");
    setBotAvatarUrl(avatar);
    setBotControlPanelAlias(controlPanelAlias || "Control Center");
  }, [identity?.botAvatarUrl, identity?.botName, identityConfig.data]);

  const providersCatalog = useQuery({
    queryKey: ["settings", "providers", "catalog"],
    queryFn: () => client.providers.catalog().catch(() => ({ all: [], connected: [] })),
  });

  const providersConfig = useQuery({
    queryKey: ["settings", "providers", "config"],
    queryFn: () => client.providers.config().catch(() => ({ default: "", providers: {} })),
  });

  const browserStatus = useQuery<BrowserStatusResponse | null>({
    queryKey: ["settings", "browser", "status"],
    queryFn: () => api("/api/engine/browser/status", { method: "GET" }).catch(() => null),
    refetchInterval: 30_000,
  });

  const setDefaultsMutation = useMutation({
    mutationFn: async ({ providerId, modelId }: { providerId: string; modelId: string }) =>
      client.providers.setDefaults(providerId, modelId),
    onSuccess: async () => {
      toast("ok", "Updated provider defaults.");
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["settings", "providers"] }),
        refreshProviderStatus(),
      ]);
    },
    onError: (error) => toast("err", error instanceof Error ? error.message : String(error)),
  });

  const setApiKeyMutation = useMutation({
    mutationFn: ({ providerId, apiKey }: { providerId: string; apiKey: string }) =>
      client.providers.setApiKey(providerId, apiKey),
    onSuccess: async () => {
      toast("ok", "API key updated.");
      await refreshProviderStatus();
    },
    onError: (error) => toast("err", error instanceof Error ? error.message : String(error)),
  });

  const saveIdentityMutation = useMutation({
    mutationFn: async () => {
      const currentBot = (identityConfig.data as any)?.identity?.bot || {};
      const currentAliases = currentBot?.aliases || {};
      const canonical = String(botName || "").trim();
      if (!canonical) throw new Error("Bot name is required.");
      const avatar = String(botAvatarUrl || "").trim();
      const controlPanelAlias = String(botControlPanelAlias || "").trim();
      return patchIdentityConfig({
        identity: {
          bot: {
            canonical_name: canonical,
            avatar_url: avatar || null,
            aliases: {
              ...currentAliases,
              control_panel: controlPanelAlias || undefined,
            },
          },
        },
      } as any);
    },
    onSuccess: async () => {
      toast("ok", "Identity updated.");
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["settings", "identity"] }),
        refreshIdentityStatus(),
      ]);
    },
    onError: (error) => toast("err", error instanceof Error ? error.message : String(error)),
  });

  const handleAvatarUpload = (file: File | null) => {
    if (!file) return;
    if (file.size > 10 * 1024 * 1024) {
      toast("err", "Avatar image is too large (max 10 MB).");
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      const value = typeof reader.result === "string" ? reader.result : "";
      if (!value) {
        toast("err", "Failed to read avatar image.");
        return;
      }
      setBotAvatarUrl(value);
    };
    reader.onerror = () => toast("err", "Failed to read avatar image.");
    reader.readAsDataURL(file);
  };

  const providers = Array.isArray(providersCatalog.data?.all) ? providersCatalog.data.all : [];
  const browserIssues = Array.isArray(browserStatus.data?.blocking_issues)
    ? browserStatus.data?.blocking_issues || []
    : [];
  const browserRecommendations = Array.isArray(browserStatus.data?.recommendations)
    ? browserStatus.data?.recommendations || []
    : [];
  const browserInstallHints = Array.isArray(browserStatus.data?.install_hints)
    ? browserStatus.data?.install_hints || []
    : [];

  const applyDefaultModel = (providerId: string, modelId: string) => {
    const next = String(modelId || "").trim();
    if (!next) return;
    setDefaultsMutation.mutate({ providerId, modelId: next });
  };

  return (
    <AnimatedPage className="grid gap-4">
      <SplitView
        main={
          <StaggerGroup className="grid gap-4">
            <PanelCard
              title="Provider defaults"
              subtitle="Provider catalog, model selection, and API key entry."
              actions={
                <div className="flex flex-wrap items-center justify-end gap-2">
                  <Badge tone={String(providersConfig.data?.default || "").trim() ? "ok" : "warn"}>
                    Default: {String(providersConfig.data?.default || "none")}
                  </Badge>
                  <Badge tone={browserStatus.data?.runnable ? "ok" : "warn"}>
                    Browser:{" "}
                    {browserStatus.data
                      ? browserStatus.data.runnable
                        ? "ready"
                        : "attention"
                      : "unknown"}
                  </Badge>
                  <Badge tone="info">
                    {String(providersCatalog.data?.connected?.length || 0)} connected
                  </Badge>
                  <button className="tcp-btn" onClick={() => setDiagnosticsOpen(true)}>
                    <i data-lucide="activity"></i>
                    Diagnostics
                  </button>
                  <button
                    className="tcp-btn"
                    onClick={() =>
                      refreshProviderStatus().then(() => toast("ok", "Provider status refreshed."))
                    }
                  >
                    <i data-lucide="refresh-cw"></i>
                    Refresh provider
                  </button>
                  <button
                    className="tcp-btn-primary"
                    onClick={() => saveIdentityMutation.mutate()}
                    disabled={saveIdentityMutation.isPending}
                  >
                    <i data-lucide="save"></i>
                    Save identity
                  </button>
                </div>
              }
            >
              <div className="grid gap-3">
                <button
                  type="button"
                  className="tcp-list-item text-left"
                  onClick={() => setProviderDefaultsOpen((prev) => !prev)}
                  aria-expanded={providerDefaultsOpen}
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="font-medium inline-flex items-center gap-2">
                        <i
                          data-lucide={providerDefaultsOpen ? "chevron-down" : "chevron-right"}
                        ></i>
                        <span>
                          {providerDefaultsOpen ? "Hide provider catalog" : "Show provider catalog"}
                        </span>
                      </div>
                      <div className="tcp-subtle mt-1 text-xs">
                        {providers.length} providers available for configuration. Expand to change
                        models and API keys.
                      </div>
                    </div>
                    <Badge tone="info">
                      {String(providersCatalog.data?.connected?.length || 0)} connected
                    </Badge>
                  </div>
                </button>

                <AnimatePresence initial={false}>
                  {providerDefaultsOpen ? (
                    <motion.div
                      className="grid gap-3"
                      initial={{ opacity: 0, y: -8 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -8 }}
                    >
                      {providers.length ? (
                        providers.map((provider: any) => {
                          const providerId = String(provider?.id || "");
                          const models = Object.keys(provider?.models || {});
                          const defaultModel = String(
                            providersConfig.data?.providers?.[providerId]?.default_model ||
                              models[0] ||
                              ""
                          );
                          const typedModel = String(
                            modelSearchByProvider[providerId] ?? defaultModel
                          ).trim();
                          const normalizedTyped = typedModel.toLowerCase();
                          const filteredModels = models
                            .filter((modelId) =>
                              normalizedTyped
                                ? modelId.toLowerCase().includes(normalizedTyped)
                                : true
                            )
                            .slice(0, 80);
                          const badge = providerCatalogBadge(provider, models.length);
                          const subtitle = providerCatalogSubtitle(provider, defaultModel);

                          return (
                            <motion.details key={providerId} layout className="tcp-list-item">
                              <summary className="cursor-pointer list-none">
                                <div className="flex items-center justify-between gap-3">
                                  <div>
                                    <div className="font-medium">{providerId}</div>
                                    <div className="tcp-subtle text-xs">{subtitle}</div>
                                  </div>
                                  <Badge tone={badge.tone}>{badge.text}</Badge>
                                </div>
                              </summary>
                              <div className="mt-3 grid gap-3">
                                <form
                                  className="grid gap-2"
                                  onSubmit={(e) => {
                                    e.preventDefault();
                                    applyDefaultModel(providerId, typedModel);
                                  }}
                                >
                                  <div className="flex gap-2">
                                    <input
                                      className="tcp-input"
                                      value={typedModel}
                                      placeholder={`Type model id for ${providerId}`}
                                      onInput={(e) =>
                                        setModelSearchByProvider((prev) => ({
                                          ...prev,
                                          [providerId]: (e.target as HTMLInputElement).value,
                                        }))
                                      }
                                    />
                                    <button className="tcp-btn" type="submit">
                                      <i data-lucide="badge-check"></i>
                                      Apply
                                    </button>
                                  </div>
                                  <div className="max-h-48 overflow-auto rounded-xl border border-slate-700/60 bg-slate-900/20 p-1">
                                    {filteredModels.length ? (
                                      filteredModels.map((modelId) => (
                                        <button
                                          key={modelId}
                                          type="button"
                                          className={`block w-full rounded-lg px-2 py-1.5 text-left text-sm hover:bg-slate-700/30 ${
                                            modelId === defaultModel ? "bg-slate-700/40" : ""
                                          }`}
                                          onClick={() => {
                                            setModelSearchByProvider((prev) => ({
                                              ...prev,
                                              [providerId]: modelId,
                                            }));
                                            applyDefaultModel(providerId, modelId);
                                          }}
                                        >
                                          {modelId}
                                        </button>
                                      ))
                                    ) : (
                                      <div className="tcp-subtle px-2 py-1 text-xs">
                                        {models.length
                                          ? "No matching models."
                                          : "No live catalog available. Type a model ID manually."}
                                      </div>
                                    )}
                                  </div>
                                </form>

                                <form
                                  onSubmit={(e) => {
                                    e.preventDefault();
                                    const input = e.currentTarget.elements.namedItem(
                                      "apiKey"
                                    ) as HTMLInputElement;
                                    const value = String(input?.value || "").trim();
                                    if (!value) return;
                                    setApiKeyMutation.mutate({ providerId, apiKey: value });
                                    input.value = "";
                                  }}
                                  className="flex gap-2"
                                >
                                  <input
                                    name="apiKey"
                                    className="tcp-input"
                                    placeholder={`Set ${providerId} API key`}
                                  />
                                  <button className="tcp-btn" type="submit">
                                    <i data-lucide="save"></i>
                                    Save
                                  </button>
                                </form>
                              </div>
                            </motion.details>
                          );
                        })
                      ) : (
                        <EmptyState text="No providers were detected from the engine catalog." />
                      )}
                    </motion.div>
                  ) : null}
                </AnimatePresence>
              </div>
            </PanelCard>

            <PanelCard
              title="Theme studio"
              subtitle="Preview tiles with richer feedback and immediate switching."
            >
              <ThemePicker themes={themes} themeId={themeId} onChange={setTheme} />
            </PanelCard>
          </StaggerGroup>
        }
        aside={
          <div className="grid gap-4">
            <PanelCard
              title="Identity preview"
              subtitle="Live preview of how the assistant appears across the panel."
            >
              <div className="grid gap-3">
                <div className="rounded-2xl border border-slate-700/60 bg-slate-950/25 p-4">
                  <div className="flex items-center justify-between gap-3">
                    <div className="inline-flex items-center gap-3">
                      <span className="tcp-brand-avatar inline-grid h-12 w-12 rounded-xl">
                        {botAvatarUrl ? (
                          <img
                            src={botAvatarUrl}
                            alt={botName || "Bot"}
                            className="block h-full w-full object-cover"
                          />
                        ) : (
                          <i data-lucide="cpu"></i>
                        )}
                      </span>
                      <div>
                        <div className="font-semibold">{botName || "Tandem"}</div>
                        <div className="tcp-subtle text-xs">
                          {botControlPanelAlias || "Control Center"}
                        </div>
                      </div>
                    </div>
                    <Toolbar>
                      <button
                        className="tcp-icon-btn"
                        title="Upload avatar"
                        aria-label="Upload avatar"
                        onClick={() => avatarInputRef.current?.click()}
                      >
                        <i data-lucide="pencil"></i>
                      </button>
                      <button
                        className="tcp-icon-btn"
                        title="Clear avatar"
                        aria-label="Clear avatar"
                        onClick={() => setBotAvatarUrl("")}
                      >
                        <i data-lucide="trash-2"></i>
                      </button>
                    </Toolbar>
                  </div>
                </div>

                <input
                  className="tcp-input"
                  value={botName}
                  onInput={(e) => setBotName((e.target as HTMLInputElement).value)}
                  placeholder="Bot name"
                />
                <input
                  className="tcp-input"
                  value={botControlPanelAlias}
                  onInput={(e) => setBotControlPanelAlias((e.target as HTMLInputElement).value)}
                  placeholder="Control panel alias"
                />
                <input
                  className="tcp-input"
                  value={botAvatarUrl}
                  onInput={(e) => setBotAvatarUrl((e.target as HTMLInputElement).value)}
                  placeholder="Avatar URL or data URL"
                />
                <input
                  ref={avatarInputRef}
                  type="file"
                  accept="image/*"
                  className="hidden"
                  onChange={(e) =>
                    handleAvatarUpload((e.target as HTMLInputElement).files?.[0] || null)
                  }
                />

                <Toolbar>
                  <button
                    className="tcp-btn"
                    onClick={() =>
                      refreshIdentityStatus().then(() => toast("ok", "Identity refreshed."))
                    }
                  >
                    <i data-lucide="refresh-cw"></i>
                    Refresh identity
                  </button>
                  <button
                    className="tcp-btn-primary"
                    onClick={() => saveIdentityMutation.mutate()}
                    disabled={saveIdentityMutation.isPending}
                  >
                    <i data-lucide="save"></i>
                    Save
                  </button>
                </Toolbar>
              </div>
            </PanelCard>

            <PanelCard
              title="Readiness snapshot"
              subtitle="High-signal operational summary for this configuration state."
            >
              <div className="grid gap-2">
                <div className="tcp-list-item">
                  <div className="font-medium">Connected providers</div>
                  <div className="tcp-subtle mt-1 text-xs">
                    {String(providersCatalog.data?.connected?.length || 0)} connected, default{" "}
                    {String(providersConfig.data?.default || "none")}
                  </div>
                </div>
                <div className="tcp-list-item">
                  <div className="font-medium">Browser automation</div>
                  <div className="tcp-subtle mt-1 text-xs">
                    {browserStatus.data
                      ? browserStatus.data.runnable
                        ? "Ready"
                        : browserStatus.data.enabled
                          ? "Enabled but blocked"
                          : "Disabled"
                      : "Unknown"}
                  </div>
                </div>
                <div className="tcp-list-item">
                  <div className="font-medium">Theme</div>
                  <div className="tcp-subtle mt-1 text-xs">
                    {themes.find((theme: any) => theme.id === themeId)?.name || themeId}
                  </div>
                </div>
              </div>
            </PanelCard>
          </div>
        }
      />

      <DetailDrawer
        open={diagnosticsOpen}
        onClose={() => setDiagnosticsOpen(false)}
        title="Browser diagnostics"
      >
        <div className="grid gap-3">
          <div className="grid gap-2 md:grid-cols-3">
            <div className="tcp-list-item">
              <div className="text-sm font-medium">Status</div>
              <div className="mt-1 text-sm">
                {browserStatus.data
                  ? browserStatus.data.runnable
                    ? "Ready"
                    : browserStatus.data.enabled
                      ? "Blocked"
                      : "Disabled"
                  : "Unknown"}
              </div>
              <div className="tcp-subtle text-xs">
                Headless default: {browserStatus.data?.headless_default ? "yes" : "no"}
              </div>
            </div>
            <div className="tcp-list-item">
              <div className="text-sm font-medium">Sidecar</div>
              <div className="mt-1 break-all text-sm">
                {browserStatus.data?.sidecar?.path || "Not found"}
              </div>
              <div className="tcp-subtle text-xs">
                {browserStatus.data?.sidecar?.version || "No version detected"}
              </div>
            </div>
            <div className="tcp-list-item">
              <div className="text-sm font-medium">Browser</div>
              <div className="mt-1 break-all text-sm">
                {browserStatus.data?.browser?.path || "Not found"}
              </div>
              <div className="tcp-subtle text-xs">
                {browserStatus.data?.browser?.version ||
                  browserStatus.data?.browser?.channel ||
                  "No version detected"}
              </div>
            </div>
          </div>

          <Toolbar>
            <button className="tcp-btn" onClick={() => void browserStatus.refetch()}>
              <i data-lucide="refresh-cw"></i>
              Refresh browser status
            </button>
            <button
              className="tcp-btn"
              onClick={() =>
                api("/api/engine/browser/status", { method: "GET" })
                  .then(() => toast("ok", "Browser diagnostics refreshed."))
                  .catch((error) =>
                    toast("err", error instanceof Error ? error.message : String(error))
                  )
              }
            >
              <i data-lucide="activity"></i>
              Re-run diagnostics
            </button>
          </Toolbar>

          {browserStatus.isLoading ? (
            <EmptyState text="Loading browser diagnostics..." />
          ) : browserStatus.data ? (
            <>
              {browserIssues.length ? (
                <div className="grid gap-2">
                  <div className="text-sm font-medium">Blocking issues</div>
                  {browserIssues.map((issue, index) => (
                    <div key={`${issue.code || "issue"}-${index}`} className="tcp-list-item">
                      <div className="text-sm font-medium">{issue.code || "browser_issue"}</div>
                      <div className="tcp-subtle text-xs">
                        {issue.message || "Unknown browser issue."}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-3 text-sm">
                  Browser automation is ready on this machine.
                </div>
              )}

              {browserRecommendations.length ? (
                <div className="grid gap-2">
                  <div className="text-sm font-medium">Recommendations</div>
                  {browserRecommendations.map((row, index) => (
                    <div key={`browser-recommendation-${index}`} className="tcp-list-item text-sm">
                      {row}
                    </div>
                  ))}
                </div>
              ) : null}

              {browserInstallHints.length ? (
                <div className="grid gap-2">
                  <div className="text-sm font-medium">Install hints</div>
                  {browserInstallHints.map((row, index) => (
                    <div key={`browser-install-hint-${index}`} className="tcp-list-item text-sm">
                      {row}
                    </div>
                  ))}
                </div>
              ) : null}

              {browserStatus.data?.last_error ? (
                <div className="tcp-subtle rounded-lg border border-slate-700/60 bg-slate-900/20 p-3 text-xs">
                  Last error: {browserStatus.data.last_error}
                </div>
              ) : null}
            </>
          ) : (
            <EmptyState text="Browser diagnostics are unavailable." />
          )}
        </div>
      </DetailDrawer>
    </AnimatedPage>
  );
}
