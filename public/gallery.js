import { html, render, useEffect, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";
import { CardFace } from "/public/card-face.js";

function Gallery() {
  const [sets, setSets] = useState(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api("GET", "/api/gallery").then(setSets, (e) => setError(e.message));
  }, []);
  return html`
    <h1>Gallery</h1>
    <p class="muted">Sets their makers have shared, newest first.</p>
    <p class="error" role="alert">${error}</p>
    ${sets === null
      ? null
      : sets.length === 0
        ? html`<p class="muted empty">Nothing here yet. Mark a set public from its settings to put it here.</p>`
        : html`<div class="gallery-grid">
            ${sets.map((set) => html`<a class="gallery-tile" href=${`/gallery/${set.id}`} key=${set.id}>
              <div class="gallery-thumb">${set.preview ? html`<${CardFace} card=${set.preview} template=${set.default_template} code=${set.code} number=${1} total=${set.card_count} />` : null}</div>
              <strong class="gallery-name">${set.name}</strong>
              <span class="gallery-meta">${set.code} · ${set.card_count} ${set.card_count === 1 ? "card" : "cards"}</span>
              <span class="gallery-by">by ${set.by}</span>
            </a>`)}
          </div>`}
  `;
}

render(html`<${Gallery} />`, document.getElementById("app"));
