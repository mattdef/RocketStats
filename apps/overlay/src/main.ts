import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";
import { OverlayState, AuthState, authLabel } from "./state";

const app = document.querySelector<HTMLDivElement>("#app");
let currentState: OverlayState | null = null;
let authActionError: string | null = null;
let localAuthDiagnostic: string | null = null;
let overlayPollTimer: number | null = null;

const OVERLAY_POLL_INTERVAL_MS = 1000;

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

function isConnected(auth: AuthState): boolean {
  return typeof auth === "object" && "Connected" in auth;
}

function isAuthInProgress(auth: AuthState): boolean {
  return auth === "StartingDeviceLogin" || (typeof auth === "object" && "WaitingForDeviceCode" in auth);
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

function rankLine(player: OverlayState["players"][number]): string {
  if (player.mmr === null) return "Rank unavailable";
  const tier = player.tier === null ? "?" : String(player.tier);
  const division = player.division === null ? "?" : String(player.division);
  return `MMR ${player.mmr.toFixed(1)} | Tier ${tier} Div ${division}`;
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

function render(state: OverlayState): void {
  const players = state.players
    .map(
      (player) => `
        <article class="player-card">
          <div>
            <strong>${player.name ?? "Detected player"}</strong>
            <span>${player.player_id}</span>
          </div>
          <p>${rankLine(player)}</p>
        </article>
      `,
    )
    .join("");

  root.innerHTML = `
    <main class="overlay-shell">
      <section class="panel">
        <p class="eyebrow">RocketStats</p>
        <h1>${state.status_message}</h1>
        <p class="muted">${authLabel(state.auth)}</p>
        <div class="auth-section">${authSection(state.auth)}</div>
        ${authDiagnosticsSection(state)}
        <p class="warning">${state.partial_roster ? "Detected players only. Full lobby is not guaranteed." : ""}</p>
        <div class="players">${players}</div>
      </section>
    </main>
  `;

  // Attach event listeners after render
  const loginBtn = document.getElementById("login-btn");
  const logoutBtn = document.getElementById("logout-btn");

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
}

async function boot(): Promise<void> {
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

void boot();
