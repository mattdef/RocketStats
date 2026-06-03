import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";
import { OverlayState, AuthState, LocalPlayerSummary, MatchSession, authLabel } from "./state";
import { initTheme, applySkin, getCurrentSkinId, setCurrentSkinId, getSkin } from "./theme";
import { renderSkinSelector, attachSkinSelectorListeners } from "./skin-selector";

type ConnectedAuthState = Extract<AuthState, { Connected: unknown }>;

const app = document.querySelector<HTMLDivElement>("#app");
let currentState: OverlayState | null = null;
let authActionError: string | null = null;
let localAuthDiagnostic: string | null = null;
let overlayPollTimer: number | null = null;
let settingsOpen = false;

const OVERLAY_POLL_INTERVAL_MS = 1000;

const PLAYLIST_NAMES: Record<number, string> = {
  10: "1v1",
  11: "2v2",
  13: "3v3",
};

const MAP_NAMES: Record<string, string> = {
  FF_Dusk_P: "Dusk",
  FF_Haunted_P: "Haunted Fields",
  FF_Bezier_P: "Neo Tokyo",
  FF_Underwater_P: "AquaDome",
  DFH_P: "DFH Stadium",
  DFH_Day_P: "DFH Stadium (Day)",
  DFH_Night_P: "DFH Stadium (Night)",
  DFH_Circuit_P: "DFH Stadium (Circuit)",
  Urban_P: "Beckwith Park",
  Urban_Night_P: "Beckwith Park (Night)",
  Urban_Goal_P: "Beckwith Park (Midnight)",
  Park_P: "Mannfield",
  Park_Night_P: "Mannfield (Night)",
  Park_Rainy_P: "Mannfield (Stormy)",
  Park_Snowy_P: "Mannfield (Snowy)",
  Curve_P: "Champion Field",
  Curve_Night_P: "Champion Field (Night)",
  Labs_Circle_P: "Pillars",
  Labs_Gravity_P: "Starbase ARC",
  Labs_Octagon_P: "Octagon",
  Labs_Underpass_P: "Forbidden Temple",
  Labs_Zero_G_P: "Core 707",
  Farm_P: "Farmstead",
  Farm_Night_P: "Farmstead (Night)",
  Farm_Snowy_P: "Farmstead (Snowy)",
  Farm_Rainy_P: "Farmstead (Overcast)",
  Throttle_P: "Speed Tunnel",
  Wasteland_P: "Wasteland",
  Wasteland_Night_P: "Wasteland (Night)",
  Badlands_P: "Wasteland",
  Badlands_Night_P: "Wasteland (Night)",
  Ricochet_P: "Utopia Coliseum",
  RocketLabs_P: "Rocket Labs",
  Throwback_P: "Throwback Stadium",
  Canyon_P: "Starbase ARC",
  Starbase_P: "Starbase ARC",
  Tut_P: "Sovereign Heights",
  CS_P: "Salty Shores",
  CS_Night_P: "Salty Shores (Night)",
  HoopsStadium_P: "Dunk House",
  Pillars_P: "Pillars",
  NeonTowers_P: "Neon Fields",
};

const tierLabels = [
  "Unranked",
  "Bronze I",
  "Bronze II",
  "Bronze III",
  "Silver I",
  "Silver II",
  "Silver III",
  "Gold I",
  "Gold II",
  "Gold III",
  "Platinum I",
  "Platinum II",
  "Platinum III",
  "Diamond I",
  "Diamond II",
  "Diamond III",
  "Champion I",
  "Champion II",
  "Champion III",
  "Grand Champion I",
  "Grand Champion II",
  "Grand Champion III",
  "Supersonic Legend",
];

if (!app) {
  throw new Error("missing #app root");
}

// narrowed to HTMLDivElement after guard; assert for closures
const root: HTMLDivElement = app;

function describeInvokeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

function renderCurrent(): void {
  if (!currentState) return;
  render(currentState);
}

function isConnected(auth: AuthState): auth is ConnectedAuthState {
  return typeof auth === "object" && "Connected" in auth;
}

function connectedAccountName(auth: AuthState): string | null {
  if (!isConnected(auth)) return null;
  return auth.Connected.player_name ?? auth.Connected.account_id;
}

function isAuthInProgress(auth: AuthState): boolean {
  return auth === "StartingDeviceLogin" || (typeof auth === "object" && "WaitingForDeviceCode" in auth);
}

function playlistName(playlist: number | null): string {
  if (playlist === null) return "Competitive";
  return PLAYLIST_NAMES[playlist] ?? `Mode ${playlist}`;
}

