import { Skin, getSkin, DEFAULT_SKIN_ID, SKINS } from "./skins";

const STORAGE_KEY = "rocketstats-skin-id";

function derivedColor(base: string, opacity: number): string {
  // For simple hex/rgb colors, produce an rgba variant
  if (base.startsWith("#")) {
    const r = parseInt(base.slice(1, 3), 16);
    const g = parseInt(base.slice(3, 5), 16);
    const b = parseInt(base.slice(5, 7), 16);
    return `rgba(${r}, ${g}, ${b}, ${opacity})`;
  }
  // For already-rgba or complex values, return as-is with a note
  return base;
}

export function applySkin(skin: Skin): void {
  const root = document.documentElement;
  const { colors, layout, typography, animations } = skin;

  // Colors
  root.style.setProperty("--bg-panel", colors.bg_panel);
  root.style.setProperty("--bg-card", colors.bg_card);
  root.style.setProperty("--bg-card-hover", derivedColor(colors.accent, 0.12));
  root.style.setProperty("--bg-card-muted", derivedColor(colors.bg_card, 0.75));
  root.style.setProperty("--bg-card-subtle", derivedColor(colors.bg_card, 0.62));
  root.style.setProperty("--bg-card-strong", derivedColor(colors.bg_card, 1.5));

  root.style.setProperty("--text-primary", colors.text_primary);
  root.style.setProperty("--text-muted", colors.text_muted);
  root.style.setProperty(
    "--text-muted-strong",
    derivedColor(colors.text_primary, 0.84),
  );
  root.style.setProperty(
    "--text-muted-medium",
    derivedColor(colors.text_primary, 0.82),
  );
  root.style.setProperty(
    "--text-muted-soft",
    derivedColor(colors.text_primary, 0.6),
  );
  root.style.setProperty(
    "--text-muted-faint",
    derivedColor(colors.text_primary, 0.58),
  );
  root.style.setProperty(
    "--text-muted-link",
    derivedColor(colors.text_primary, 0.5),
  );
  root.style.setProperty("--text-accent", "#1a1a2e");

  root.style.setProperty("--color-accent", colors.accent);
  root.style.setProperty("--color-accent-hover", colors.accent_hover);
  root.style.setProperty("--color-error", colors.error);
  root.style.setProperty("--color-warning", colors.warning);
  root.style.setProperty("--color-border", colors.border);
  root.style.setProperty(
    "--color-border-light",
    derivedColor(colors.text_primary, 0.1),
  );
  root.style.setProperty(
    "--color-border-muted",
    derivedColor(colors.text_primary, 0.08),
  );
  root.style.setProperty(
    "--color-border-strong",
    derivedColor(colors.text_primary, 0.12),
  );

  // Layout
  root.style.setProperty("--panel-width", `${layout.panel_width}px`);
  root.style.setProperty("--radius-card", `${layout.card_radius}px`);

  // Typography
  root.style.setProperty("--font-family", typography.font_family);
  root.style.setProperty("--font-size-base", `${typography.font_size_base}px`);
  root.style.setProperty(
    "--font-size-heading",
    `${typography.font_size_heading}px`,
  );

  // Animations
  root.style.setProperty("--transition-speed", animations.transition_duration);
  root.style.setProperty("--hover-scale", String(animations.hover_scale));
}

export function getCurrentSkinId(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? DEFAULT_SKIN_ID;
  } catch {
    return DEFAULT_SKIN_ID;
  }
}

export function setCurrentSkinId(id: string): void {
  try {
    localStorage.setItem(STORAGE_KEY, id);
  } catch {
    // localStorage unavailable — silently ignore
  }
}

export function initTheme(): Skin {
  const id = getCurrentSkinId();
  const skin = getSkin(id);
  applySkin(skin);
  return skin;
}

export { SKINS, getSkin, DEFAULT_SKIN_ID };
export type { Skin };
