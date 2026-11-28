import { html, render, useEffect, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";
import { CardFace } from "/public/card-face.js";

const query = new URLSearchParams(location.search).get("q")?.trim() ?? "";

function Gallery() {
  const [sets, setSets] = useState(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api("GET", query ? `/api/gallery?${new URLSearchParams({ q: query })}` : "/api/gallery").then(setSets, (e) => setError(e.message));
  }, []);
  return html`
    <h1>Gallery</h1>
    <form class="row gallery-search" role="search" action="/gallery" method="get">
      <label>Search public sets
        <input type="search" name="q" placeholder="Set name, code, or maker" defaultValue=${query} />
      </label>
      <button type="submit">Search</button>
      ${query ? html`<a href="/gallery">Clear search</a>` : null}
    </form>
    <p class="error" role="alert">${error}</p>
    ${sets === null
      ? null
      : sets.length === 0
        ? html`<p class="muted empty" role="status">${query ? "No public sets match your search." : "No public sets yet."}</p>`
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
