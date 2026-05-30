import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";
import { OverlayState, authLabel } from "./state";

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
        <p class="warning">${state.partial_roster ? "Detected players only. Full lobby is not guaranteed." : ""}</p>
        <div class="players">${players}</div>
      </section>
    </main>
  `;
}

async function boot(): Promise<void> {
  render(await invoke<OverlayState>("get_overlay_state"));
  await listen<OverlayState>("overlay-state", (event) => render(event.payload));
}

void boot();
