export const DEFAULT_THEME_ID = "web_control";
const STORAGE_KEY = "tandem.themeId";

export const THEMES = [
  {
    id: "web_control",
    name: "Web Control",
    cssVars: {
      "--color-background": "#121212",
      "--color-surface": "#141414",
      "--color-surface-elevated": "#1a1a1a",
      "--color-border": "rgba(245, 245, 245, 0.10)",
      "--color-border-subtle": "rgba(245, 245, 245, 0.06)",
      "--color-primary": "#F59E0B",
      "--color-primary-hover": "#D97706",
      "--color-primary-muted": "#B45309",
      "--color-secondary": "#EF4444",
      "--color-secondary-hover": "#DC2626",
      "--color-success": "#10B981",
      "--color-warning": "#F59E0B",
      "--color-error": "#EF4444",
      "--color-text": "#F5F5F5",
      "--color-text-muted": "rgba(245, 245, 245, 0.70)",
      "--color-text-subtle": "rgba(245, 245, 245, 0.50)",
      "--color-glass": "rgba(255, 255, 255, 0.03)",
      "--color-glass-border": "rgba(255, 255, 255, 0.08)",
      "--font-sans": '"Geist Sans", "Inter", system-ui, -apple-system, sans-serif',
      "--font-mono": '"Geist Mono", "JetBrains Mono", "Fira Code", ui-monospace, SFMono-Regular, Menlo, monospace',
      "--tcp-glow-a": "rgba(245, 158, 11, 0.16)",
      "--tcp-glow-b": "rgba(239, 68, 68, 0.12)"
    }
  }
];

export function getThemeById(themeId) {
  const id = String(themeId || "").trim() || DEFAULT_THEME_ID;
  return THEMES.find((theme) => theme.id === id) || THEMES[0];
}

export function getActiveThemeId() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return getThemeById(saved).id;
  } catch {
    return DEFAULT_THEME_ID;
  }
}

export function applyTheme(themeId) {
  const theme = getThemeById(themeId);
  for (const [name, value] of Object.entries(theme.cssVars)) {
    document.documentElement.style.setProperty(name, String(value));
  }
  document.documentElement.dataset.theme = theme.id;
  document.documentElement.style.colorScheme = "dark";
  return theme;
}

export function setControlPanelTheme(themeId) {
  const theme = applyTheme(themeId);
  try {
    localStorage.setItem(STORAGE_KEY, theme.id);
  } catch {
    // ignore storage failures
  }
  return theme;
}
