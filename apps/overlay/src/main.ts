import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";
import { OverlayState, AuthState, authLabel } from "./state";

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("missing #app root");
}

// narrowed to HTMLDivElement after guard; assert for closures
const root: HTMLDivElement = app;

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

  if (typeof auth === "object" && "WaitingForDeviceCode" in auth) {
    const { user_code, verification_uri } = auth.WaitingForDeviceCode;
    return `
      <div class="device-code">
        <p class="code-label">Enter this code:</p>
        <p class="code-value">${user_code}</p>
        <a class="code-link" href="${verification_uri}" target="_blank">${verification_uri}</a>
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
      invoke("start_login").catch(console.error);
    });
  }

  if (logoutBtn) {
    logoutBtn.addEventListener("click", () => {
      invoke("logout").catch(console.error);
    });
  }
}

async function boot(): Promise<void> {
  render(await invoke<OverlayState>("get_overlay_state"));
  await listen<OverlayState>("overlay-state", (event) => render(event.payload));
}

void boot();