function mapDisplayName(rawMap: string | null): string {
  if (rawMap === null) return "Unknown map";
  return MAP_NAMES[rawMap] ?? rawMap;
}

function formatDuration(seconds: number | null): string {
  if (seconds === null) return "—";
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function startOverlayPolling(): void {
  if (overlayPollTimer !== null) return;
  overlayPollTimer = window.setInterval(() => {
    void refreshOverlayState();
  }, OVERLAY_POLL_INTERVAL_MS);
}

function stopOverlayPolling(): void {
  if (overlayPollTimer === null) return;
  window.clearInterval(overlayPollTimer);
  overlayPollTimer = null;
}

function syncOverlayPolling(): void {
  const shouldPoll =
    !currentState ||
    authActionError !== null ||
    localAuthDiagnostic !== null ||
    isAuthInProgress(currentState.auth);

  if (shouldPoll && !(currentState && isConnected(currentState.auth))) {
    startOverlayPolling();
    return;
  }

  stopOverlayPolling();
}

async function refreshOverlayState(): Promise<void> {
  try {
    currentState = await invoke<OverlayState>("get_overlay_state");
    authActionError = null;
    localAuthDiagnostic = null;
    renderCurrent();
    syncOverlayPolling();
  } catch (error) {
    authActionError = describeInvokeError(error);
    console.error(error);
    renderCurrent();
  }
}

function authDiagnosticsSection(state: OverlayState): string {
  const diagnostics = [...state.auth_diagnostics];
  if (localAuthDiagnostic) diagnostics.unshift(localAuthDiagnostic);
  if (authActionError) diagnostics.unshift(`UI error: ${authActionError}`);

  if (!diagnostics.length) return "";
  if (typeof state.auth === "object" && "Connected" in state.auth) return "";

  const items = diagnostics
    .map((entry) => `<li>${entry}</li>`)
    .join("");

  return `
    <section class="auth-diagnostics">
      <p class="diagnostics-title">Auth diagnostics</p>
      <ul>${items}</ul>
    </section>
  `;
}

// ── Idle ──────────────────────────────────────────────────────────────

function idleProfile(state: OverlayState): LocalPlayerSummary | null {
  if (!isConnected(state.auth)) return null;
  if (state.match_session.phase !== "Idle") return null;
  if (state.players.length > 0) return null;

  const fallbackName = connectedAccountName(state.auth) ?? "Connected player";
  return (
    state.local_player ?? {
      display_name: fallbackName,
      ranked_1v1_mmr: null,
      ranked_1v1_tier: null,
      ranked_1v1_division: null,
      ranked_2v2_mmr: null,
      ranked_2v2_tier: null,
      ranked_2v2_division: null,
      ranked_3v3_mmr: null,
      ranked_3v3_tier: null,
      ranked_3v3_division: null,
    }
  );
}

function playlistMmrLabel(mmr: number | null): string {
  if (mmr === null) return "—";
  return Math.round(mmr).toString();
}

function playlistRankLabel(tier: number | null, division: number | null): string {
  if (tier === null) return "Rank unavailable";

  const baseLabel = tierLabels[tier] ?? `Tier ${tier}`;
  if (division === null) return baseLabel;
  return `${baseLabel} Div ${division}`;
}

function playlistRow(label: string, mmr: number | null, tier: number | null, division: number | null): string {
  return `
    <article class="idle-stat">
      <p class="idle-stat-label">${label}</p>
      <p class="idle-stat-value">${playlistMmrLabel(mmr)}</p>
      <p class="idle-stat-rank">${playlistRankLabel(tier, division)}</p>
    </article>
  `;
}

function idleProfileSection(summary: LocalPlayerSummary): string {
  return `
    <section class="idle-profile">
      <h1>${summary.display_name}</h1>
      <p class="muted idle-subtitle">Competitive profiles</p>
      <div class="idle-stats">
        ${playlistRow("SOLO", summary.ranked_1v1_mmr, summary.ranked_1v1_tier, summary.ranked_1v1_division)}
        ${playlistRow("DOUBLES", summary.ranked_2v2_mmr, summary.ranked_2v2_tier, summary.ranked_2v2_division)}
        ${playlistRow("STANDARD", summary.ranked_3v3_mmr, summary.ranked_3v3_tier, summary.ranked_3v3_division)}
      </div>
      <button class="auth-btn auth-btn-secondary" id="logout-btn">Disconnect</button>
    </section>
  `;
}

// ── Matchmaking ───────────────────────────────────────────────────────

function renderMatchmaking(session: MatchSession): string {
  const mode = playlistName(session.playlist);
  const regions = session.regions.length > 0 ? session.regions.join(" · ") : "";

  return `
    <section class="match-phase match-making">
      <div class="match-phase-icon">
        <span class="pulse-ring"></span>
        <span class="pulse-dot"></span>
      </div>
      <h1>Searching</h1>
      <p class="match-mode">${mode}</p>
      ${regions ? `<p class="match-regions muted">${regions}</p>` : ""}
    </section>
  `;
}

// ── Joining ───────────────────────────────────────────────────────────

function renderJoining(session: MatchSession): string {
  const map = mapDisplayName(session.map);
  const mode = playlistName(session.playlist);

  return `
    <section class="match-phase match-joining">
      <div class="match-phase-icon">
        <span class="spinner"></span>
      </div>
      <h1>Loading</h1>
      <p class="match-map">${map}</p>
      <p class="match-mode muted">${mode}</p>
    </section>
  `;
}

// ── InMatch ───────────────────────────────────────────────────────────

function renderInMatch(state: OverlayState): string {
  const session = state.match_session;
  const mode = playlistName(session.playlist);
  const map = mapDisplayName(session.map);
  const players = state.players;

  const playerCards = players
    .map((player) => {
      const tier = player.tier !== null ? (tierLabels[player.tier] ?? `Tier ${player.tier}`) : "";
      const division = player.division !== null ? ` Div ${player.division}` : "";
      const rank = tier ? `${tier}${division}` : "";
      const mmr = player.mmr !== null ? Math.round(player.mmr).toString() : "";

      return `
        <article class="player-card">
          <div class="player-card-header">
            <strong>${player.name ?? "Detected player"}</strong>
            ${mmr ? `<span class="player-mmr">${mmr}</span>` : ""}
          </div>
          ${rank ? `<p class="player-rank">${rank}</p>` : ""}
        </article>
      `;
    })
    .join("");

  const playerCount = players.length;
  const playerLabel = playerCount === 1 ? "1 player detected" : `${playerCount} players detected`;

  return `
    <section class="match-phase match-in-match">
      <div class="match-info-bar">
        <span class="match-info-mode">${mode}</span>
        <span class="match-info-sep">·</span>
        <span class="match-info-map">${map}</span>
      </div>
      <div class="match-players">
        ${playerCards || `<p class="muted">Waiting for players…</p>`}
      </div>
      <p class="match-player-count muted">${playerLabel}</p>
    </section>
  `;
}

// ── Match Ended ───────────────────────────────────────────────────────

function renderMatchEnded(state: OverlayState): string {
  const session = state.match_session;
  const mode = playlistName(session.playlist);
  const map = mapDisplayName(session.map);
  const score = session.local_score;
  const duration = formatDuration(session.duration_seconds);
  const xp = session.xp;

  return `
    <section class="match-phase match-ended">
      <h1>Match Over</h1>
      <div class="match-summary">
        <div class="match-summary-row">
          <span class="match-summary-label">Mode</span>
          <span class="match-summary-value">${mode}</span>
        </div>
        <div class="match-summary-row">
          <span class="match-summary-label">Map</span>
          <span class="match-summary-value">${map}</span>
        </div>
        <div class="match-summary-row">
          <span class="match-summary-label">Duration</span>
          <span class="match-summary-value">${duration}</span>
        </div>
        ${score !== null ? `
        <div class="match-summary-row">
          <span class="match-summary-label">Score</span>
          <span class="match-summary-value">${score}</span>
        </div>
        ` : ""}
        ${xp !== null ? `
        <div class="match-summary-row">
          <span class="match-summary-label">XP</span>
          <span class="match-summary-value">${xp}</span>
        </div>
        ` : ""}
      </div>
    </section>
  `;
}

// ── Settings / Auth ───────────────────────────────────────────────────

function settingsSection(): string {
  if (!settingsOpen) return "";
  const currentSkinId = getCurrentSkinId();
  return `
    <section class="settings-panel">
      ${renderSkinSelector(currentSkinId)}
    </section>
  `;
}

function authSection(auth: AuthState): string {
  if (auth === "Unauthenticated") {
    return `
      <button class="auth-btn" id="login-btn">Sign in with Epic</button>
    `;
  }

  if (auth === "Expired") {
    return `
      <p class="muted">Session expired</p>
      <button class="auth-btn" id="login-btn">Sign in again</button>
    `;
  }

  if (auth === "StartingDeviceLogin") {
    return `
      <p class="muted">Contacting Epic services...</p>
    `;
  }

  if (typeof auth === "object" && "WaitingForDeviceCode" in auth) {
    const { user_code, verification_uri } = auth.WaitingForDeviceCode;
    return `
      <div class="device-code">
        <p class="code-label">Enter this code:</p>
        <p class="code-value">${user_code}</p>
        <a class="auth-btn code-open-btn" href="${verification_uri}" target="_blank" rel="noreferrer">Open Epic login page</a>
        <a class="code-link" href="${verification_uri}" target="_blank" rel="noreferrer">${verification_uri}</a>
      </div>
    `;
  }

  if (typeof auth === "object" && "Connected" in auth) {
    const name = auth.Connected.player_name ?? auth.Connected.account_id;
    return `
      <p class="muted">Signed in as <strong>${name}</strong></p>
      <button class="auth-btn auth-btn-secondary" id="logout-btn">Disconnect</button>
    `;
  }

  if (typeof auth === "object" && "Error" in auth) {
    return `
      <p class="error">${auth.Error.message}</p>
      <button class="auth-btn" id="login-btn">Retry</button>
    `;
  }

  return "";
}

// ── Main Render ───────────────────────────────────────────────────────

function render(state: OverlayState): void {
  const idleLocalPlayer = idleProfile(state);
  const phase = state.match_session.phase;

  let phaseContent: string;

  if (idleLocalPlayer) {
    phaseContent = idleProfileSection(idleLocalPlayer);
  } else {
    switch (phase) {
      case "Matchmaking":
        phaseContent = renderMatchmaking(state.match_session);
        break;
      case "Joining":
        phaseContent = renderJoining(state.match_session);
        break;
      case "InMatch":
        phaseContent = renderInMatch(state);
        break;
      case "Ended":
        phaseContent = renderMatchEnded(state);
        break;
      default:
        phaseContent = `
          <h1>${state.status_message}</h1>
          <p class="muted">${authLabel(state.auth)}</p>
          <div class="auth-section">${authSection(state.auth)}</div>
        `;
    }
  }

  const phaseClass = idleLocalPlayer ? "phase-idle" : `phase-${phase.toLowerCase()}`;

  root.innerHTML = `
    <main class="overlay-shell">
      <section class="panel ${phaseClass}">
        <div class="panel-header">
          <p class="eyebrow">RocketStats</p>
          <button class="settings-toggle" id="settings-toggle">⚙</button>
        </div>
        ${phaseContent}
        ${authDiagnosticsSection(state)}
        ${settingsSection()}
      </section>
    </main>
  `;

  // Attach event listeners after render
  const loginBtn = document.getElementById("login-btn");
  const logoutBtn = document.getElementById("logout-btn");
  const settingsToggle = document.getElementById("settings-toggle");

  if (loginBtn) {
    loginBtn.addEventListener("click", () => {
      console.info("[auth] sign-in clicked");
      authActionError = null;
      localAuthDiagnostic = "Sign-in clicked";
      startOverlayPolling();
      renderCurrent();
      invoke("start_login")
        .then(() => refreshOverlayState())
        .catch((error) => {
          authActionError = describeInvokeError(error);
          localAuthDiagnostic = null;
          console.error(error);
          renderCurrent();
        });
    });
  }

  if (logoutBtn) {
    logoutBtn.addEventListener("click", () => {
      console.info("[auth] logout clicked");
      authActionError = null;
      localAuthDiagnostic = "Disconnect clicked";
      startOverlayPolling();
      renderCurrent();
      invoke("logout")
        .then(() => refreshOverlayState())
        .catch((error) => {
          authActionError = describeInvokeError(error);
          localAuthDiagnostic = null;
          console.error(error);
          renderCurrent();
        });
    });
  }

  if (settingsToggle) {
    settingsToggle.addEventListener("click", () => {
      settingsOpen = !settingsOpen;
      renderCurrent();
    });
  }

  // Skin selector listeners
  if (settingsOpen) {
    attachSkinSelectorListeners((skinId: string) => {
      console.info("[theme] skin selected:", skinId);
      setCurrentSkinId(skinId);
      applySkin(getSkin(skinId));
      renderCurrent();
    });
  }
}

async function boot(): Promise<void> {
  // Apply stored theme
  initTheme();

  void listen<OverlayState>("overlay-state", (event) => {
    console.info("[auth] overlay-state update", event.payload.auth);
    currentState = event.payload;
    authActionError = null;
    localAuthDiagnostic = null;
    renderCurrent();
    syncOverlayPolling();
  }).catch((error) => {
    authActionError = describeInvokeError(error);
    console.error(error);
    renderCurrent();
  });

  currentState = await invoke<OverlayState>("get_overlay_state");
  authActionError = null;
  renderCurrent();
  syncOverlayPolling();
}

void listen<boolean>("click-through-toggled", (event) => {
  console.info("Overlay click-through toggled:", event.payload);
});

void boot();
