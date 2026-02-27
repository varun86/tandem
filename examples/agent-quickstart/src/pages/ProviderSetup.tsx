import { useState, useEffect } from "react";
import { useAuth } from "../AuthContext";
import { api, type ProviderCatalog } from "../api";
import { Settings2, CheckCircle, AlertCircle, Eye, EyeOff, Loader2, ArrowRight } from "lucide-react";

interface Props { onDone?: () => void; }

/* Well-known providers with user-friendly labels */
const PROVIDER_HINTS: Record<string, { label: string; keyUrl: string; placeholder: string }> = {
    openai: { label: "OpenAI", keyUrl: "https://platform.openai.com/api-keys", placeholder: "sk-proj-…" },
    anthropic: { label: "Anthropic", keyUrl: "https://console.anthropic.com/settings/keys", placeholder: "sk-ant-…" },
    google: { label: "Google / Gemini", keyUrl: "https://aistudio.google.com/app/apikey", placeholder: "AIza…" },
    groq: { label: "Groq", keyUrl: "https://console.groq.com/keys", placeholder: "gsk_…" },
    mistral: { label: "Mistral", keyUrl: "https://console.mistral.ai/api-keys/", placeholder: "…" },
    ollama: { label: "Ollama (local)", keyUrl: "", placeholder: "ollama (no key needed)" },
};

