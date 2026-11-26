export type ThemePreference = "system" | "light" | "dark";
export type DensityPreference = "comfortable" | "compact";
export type TextScalePreference = "100" | "110" | "125";

export type LocalPreferences = {
  theme: ThemePreference;
  density: DensityPreference;
  textScale: TextScalePreference;
  reducedMotion: boolean;
  /** Local display-only choice; it never changes the semantic answer or provider request. */
  richPresentationEnabled: boolean;
};

export const LOCAL_PREFERENCES_STORAGE_KEY = "litecowork.local-preferences.v1";

export const DEFAULT_LOCAL_PREFERENCES: LocalPreferences = {
  theme: "light",
  density: "comfortable",
  textScale: "100",
  reducedMotion: false,
  richPresentationEnabled: true,
};

export type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

const THEMES: readonly ThemePreference[] = ["system", "light", "dark"];
const DENSITIES: readonly DensityPreference[] = ["comfortable", "compact"];
const TEXT_SCALES: readonly TextScalePreference[] = ["100", "110", "125"];

/** Field-wise validation: an unknown or corrupt field falls back to its default; nothing throws. */
export function parseLocalPreferences(raw: string | null | undefined): LocalPreferences {
  if (!raw) return { ...DEFAULT_LOCAL_PREFERENCES };
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return { ...DEFAULT_LOCAL_PREFERENCES };
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return { ...DEFAULT_LOCAL_PREFERENCES };
  }
  const record = value as Record<string, unknown>;
  return {
    theme: THEMES.find(item => item === record.theme) ?? DEFAULT_LOCAL_PREFERENCES.theme,
    density: DENSITIES.find(item => item === record.density) ?? DEFAULT_LOCAL_PREFERENCES.density,
    textScale: TEXT_SCALES.find(item => item === record.textScale) ?? DEFAULT_LOCAL_PREFERENCES.textScale,
    reducedMotion: typeof record.reducedMotion === "boolean" ? record.reducedMotion : DEFAULT_LOCAL_PREFERENCES.reducedMotion,
    // Older local preference snapshots predate this field and keep the default-on behavior.
    richPresentationEnabled: typeof record.richPresentationEnabled === "boolean"
      ? record.richPresentationEnabled
      : DEFAULT_LOCAL_PREFERENCES.richPresentationEnabled,
  };
}

export function loadLocalPreferences(storage: PreferenceStorage | null): LocalPreferences {
  if (!storage) return { ...DEFAULT_LOCAL_PREFERENCES };
  try {
    return parseLocalPreferences(storage.getItem(LOCAL_PREFERENCES_STORAGE_KEY));
  } catch {
    return { ...DEFAULT_LOCAL_PREFERENCES };
  }
}

/** Returns false when persistence failed; the in-memory value still applies for this session. */
export function saveLocalPreferences(storage: PreferenceStorage | null, preferences: LocalPreferences): boolean {
  if (!storage) return false;
  try {
    storage.setItem(LOCAL_PREFERENCES_STORAGE_KEY, JSON.stringify(preferences));
    return true;
  } catch {
    return false;
  }
}

export type PreferenceRoot = {
  setAttribute(name: string, value: string): void;
  style: { setProperty(name: string, value: string): void };
};

/** Applies preferences as root attributes and CSS variables only. Pure DOM and storage work only. */
export function applyLocalPreferences(root: PreferenceRoot, preferences: LocalPreferences): void {
  root.setAttribute("data-theme", preferences.theme);
  root.setAttribute("data-density", preferences.density);
  root.setAttribute("data-reduced-motion", preferences.reducedMotion ? "true" : "false");
  root.style.setProperty("--lc-text-scale", String(Number(preferences.textScale) / 100));
}

export function browserPreferenceStorage(): PreferenceStorage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}
