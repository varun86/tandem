import React, { createContext, useContext, useEffect, useState } from "react";
import { api, PORTAL_AUTH_EXPIRED_EVENT } from "./api";

const TOKEN_KEY = "tandem_aq_token";

interface AuthState {
    token: string | null;
    isLoading: boolean;
    providerConfigured: boolean;
    providerLoading: boolean;
    login: (token: string) => void;
    logout: () => void;
}

const Ctx = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
    const [token, setToken] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [providerConfigured, setProviderConfigured] = useState(false);
    const [providerLoading, setProviderLoading] = useState(false);

    useEffect(() => {
        const stored = localStorage.getItem(TOKEN_KEY);
        if (stored) {
            api.setToken(stored);
            setToken(stored);
        }
        setIsLoading(false);
    }, []);

    useEffect(() => {
        if (!token) return;
        setProviderLoading(true);
        api
            .resolveModelSpec()
            .then((spec) => setProviderConfigured(!!spec))
            .catch(() => setProviderConfigured(false))
            .finally(() => setProviderLoading(false));
    }, [token]);

    useEffect(() => {
        const handler = () => {
            setToken(null);
            localStorage.removeItem(TOKEN_KEY);
            api.setToken("");
        };
        window.addEventListener(PORTAL_AUTH_EXPIRED_EVENT, handler);
        return () => window.removeEventListener(PORTAL_AUTH_EXPIRED_EVENT, handler);
    }, []);

    const login = (t: string) => {
        localStorage.setItem(TOKEN_KEY, t);
        api.setToken(t);
        setToken(t);
    };

    const logout = () => {
        localStorage.removeItem(TOKEN_KEY);
        api.setToken("");
        setToken(null);
    };

    return (
        <Ctx.Provider value={{ token, isLoading, providerConfigured, providerLoading, login, logout }}>
            {children}
        </Ctx.Provider>
    );
}

export function useAuth() {
    const ctx = useContext(Ctx);
    if (!ctx) throw new Error("useAuth must be used inside AuthProvider");
    return ctx;
}
