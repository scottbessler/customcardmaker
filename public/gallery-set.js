import { html, render, useEffect, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";
import { CardFace } from "/public/card-face.js";

const setId = document.getElementById("app").dataset.setId;

function PublicSet() {
  const [set, setSet] = useState(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api("GET", `/api/gallery/${setId}`).then(setSet, (e) => setError(e.message));
  }, []);
  if (!set) return html`<p class="error" role="alert">${error}</p>`;
  const count = set.cards.length;
  return html`
    <p class="back"><a href="/gallery">← Gallery</a></p>
    <h1>${set.name}</h1>
    <p class="muted set-meta"><span class="set-code">${set.code}</span> · ${count} ${count === 1 ? "card" : "cards"} · by ${set.by}</p>
    <div class="public-cards">
      ${set.cards.map((c) => html`<div class="public-card" key=${c.position}>
        <${CardFace} card=${c.card} code=${set.code} number=${c.position} total=${count} />
      </div>`)}
    </div>
  `;
}

render(html`<${PublicSet} />`, document.getElementById("app"));
