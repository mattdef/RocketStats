import "./styles.css";

const app = document.querySelector<HTMLDivElement>("#app");

if (!app) {
  throw new Error("missing #app root");
}

app.innerHTML = `
  <main class="overlay-shell">
    <section class="panel">
      <p class="eyebrow">RocketStats</p>
      <h1>Overlay starting</h1>
      <p class="muted">Waiting for backend state.</p>
    </section>
  </main>
`;
