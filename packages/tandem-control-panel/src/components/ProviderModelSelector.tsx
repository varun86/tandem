type ProviderOption = {
  id: string;
  models: string[];
  configured?: boolean;
};

type ModelDraft = {
  provider: string;
  model: string;
};

function mergeOptionValues(values: string[], currentValue: string) {
  const seen = new Set<string>();
  const merged: string[] = [];
  for (const raw of [currentValue, ...values]) {
    const value = String(raw || "").trim();
    if (!value || seen.has(value)) continue;
    seen.add(value);
    merged.push(value);
  }
  return merged;
}

export function ProviderModelSelector({
  providerLabel,
  modelLabel,
  draft,
  providers,
  onChange,
  inheritLabel = "Workspace default",
  disabled = false,
}: {
  providerLabel: string;
  modelLabel: string;
  draft: ModelDraft;
  providers: ProviderOption[];
  onChange: (draft: ModelDraft) => void;
  inheritLabel?: string;
  disabled?: boolean;
}) {
  const modelOptions = providers.find((provider) => provider.id === draft.provider)?.models || [];
  return (
    <div className="grid gap-3 md:grid-cols-2">
      <label className="block text-sm">
        <div className="mb-1 flex items-center gap-2 font-medium text-slate-200">
          <i data-lucide="cpu"></i>
          <span>{providerLabel}</span>
        </div>
        <select
          value={draft.provider}
          onInput={(event) => {
            const provider = (event.target as HTMLSelectElement).value;
            const nextModels = providers.find((row) => row.id === provider)?.models || [];
            onChange({ provider, model: nextModels[0] || "" });
          }}
          className="tcp-select h-10 w-full"
          disabled={disabled}
        >
          <option value="">{inheritLabel}</option>
          {mergeOptionValues(
            providers.map((provider) => provider.id),
            draft.provider
          ).map((providerId) => (
            <option key={providerId} value={providerId}>
              {providerId}
              {providers.find((provider) => provider.id === providerId)?.configured === false
                ? " (not configured)"
                : ""}
            </option>
          ))}
        </select>
      </label>
      <label className="block text-sm">
        <div className="mb-1 flex items-center gap-2 font-medium text-slate-200">
          <i data-lucide="sparkles"></i>
          <span>{modelLabel}</span>
        </div>
        <select
          value={draft.model}
          onInput={(event) =>
            onChange({ ...draft, model: (event.target as HTMLSelectElement).value })
          }
          className="tcp-select h-10 w-full"
          disabled={disabled || !draft.provider}
        >
          <option value="">{draft.provider ? "Select a model" : inheritLabel}</option>
          {mergeOptionValues(modelOptions, draft.model).map((model) => (
            <option key={model} value={model}>
              {model}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}
