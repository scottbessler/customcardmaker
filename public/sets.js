import { html, render, useEffect, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";

function NewSet({ onCreated }) {
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function submit(event) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);
    setBusy(true);
    setError("");
    try {
      const set = await api("POST", "/api/sets", { name: data.get("name"), code: data.get("code") });
      form.reset();
      onCreated(set);
    } catch (e) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }
  return html`<form class="panel new-set" onSubmit=${submit}>
    <h2>New set</h2>
    <div class="row">
      <label>Name<input name="name" required maxlength="64" placeholder="Duskmourn Redux" autocomplete="off" /></label>
      <label>Code<input name="code" required minlength="3" maxlength="5" placeholder="DSR" autocomplete="off" /></label>
      <button type="submit" aria-busy=${busy ? "true" : "false"} disabled=${busy}>Create set</button>
    </div>
    <p class="error" role="alert">${error}</p>
  </form>`;
}

function Sets() {
  const [sets, setSets] = useState(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api("GET", "/api/sets").then(setSets, (e) => setError(e.message));
  }, []);
  return html`
    <div class="sets-bar">
      <h1>Your sets</h1>
      <a class="chip" href="/import">Import from Cockatrice</a>
    </div>
    <p class="error" role="alert">${error}</p>
    <${NewSet} onCreated=${(set) => { location.href = `/sets/${set.id}`; }} />
    ${sets === null
      ? null
      : sets.length === 0
        ? html`<p class="muted empty">No sets yet. Make one above and start adding cards.</p>`
        : html`<div class="set-grid">
            ${sets.map((set) => html`<a class="set-card" href=${`/sets/${set.id}`}>
              <strong>${set.code}</strong>
              <span>${set.name}</span>
              <small>${set.card_count} ${set.card_count === 1 ? "card" : "cards"}</small>
            </a>`)}
          </div>`}
  `;
}

render(html`<${Sets} />`, document.getElementById("app"));
