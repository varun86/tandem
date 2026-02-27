import React, { useState } from "react";
import { useAuth } from "../AuthContext";
import { Zap, Eye, EyeOff, AlertCircle } from "lucide-react";
import { api } from "../api";

export default function Login() {
    const { login } = useAuth();
    const [token, setToken] = useState("");
    const [show, setShow] = useState(false);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        const t = token.trim();
        if (!t) return;
        setLoading(true);
        setError(null);
        // Verify token by calling /global/health
        api.setToken(t);
        try {
            await api.getHealth();
            login(t);
        } catch {
            setError("Could not connect to the engine with that token. Check your engine URL and key.");
            api.setToken("");
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="min-h-full flex items-center justify-center bg-gray-950 px-4">
            <div className="w-full max-w-sm">
                <div className="text-center mb-8">
                    <div className="inline-flex items-center justify-center w-14 h-14 rounded-2xl bg-violet-600/20 border border-violet-600/30 mb-4">
                        <Zap size={28} className="text-violet-400" />
                    </div>
                    <h1 className="text-2xl font-bold text-white">Tandem Agent</h1>
                    <p className="text-gray-400 text-sm mt-1">Enter your engine access token to continue.</p>
                </div>

                <form onSubmit={(e) => void handleSubmit(e)} className="space-y-4">
                    <div className="relative">
                        <input
                            type={show ? "text" : "password"}
                            value={token}
                            onChange={(e) => setToken(e.target.value)}
                            placeholder="Engine access token"
                            autoFocus
                            className="w-full bg-gray-800 border border-gray-700 rounded-xl pl-4 pr-12 py-3 text-sm text-gray-100 placeholder:text-gray-500 focus:outline-none focus:ring-2 focus:ring-violet-500/50 focus:border-violet-600/40 font-mono"
                        />
                        <button
                            type="button"
                            onClick={() => setShow((s) => !s)}
                            className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300"
                        >
                            {show ? <EyeOff size={16} /> : <Eye size={16} />}
                        </button>
                    </div>

                    {error && (
                        <div className="flex items-start gap-2 text-rose-400 bg-rose-900/20 border border-rose-800/40 rounded-xl px-3 py-2 text-xs">
                            <AlertCircle size={13} className="mt-0.5 shrink-0" />{error}
                        </div>
                    )}

                    <button
                        type="submit"
                        disabled={!token.trim() || loading}
                        className="w-full py-3 rounded-xl bg-violet-600 hover:bg-violet-500 disabled:bg-gray-700 disabled:text-gray-500 text-white font-semibold text-sm transition-colors"
                    >
                        {loading ? "Connecting…" : "Sign in"}
                    </button>
                </form>

                <p className="text-xs text-gray-600 text-center mt-6">
                    Your token is stored in localStorage and sent as a Bearer header to the engine.
                </p>
            </div>
        </div>
    );
}