export default function ProviderSetup({ onDone }: Props) {
    const { providerConfigured } = useAuth();
    const [catalog, setCatalog] = useState<ProviderCatalog | null>(null);
    const [selectedProvider, setSelectedProvider] = useState("");
    const [selectedModel, setSelectedModel] = useState("");
    const [apiKey, setApiKey] = useState("");
    const [showKey, setShowKey] = useState(false);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);

    useEffect(() => {
        api.getProviderCatalog()
            .then((c) => {
                setCatalog(c);
                // Auto-select first connected provider
                const first = (c.connected || [])[0] || (c.all || [])[0]?.id || "";
                setSelectedProvider(first);
                if (first) {
                    const entry = (c.all || []).find((e) => e.id === first);
                    const models = Object.keys(entry?.models || {});
                    setSelectedModel(models[0] || "");
                }
            })
            .catch(() => { /* ignore */ });
    }, []);

    const handleProviderChange = (pid: string) => {
        setSelectedProvider(pid);
        const entry = (catalog?.all || []).find((e) => e.id === pid);
        setSelectedModel(Object.keys(entry?.models || {})[0] || "");
        setApiKey("");
    };

    const save = async () => {
        if (!selectedProvider || !selectedModel) { setError("Select a provider and model."); return; }
        setSaving(true); setError(null); setSuccess(false);
        try {
            if (apiKey.trim()) await api.setProviderAuth(selectedProvider, apiKey.trim());
            await api.setProviderDefaults(selectedProvider, selectedModel);
            setSuccess(true);
            onDone?.();
        } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
        finally { setSaving(false); }
    };

    const hint = PROVIDER_HINTS[selectedProvider];
    const entry = (catalog?.all || []).find((e) => e.id === selectedProvider);
    const models = Object.keys(entry?.models || {});
    const connected = new Set(catalog?.connected || []);

    return (
        <div className="h-full overflow-y-auto bg-gray-950">
            <div className="max-w-lg mx-auto px-4 py-8 space-y-6">
                <div>
                    <h1 className="text-2xl font-bold text-white flex items-center gap-2">
                        <Settings2 className="text-blue-400" size={22} />Provider Setup
                    </h1>
                    <p className="text-sm text-gray-400 mt-1">Configure the AI provider and model your agents will use.</p>
                </div>

                {providerConfigured && (
                    <div className="flex items-center gap-2 text-emerald-400 bg-emerald-900/20 border border-emerald-800/40 rounded-xl px-4 py-3 text-sm">
                        <CheckCircle size={14} />Provider configured — change it below if needed.
                    </div>
                )}

                {!catalog ? (
                    <div className="flex justify-center py-12"><Loader2 size={22} className="animate-spin text-gray-600" /></div>
                ) : (
                    <div className="space-y-4">
                        {/* Provider selector */}
                        <div>
                            <label className="block text-sm text-gray-400 mb-2">Provider</label>
                            <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
                                {(catalog.all || []).map((e) => {
                                    const h = PROVIDER_HINTS[e.id];
                                    const label = h?.label || e.name || e.id;
                                    const isConn = connected.has(e.id);
                                    return (
                                        <button
                                            key={e.id}
                                            onClick={() => handleProviderChange(e.id)}
                                            className={`flex items-center justify-between gap-1.5 rounded-xl border px-3 py-2.5 text-sm transition-colors text-left ${selectedProvider === e.id ? "border-blue-500/50 bg-blue-500/10 text-blue-200" : "border-gray-700 text-gray-400 hover:bg-gray-800 hover:text-gray-200"}`}
                                        >
                                            <span className="truncate">{label}</span>
                                            {isConn && <CheckCircle size={12} className="text-emerald-400 shrink-0" />}
                                        </button>
                                    );
                                })}
                            </div>
                        </div>

                        {/* Model selector */}
                        {models.length > 0 && (
                            <div>
                                <label className="block text-sm text-gray-400 mb-2">Model</label>
                                <select
                                    value={selectedModel}
                                    onChange={(e) => setSelectedModel(e.target.value)}
                                    className="w-full bg-gray-800 border border-gray-700 rounded-xl px-3 py-2.5 text-sm text-gray-200 focus:outline-none focus:ring-2 focus:ring-blue-500/40"
                                >
                                    {models.map((m) => <option key={m} value={m}>{m}</option>)}
                                </select>
                            </div>
                        )}

                        {/* API key */}
                        {hint && hint.keyUrl !== "" && (
                            <div>
                                <label className="block text-sm text-gray-400 mb-1">
                                    API Key
                                    {hint.keyUrl && (
                                        <a href={hint.keyUrl} target="_blank" rel="noopener noreferrer" className="ml-2 text-blue-400 hover:text-blue-300 text-xs">
                                            Get key →
                                        </a>
                                    )}
                                </label>
                                <div className="relative">
                                    <input
                                        type={showKey ? "text" : "password"}
                                        value={apiKey}
                                        onChange={(e) => setApiKey(e.target.value)}
                                        placeholder={hint?.placeholder || "API key…"}
                                        className="w-full bg-gray-800 border border-gray-700 rounded-xl pl-3 pr-10 py-2.5 text-sm text-gray-200 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500/40 font-mono"
                                    />
                                    <button type="button" onClick={() => setShowKey((s) => !s)} className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300">
                                        {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
                                    </button>
                                </div>
                            </div>
                        )}

                        {error && (
                            <div className="flex items-start gap-2 text-rose-400 bg-rose-900/20 border border-rose-800/40 rounded-xl px-3 py-2 text-sm">
                                <AlertCircle size={13} className="mt-0.5 shrink-0" />{error}
                            </div>
                        )}
                        {success && (
                            <div className="flex items-center gap-2 text-emerald-400 bg-emerald-900/20 border border-emerald-800/40 rounded-xl px-3 py-2.5 text-sm">
                                <CheckCircle size={14} />Provider saved! Ready to chat.
                            </div>
                        )}

                        <button
                            onClick={() => void save()} disabled={saving || !selectedProvider || !selectedModel}
                            className="w-full flex items-center justify-center gap-2 py-3 rounded-xl bg-blue-600 hover:bg-blue-500 disabled:bg-gray-700 disabled:text-gray-500 text-white font-semibold text-sm transition-colors"
                        >
                            {saving ? <Loader2 size={16} className="animate-spin" /> : <ArrowRight size={16} />}
                            {saving ? "Saving…" : "Save Provider"}
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
