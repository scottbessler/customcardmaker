import { html, render, useEffect, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";
import { CardFace } from "/public/card-face.js";
import { Credit } from "/public/art-search.js";

const setId = document.getElementById("app").dataset.setId;

/** One card of a public set; a double-faced one can be turned over. */
function PublicCard({ entry, set, count }) {
  const [flipped, setFlipped] = useState(false);
  return html`<div class="public-card">
    <${CardFace} card=${entry.card} flipped=${flipped} template=${set.default_template} code=${set.code} number=${entry.position} total=${count} />
    ${entry.card.back ? html`<button type="button" class="quiet flip-card" onClick=${() => setFlipped(!flipped)}>Flip</button>` : null}
  </div>`;
}

function PublicSet() {
  const [set, setSet] = useState(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api("GET", `/api/gallery/${setId}`).then(setSet, (e) => setError(e.message));
  }, []);
  if (!set) return html`<p class="error" role="alert">${error}</p>`;
  const count = set.cards.length;
  // Borrowed art carries its credit; a licence asks for it to be shown.
  const credited = set.cards.flatMap((c) =>
    [c.card, c.card.back].filter((face) => face?.art && set.credits?.[face.art]).map((face) => ({ position: c.position, face })),
  );
  return html`
    <p class="back"><a href="/gallery">← Gallery</a></p>
    <h1>${set.name}</h1>
    <p class="muted set-meta"><span class="set-code">${set.code}</span> · ${count} ${count === 1 ? "card" : "cards"} · by ${set.by}</p>
    <div class="public-cards">
      ${set.cards.map((c) => html`<${PublicCard} key=${c.position} entry=${c} set=${set} count=${count} />`)}
    </div>
    ${credited.length
      ? html`<section class="art-credits" aria-label="Art credits">
          <h2>Art credits</h2>
          <ul>${credited.map((c) => html`<li key=${`${c.position}${c.face.name}`}><strong>${c.face.name || "Untitled"}</strong> — <${Credit} credit=${set.credits[c.face.art]} /></li>`)}</ul>
        </section>`
      : null}
  `;
}

render(html`<${PublicSet} />`, document.getElementById("app"));
