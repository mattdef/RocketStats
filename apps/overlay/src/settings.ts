import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";

interface Settings {
  log_path: string;
  app_log_dir: string;
  opacity: number;
  always_on_top: boolean;
  click_through: boolean;
}

type StatusState = "idle" | "success" | "error";

interface SettingsForm {
  form: HTMLFormElement;
  logPath: HTMLInputElement;
  appLogDir: HTMLInputElement;
  opacity: HTMLInputElement;
  opacityValue: HTMLOutputElement;
  alwaysOnTop: HTMLInputElement;
  clickThrough: HTMLInputElement;
  saveButton: HTMLButtonElement;
  resetButton: HTMLButtonElement;
  status: HTMLParagraphElement;
}

const DEFAULT_OPACITY = 0.9;

const SETTINGS_STYLES = `
  html,
  body,
  #settings-app {
    min-height: 100%;
    margin: 0;
    background:
      radial-gradient(circle at top, rgba(255, 207, 117, 0.14), transparent 32%),
      linear-gradient(180deg, #0d1217 0%, #131a22 100%);
  }

  body {
    overflow: auto;
  }

  #settings-app {
    color: #f5f1e8;
  }

  .settings-shell {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    box-sizing: border-box;
  }

  .settings-panel {
    width: min(100%, 440px);
    background:
      linear-gradient(135deg, rgba(17, 21, 27, 0.96), rgba(27, 34, 41, 0.92)),
      radial-gradient(circle at 20% 10%, rgba(255, 180, 80, 0.18), transparent 36%);
  }

  .settings-form {
    display: grid;
    gap: 16px;
    margin-top: 18px;
  }

  .field-group {
    display: grid;
    gap: 8px;
  }

  .field-label,
  .toggle-label {
    color: #f5f1e8;
    font-size: 14px;
    font-weight: 600;
  }

  .text-input,
  .range-input {
    width: 100%;
    box-sizing: border-box;
  }

  .text-input {
    padding: 12px 14px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 14px;
    background: rgba(7, 10, 14, 0.74);
    color: #f5f1e8;
    font: inherit;
  }

  .text-input::placeholder {
    color: rgba(245, 241, 232, 0.42);
  }

  .text-input:focus-visible,
  .range-input:focus-visible,
  .checkbox-input:focus-visible,
  .button:focus-visible {
    outline: 2px solid rgba(255, 207, 117, 0.85);
    outline-offset: 2px;
  }

  .range-row {
    display: grid;
    gap: 10px;
  }

  .range-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .range-value {
    color: #ffcf75;
    font-size: 14px;
    font-weight: 600;
  }

  .range-input {
    height: 6px;
    margin: 0;
    appearance: none;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
  }

  .range-input::-webkit-slider-thumb {
    width: 18px;
    height: 18px;
    appearance: none;
    border: 0;
    border-radius: 50%;
    background: #ffcf75;
    box-shadow: 0 0 0 4px rgba(255, 207, 117, 0.18);
    cursor: pointer;
  }

  .range-input::-moz-range-track {
    height: 6px;
    border: 0;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
  }

  .range-input::-moz-range-thumb {
    width: 18px;
    height: 18px;
    border: 0;
    border-radius: 50%;
    background: #ffcf75;
    box-shadow: 0 0 0 4px rgba(255, 207, 117, 0.18);
    cursor: pointer;
  }

  .checkbox-group {
    display: grid;
    gap: 12px;
  }

  .checkbox-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 14px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.06);
  }

  .checkbox-copy {
    display: grid;
    gap: 4px;
  }

  .checkbox-copy .muted {
    font-size: 12px;
  }

  .checkbox-input {
    width: 18px;
    height: 18px;
    margin: 0;
    accent-color: #ffcf75;
  }

  .actions {
    display: flex;
    gap: 12px;
  }

  .button {
    flex: 1;
    padding: 12px 14px;
    border: 1px solid transparent;
    border-radius: 14px;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    transition: transform 120ms ease, box-shadow 120ms ease, opacity 120ms ease;
  }

  .button:hover:not(:disabled) {
    transform: translateY(-1px);
  }

  .button:disabled {
    opacity: 0.68;
    cursor: wait;
  }

  .button-primary {
    border-color: rgba(255, 207, 117, 0.7);
    background: #ffcf75;
    color: #171b21;
    box-shadow: 0 12px 28px rgba(255, 207, 117, 0.18);
  }

  .button-secondary {
    border-color: rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.08);
    color: #f5f1e8;
  }

  .settings-status {
    min-height: 20px;
    margin: 0;
    color: rgba(245, 241, 232, 0.74);
    font-size: 13px;
  }

  .settings-status[data-state="success"] {
    color: #ffcf75;
  }

  .settings-status[data-state="error"] {
    color: #ff9a9a;
  }
`;

const app = document.querySelector<HTMLDivElement>("#settings-app");

if (!app) {
  throw new Error("missing #settings-app root");
}

const root: HTMLDivElement = app;

installStyles();
const form = render();

function installStyles(): void {
  if (document.getElementById("settings-page-styles")) {
    return;
  }

  const style = document.createElement("style");
  style.id = "settings-page-styles";
  style.textContent = SETTINGS_STYLES;
  document.head.append(style);
}

