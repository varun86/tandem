import { useState, useEffect, useCallback } from "react";
import { client } from "../api";
import {
  MessageCircle,
  RefreshCw,
  AlertCircle,
  CheckCircle,
  XCircle,
  Loader2,
  Eye,
  EyeOff,
  Trash2,
} from "lucide-react";

type Channel = "telegram" | "discord" | "slack";

interface ChannelStatus {
  connected?: boolean;
  sessions?: number;
  error?: string;
}

const CHANNEL_META: Record<
  Channel,
  {
    label: string;
    icon: string;
    color: string;
    fields: { key: string; label: string; placeholder: string; secret?: boolean }[];
  }
> = {
  telegram: {
    label: "Telegram",
    icon: "✈️",
    color: "sky",
    fields: [
      { key: "bot_token", label: "Bot Token", placeholder: "123456:ABC-DEF...", secret: true },
      {
        key: "allowed_users",
        label: "Allowed Users",
        placeholder: "@alice,@bob  (or * for anyone)",
      },
    ],
  },
  discord: {
    label: "Discord",
    icon: "💬",
    color: "indigo",
    fields: [
      { key: "bot_token", label: "Bot Token", placeholder: "MTI3NTM2...", secret: true },
      { key: "guild_id", label: "Guild ID (optional)", placeholder: "123456789" },
      { key: "allowed_users", label: "Allowed User IDs", placeholder: "1234567890,0987654321" },
    ],
  },
  slack: {
    label: "Slack",
    icon: "⚡",
    color: "yellow",
    fields: [
      { key: "bot_token", label: "Bot Token", placeholder: "xoxb-...", secret: true },
      { key: "channel_id", label: "Channel ID", placeholder: "C0123ABCDEF" },
      { key: "allowed_users", label: "Allowed User IDs", placeholder: "U0123,U0456" },
    ],
  },
};

