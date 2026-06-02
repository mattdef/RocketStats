import { SKINS } from "./skins";

function colorSwatch(color: string): string {
  // For gradient backgrounds, extract the first solid color or use a fallback
  const solid = color.includes("gradient") ? "rgba(255,255,255,0.2)" : color;
  return `<span class="skin-swatch" style="background:${solid}"></span>`;
}

export function renderSkinSelector(currentSkinId: string): string {
  const options = SKINS.map((skin) => {
    const selected = skin.id === currentSkinId;
    return `
      <article class="skin-option${selected ? " skin-option--selected" : ""}" data-skin-id="${skin.id}">
        <div class="skin-option-swatches">
          ${colorSwatch(skin.colors.accent)}
          ${colorSwatch(skin.colors.bg_panel)}
          ${colorSwatch(skin.colors.text_primary)}
        </div>
        <div class="skin-option-info">
          <p class="skin-option-name">${skin.name}</p>
        </div>
        ${selected ? `<span class="skin-option-check">✓</span>` : ""}
      </article>
    `;
  }).join("");

  return `
    <section class="skin-selector">
      <p class="skin-selector-title">Skin</p>
      <div class="skin-selector-grid">${options}</div>
    </section>
  `;
}

export function attachSkinSelectorListeners(
  onSkinChange: (skinId: string) => void,
): void {
  document.querySelectorAll<HTMLElement>(".skin-option").forEach((el) => {
    el.addEventListener("click", () => {
      const skinId = el.dataset.skinId;
      if (skinId) onSkinChange(skinId);
    });
  });
}
