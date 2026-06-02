export interface SkinColors {
  bg_panel: string;
  bg_card: string;
  text_primary: string;
  text_muted: string;
  accent: string;
  accent_hover: string;
  border: string;
  error: string;
  warning: string;
}

export interface SkinLayout {
  panel_position: "top-right" | "top-left" | "bottom-right" | "bottom-left";
  panel_width: number;
  card_radius: number;
  compact_mode: boolean;
}

export interface SkinTypography {
  font_family: string;
  font_size_base: number;
  font_size_heading: number;
}

export interface SkinAnimations {
  transition_duration: string;
  hover_scale: number;
}

export interface Skin {
  id: string;
  name: string;
  colors: SkinColors;
  layout: SkinLayout;
  typography: SkinTypography;
  animations: SkinAnimations;
}

const rocketStatsDark: Skin = {
  id: "rocket-stats-dark",
  name: "Rocket Stats Dark",
  colors: {
    bg_panel:
      "linear-gradient(135deg, rgba(18, 22, 28, 0.88), rgba(35, 43, 48, 0.68)), radial-gradient(circle at 20% 10%, rgba(255, 180, 80, 0.22), transparent 36%)",
    bg_card: "rgba(255, 255, 255, 0.08)",
    text_primary: "#f5f1e8",
    text_muted: "rgba(245, 241, 232, 0.74)",
    accent: "#ffcf75",
    accent_hover: "#ff9f43",
    border: "rgba(255, 255, 255, 0.18)",
    error: "#ff6b6b",
    warning: "#ffcf75",
  },
  layout: {
    panel_position: "top-right",
    panel_width: 360,
    card_radius: 16,
    compact_mode: false,
  },
  typography: {
    font_family: '"Space Grotesk", "Segoe UI", sans-serif',
    font_size_base: 14,
    font_size_heading: 28,
  },
  animations: {
    transition_duration: "0.2s",
    hover_scale: 1,
  },
};

const minimalLight: Skin = {
  id: "minimal-light",
  name: "Minimal Light",
  colors: {
    bg_panel:
      "linear-gradient(135deg, rgba(255, 253, 248, 0.96), rgba(244, 237, 226, 0.92)), radial-gradient(circle at 20% 10%, rgba(79, 140, 255, 0.16), transparent 36%)",
    bg_card: "rgba(255, 255, 255, 0.82)",
    text_primary: "#1f2933",
    text_muted: "rgba(31, 41, 51, 0.72)",
    accent: "#4f8cff",
    accent_hover: "#3b6fe0",
    border: "rgba(31, 41, 51, 0.12)",
    error: "#d64545",
    warning: "#c98000",
  },
  layout: {
    panel_position: "top-right",
    panel_width: 360,
    card_radius: 16,
    compact_mode: false,
  },
  typography: {
    font_family: '"Space Grotesk", "Segoe UI", sans-serif',
    font_size_base: 14,
    font_size_heading: 28,
  },
  animations: {
    transition_duration: "0.2s",
    hover_scale: 1,
  },
};

const rlBoost: Skin = {
  id: "rl-boost",
  name: "RL Boost",
  colors: {
    bg_panel:
      "linear-gradient(135deg, rgba(8, 12, 20, 0.94), rgba(16, 27, 44, 0.86)), radial-gradient(circle at 18% 12%, rgba(255, 127, 58, 0.24), transparent 34%)",
    bg_card: "rgba(255, 255, 255, 0.1)",
    text_primary: "#f4f7fb",
    text_muted: "rgba(244, 247, 251, 0.78)",
    accent: "linear-gradient(135deg, #ff8c3a, #3fa7ff)",
    accent_hover: "linear-gradient(135deg, #ff9f43, #67b7ff)",
    border: "rgba(99, 159, 255, 0.26)",
    error: "#ff6b6b",
    warning: "#ffb347",
  },
  layout: {
    panel_position: "top-right",
    panel_width: 400,
    card_radius: 20,
    compact_mode: false,
  },
  typography: {
    font_family: '"Space Grotesk", "Segoe UI", sans-serif',
    font_size_base: 15,
    font_size_heading: 30,
  },
  animations: {
    transition_duration: "0.2s",
    hover_scale: 1.02,
  },
};

export const SKINS: Skin[] = [rocketStatsDark, minimalLight, rlBoost];

export const DEFAULT_SKIN_ID = "rocket-stats-dark";

export function getSkin(id: string): Skin {
  return SKINS.find((skin) => skin.id === id) ?? rocketStatsDark;
}