function render(): SettingsForm {
  root.innerHTML = `
    <main class="settings-shell">
      <section class="panel settings-panel">
        <p class="eyebrow">RocketStats</p>
        <h1>Overlay settings</h1>
        <p class="muted">Update the overlay path, app log directory, opacity, and window behavior.</p>
        <form id="settings-form" class="settings-form">
          <div class="field-group">
            <label class="field-label" for="log-path">Log path</label>
            <input
              id="log-path"
              class="text-input"
              type="text"
              autocomplete="off"
              spellcheck="false"
            />
          </div>

          <div class="field-group">
            <label class="field-label" for="app-log-dir">Application log directory</label>
            <input
              id="app-log-dir"
              class="text-input"
              type="text"
              autocomplete="off"
              spellcheck="false"
            />
          </div>

          <div class="field-group range-row">
            <div class="range-header">
              <label class="field-label" for="opacity">Opacity</label>
              <output id="opacity-value" class="range-value">90%</output>
            </div>
            <input
              id="opacity"
              class="range-input"
              type="range"
              min="0.1"
              max="1.0"
              step="0.05"
            />
          </div>

          <div class="checkbox-group">
            <label class="checkbox-row" for="always-on-top">
              <span class="checkbox-copy">
                <span class="toggle-label">Always on top</span>
                <span class="muted">Keep the overlay window above the game.</span>
              </span>
              <input id="always-on-top" class="checkbox-input" type="checkbox" />
            </label>

            <label class="checkbox-row" for="click-through">
              <span class="checkbox-copy">
                <span class="toggle-label">Click-through</span>
                <span class="muted">Ignore mouse input while the overlay is visible.</span>
              </span>
              <input id="click-through" class="checkbox-input" type="checkbox" />
            </label>
          </div>

          <div class="actions">
            <button id="save-settings" class="button button-primary" type="submit">Save</button>
            <button id="reset-settings" class="button button-secondary" type="button">Reset</button>
          </div>

          <p id="settings-status" class="settings-status" aria-live="polite"></p>
        </form>
      </section>
    </main>
  `;

  return {
    form: requireElement<HTMLFormElement>("#settings-form"),
    logPath: requireElement<HTMLInputElement>("#log-path"),
    appLogDir: requireElement<HTMLInputElement>("#app-log-dir"),
    opacity: requireElement<HTMLInputElement>("#opacity"),
    opacityValue: requireElement<HTMLOutputElement>("#opacity-value"),
    alwaysOnTop: requireElement<HTMLInputElement>("#always-on-top"),
    clickThrough: requireElement<HTMLInputElement>("#click-through"),
    saveButton: requireElement<HTMLButtonElement>("#save-settings"),
    resetButton: requireElement<HTMLButtonElement>("#reset-settings"),
    status: requireElement<HTMLParagraphElement>("#settings-status"),
  };
}

function requireElement<T extends Element>(selector: string): T {
  const element = root.querySelector<T>(selector);

  if (!element) {
    throw new Error(`missing ${selector}`);
  }

  return element;
}

function formatOpacity(opacity: number): string {
  return `${Math.round(opacity * 100)}%`;
}

function applySettings(settings: Settings): void {
  form.logPath.value = settings.log_path;
  form.appLogDir.value = settings.app_log_dir;
  form.opacity.value = settings.opacity.toFixed(2);
  form.opacityValue.textContent = formatOpacity(settings.opacity);
  form.alwaysOnTop.checked = settings.always_on_top;
  form.clickThrough.checked = settings.click_through;
}

function readSettingsFromForm(): Settings {
  const opacity = Number.parseFloat(form.opacity.value);

  return {
    log_path: form.logPath.value,
    app_log_dir: form.appLogDir.value,
    opacity: Number.isFinite(opacity) ? opacity : DEFAULT_OPACITY,
    always_on_top: form.alwaysOnTop.checked,
    click_through: form.clickThrough.checked,
  };
}

function setBusy(isBusy: boolean): void {
  form.logPath.disabled = isBusy;
  form.appLogDir.disabled = isBusy;
  form.opacity.disabled = isBusy;
  form.alwaysOnTop.disabled = isBusy;
  form.clickThrough.disabled = isBusy;
  form.saveButton.disabled = isBusy;
  form.resetButton.disabled = isBusy;
}

function setStatus(message: string, state: StatusState = "idle"): void {
  form.status.textContent = message;
  form.status.dataset.state = state === "idle" ? "" : state;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function refreshSettings(statusMessage = ""): Promise<void> {
  setBusy(true);

  try {
    applySettings(await invoke<Settings>("get_settings"));
    setStatus(statusMessage);
  } catch (error) {
    setStatus(`Unable to load settings: ${errorMessage(error)}`, "error");
  } finally {
    setBusy(false);
  }
}

async function saveSettings(): Promise<void> {
  setBusy(true);
  setStatus("Saving settings…");

  try {
    const settings = readSettingsFromForm();
    const saved = await invoke<Settings>("save_settings", { settings });
    applySettings(saved);
    setStatus("Settings saved.", "success");
  } catch (error) {
    setStatus(`Unable to save settings: ${errorMessage(error)}`, "error");
  } finally {
    setBusy(false);
  }
}

form.form.addEventListener("submit", (event) => {
  event.preventDefault();
  void saveSettings();
});

form.resetButton.addEventListener("click", () => {
  void refreshSettings("Settings reloaded.");
});

form.opacity.addEventListener("input", () => {
  const opacity = Number.parseFloat(form.opacity.value);
  form.opacityValue.textContent = formatOpacity(
    Number.isFinite(opacity) ? opacity : DEFAULT_OPACITY,
  );
});

async function boot(): Promise<void> {
  await refreshSettings();

  try {
    const unlisten = await listen("settings-updated", () => {
      void refreshSettings("Settings updated.");
    });

    window.addEventListener(
      "beforeunload",
      () => {
        void unlisten();
      },
      { once: true },
    );
  } catch (error) {
    setStatus(`Unable to subscribe to updates: ${errorMessage(error)}`, "error");
  }
}

void boot();