function SecretInput({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  const [show, setShow] = useState(false);
  return (
    <div className="relative">
      <input
        type={show ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full bg-gray-800 border border-gray-700 rounded-xl pl-3 pr-10 py-2 text-sm text-gray-200 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-purple-500/40 font-mono"
      />
      <button
        type="button"
        onClick={() => setShow((s) => !s)}
        className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300"
      >
        {show ? <EyeOff size={14} /> : <Eye size={14} />}
      </button>
    </div>
  );
}

function ChannelCard({
  channel,
  status,
  onRefresh,
}: {
  channel: Channel;
  status: ChannelStatus | undefined;
  onRefresh: () => void;
}) {
  const meta = CHANNEL_META[channel];
  const [expanded, setExpanded] = useState(!status?.connected);
  const [fields, setFields] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const colorMap: Record<string, string> = {
    sky: "text-sky-400 bg-sky-400/10 border-sky-800/40",
    indigo: "text-indigo-400 bg-indigo-400/10 border-indigo-800/40",
    yellow: "text-yellow-400 bg-yellow-400/10 border-yellow-800/40",
  };
  const indicatorColor = colorMap[meta.color] || "text-gray-400 bg-gray-800 border-gray-700";

  const save = async () => {
    setSaving(true);
    setError(null);
    setSuccess(null);
    try {
      await client.config.channels.upsert(channel, fields as Record<string, unknown>);
      setSuccess("Saved! Restart the channel listener to apply changes.");
      setExpanded(false);
      onRefresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const remove = async () => {
    setRemoving(true);
    setError(null);
    try {
      await client.config.channels.remove(channel);
      onRefresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRemoving(false);
    }
  };

  return (
    <div className="bg-gray-900/60 border border-gray-800 rounded-2xl overflow-hidden">
      <button
        onClick={() => setExpanded((o) => !o)}
        className="w-full flex items-center gap-3 px-4 py-4 hover:bg-gray-800/30 transition-colors text-left"
      >
        <span className="text-xl shrink-0">{meta.icon}</span>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold text-gray-100">{meta.label}</p>
          {status?.connected ? (
            <p className="text-xs text-gray-400 mt-0.5">
              {status.sessions !== undefined
                ? `${status.sessions} active session${status.sessions !== 1 ? "s" : ""}`
                : "Connected"}
            </p>
          ) : (
            <p className="text-xs text-gray-600 mt-0.5">Not connected</p>
          )}
        </div>
        <div
          className={`flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-full border ${indicatorColor}`}
        >
          {status?.connected ? (
            <>
              <CheckCircle size={12} />
              Connected
            </>
          ) : status?.error ? (
            <>
              <XCircle size={12} />
              Error
            </>
          ) : (
            <>
              <AlertCircle size={12} />
              Not set up
            </>
          )}
        </div>
      </button>

      {status?.error && (
        <div className="px-4 pb-3 text-xs text-rose-400 bg-rose-900/10 border-t border-rose-800/30 py-2">
          Error: {status.error}
        </div>
      )}

      {expanded && (
        <div className="border-t border-gray-800 px-4 py-4 space-y-3">
          <p className="text-xs text-gray-500">
            Configure your {meta.label} bot credentials. Tokens are stored in the engine and never
            logged.
          </p>
          {meta.fields.map((f) => (
            <div key={f.key}>
              <label className="block text-xs text-gray-400 mb-1">{f.label}</label>
              {f.secret ? (
                <SecretInput
                  value={fields[f.key] || ""}
                  onChange={(v) => setFields((p) => ({ ...p, [f.key]: v }))}
                  placeholder={f.placeholder}
                />
              ) : (
                <input
                  value={fields[f.key] || ""}
                  onChange={(e) => setFields((p) => ({ ...p, [f.key]: e.target.value }))}
                  placeholder={f.placeholder}
                  className="w-full bg-gray-800 border border-gray-700 rounded-xl px-3 py-2 text-sm text-gray-200 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-purple-500/40"
                />
              )}
            </div>
          ))}
          {error && <p className="text-xs text-rose-400">{error}</p>}
          {success && <p className="text-xs text-emerald-400">{success}</p>}
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={() => void save()}
              disabled={saving}
              className="flex-1 py-2 rounded-xl bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white text-sm font-medium transition-colors"
            >
              {saving ? "Saving…" : status?.connected ? "Update" : "Enable"}
            </button>
            {status?.connected && (
              <button
                onClick={() => void remove()}
                disabled={removing}
                className="px-3 py-2 rounded-xl border border-gray-700 text-gray-400 hover:text-rose-400 hover:border-rose-800/50 text-sm transition-colors"
              >
                {removing ? <Loader2 size={14} className="animate-spin" /> : <Trash2 size={14} />}
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default function Channels() {
  const [status, setStatus] = useState<Record<string, ChannelStatus> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await client.config.channels.status();
      setStatus(s as Record<string, ChannelStatus>);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="h-full overflow-y-auto bg-gray-950">
      <div className="max-w-2xl mx-auto px-4 py-8 space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-white flex items-center gap-2">
              <MessageCircle className="text-purple-400" size={22} />
              Channels
            </h1>
            <p className="text-sm text-gray-400 mt-1">
              Connect Telegram, Discord, or Slack to chat with your agents.
            </p>
          </div>
          <button
            onClick={() => void load()}
            className="p-2 rounded-lg hover:bg-gray-800 text-gray-400 hover:text-white transition-colors"
          >
            <RefreshCw size={16} className={loading ? "animate-spin" : ""} />
          </button>
        </div>

        {error && (
          <div className="flex items-center gap-2 text-rose-400 bg-rose-900/20 border border-rose-800/40 rounded-xl px-4 py-3 text-sm">
            <AlertCircle size={14} />
            {error}
          </div>
        )}

        {loading ? (
          <div className="flex justify-center py-12">
            <Loader2 size={24} className="animate-spin text-gray-600" />
          </div>
        ) : (
          <div className="space-y-4">
            {(["telegram", "discord", "slack"] as Channel[]).map((ch) => (
              <ChannelCard
                key={ch}
                channel={ch}
                status={status?.[ch]}
                onRefresh={() => void load()}
              />
            ))}
          </div>
        )}

        <div className="bg-gray-900/40 border border-gray-800/60 rounded-xl p-4 text-xs text-gray-500 space-y-1">
          <p className="font-medium text-gray-400">How channels work</p>
          <p>
            Each incoming message maps to a per-user Tandem session. Your agent responds with the
            same tools it uses in chat — web search, file access, memory, and more.
          </p>
          <p className="mt-2">
            Use <span className="font-mono text-gray-400">/new</span>,{" "}
            <span className="font-mono text-gray-400">/sessions</span>,{" "}
            <span className="font-mono text-gray-400">/resume</span>, and{" "}
            <span className="font-mono text-gray-400">/help</span> from within Telegram/Discord to
            manage sessions.
          </p>
        </div>
      </div>
    </div>
  );
}
