import {
  argbFromHex,
  hexFromArgb,
  themeFromSourceColor,
  type Scheme,
} from "@material/material-color-utilities";

export type ThemeMode = "system" | "light" | "dark";

const TOKEN_OVERRIDES: Record<string, string> = {
  background: "background",
  surface: "surface",
  surfaceVariant: "surface-variant",
  surfaceDim: "surface-dim",
  surfaceBright: "surface-bright",
  surfaceContainerLowest: "surface-container-lowest",
  surfaceContainerLow: "surface-container-low",
  surfaceContainer: "surface-container",
  surfaceContainerHigh: "surface-container-high",
  surfaceContainerHighest: "surface-container-highest",
  onSurface: "on-surface",
  onSurfaceVariant: "on-surface-variant",
  inverseSurface: "inverse-surface",
  inverseOnSurface: "inverse-on-surface",
  outline: "outline",
  outlineVariant: "outline-variant",
  primary: "primary",
  onPrimary: "on-primary",
  primaryContainer: "primary-container",
  onPrimaryContainer: "on-primary-container",
  secondary: "secondary",
  onSecondary: "on-secondary",
  secondaryContainer: "secondary-container",
  onSecondaryContainer: "on-secondary-container",
  tertiary: "tertiary",
  onTertiary: "on-tertiary",
  tertiaryContainer: "tertiary-container",
  onTertiaryContainer: "on-tertiary-container",
  error: "error",
  onError: "on-error",
  errorContainer: "error-container",
  onErrorContainer: "on-error-container",
  shadow: "shadow",
  scrim: "scrim",
};

function kebab(value: string): string {
  return value.replace(/[A-Z]/g, (char) => `-${char.toLowerCase()}`);
}

export function isDarkMode(mode: ThemeMode): boolean {
  if (mode === "dark") return true;
  if (mode === "light") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function applyTheme(seed: string, mode: ThemeMode): void {
  const theme = themeFromSourceColor(argbFromHex(seed));
  const dark = isDarkMode(mode);
  const scheme: Scheme = dark ? theme.schemes.dark : theme.schemes.light;
  const json = scheme.toJSON() as Record<string, number>;
  const root = document.documentElement;

  for (const [key, argb] of Object.entries(json)) {
    const token = TOKEN_OVERRIDES[key] ?? kebab(key);
    root.style.setProperty(`--md-sys-color-${token}`, hexFromArgb(argb));
  }

  // The skill's "shield dark + connected green" palette. Material 3 keeps the
  // component semantics; these surfaces give the app its own night-time identity.
  // Remove them when switching back to light, otherwise the inline dark values
  // would keep overriding the light scheme.
  const darkSurfaces: Record<string, string> = {
    "surface": "#0F172A",
    "background": "#0F172A",
    "surface-dim": "#0B1222",
    "surface-bright": "#1E2942",
    "surface-container-lowest": "#0B1222",
    "surface-container-low": "#10192E",
    "surface-container": "#192134",
    "surface-container-high": "#1E2942",
    "surface-container-highest": "#243250",
    "on-surface": "#F8FAFC",
    "on-surface-variant": "#94A3B8",
    "outline": "#334155",
    "outline-variant": "rgba(255, 255, 255, 0.08)",
    "inverse-surface": "#E2E8F0",
    "inverse-on-surface": "#0F172A",
  };
  for (const token of Object.keys(darkSurfaces)) {
    root.style.removeProperty(`--md-sys-color-${token}`);
  }
  if (dark) {
    for (const [token, value] of Object.entries(darkSurfaces)) {
      root.style.setProperty(`--md-sys-color-${token}`, value);
    }
  }

  root.style.setProperty("--m3-accent", seed);
  root.style.setProperty("color-scheme", dark ? "dark" : "light");
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.seed = seed;
}

export function watchSystemTheme(callback: () => void): () => void {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  media.addEventListener("change", callback);
  return () => media.removeEventListener("change", callback);
}
