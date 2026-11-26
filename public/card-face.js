import { html } from "/public/vendor/htm-preact.js";
import { artUrl } from "/public/art.js";
import { costSymbols, frameOf, rulesSize, symbolKey, tokenize } from "/public/frame.js";

/**
 * One symbol, drawn by the Mana font. Something the font has no glyph for is
 * shown as typed, braces and all, so a typo is visible rather than a blank disc.
 */
export function Mana({ symbol }) {
  const key = symbolKey(symbol);
  if (!key) return html`<span class="mana-unknown">{${symbol}}</span>`;
  return html`<i class=${`mana ms ms-cost ms-shadow ms-${key}`} data-symbol=${symbol} role="img" aria-label=${symbol}></i>`;
}

function Line({ text }) {
  return html`<p>${tokenize(text).map((t) => (t.symbol ? html`<${Mana} symbol=${t.symbol} />` : t.text))}</p>`;
}

/**
 * One printed card. Its size is its container's: every measurement inside is
 * in container-width units, so it renders the same at any scale.
 */
export function CardFace({ card, code = "", number = 0, total = 0, selected = false, onClick }) {
  const frame = frameOf(card);
  const rules = card.rules_text ? card.rules_text.split("\n") : [];
  const flavor = card.flavor_text ? card.flavor_text.split("\n") : [];
  const collector = number ? `${String(number).padStart(3, "0")}${total ? `/${String(total).padStart(3, "0")}` : ""}` : "";
  const label = card.name || "Untitled card";
  const Tag = onClick ? "button" : "article";
  return html`<${Tag}
    class=${`mtg-card frame-${frame} rarity-${card.rarity}${selected ? " selected" : ""}`}
    aria-label=${label}
    aria-pressed=${onClick ? String(selected) : undefined}
    onClick=${onClick}
  >
    <div class="mtg-frame">
      <header class="mtg-name">
        <span class="mtg-title">${card.name || html`<em>Untitled</em>`}</span>
        <span class="mtg-cost">${costSymbols(card.mana_cost).map((s) => html`<${Mana} symbol=${s} />`)}</span>
      </header>
      <div class=${`mtg-art${card.art ? " has-art" : ""}`} style=${card.art ? `background-image:url(${artUrl(card.art)})` : undefined}></div>
      <div class="mtg-type"><span>${card.type_line}</span><i class="mtg-pip"></i></div>
      <div class="mtg-box">
        <div class="mtg-rules" style=${`font-size:${rulesSize(card)}cqw`}>
          ${rules.map((text) => html`<${Line} text=${text} />`)}
          ${flavor.length ? html`<div class="mtg-flavor">${flavor.map((text) => html`<p>${text}</p>`)}</div>` : null}
        </div>
      </div>
      ${card.power_toughness ? html`<div class="mtg-pt">${card.power_toughness}</div>` : null}
      <footer class="mtg-foot">
        <span>${collector}${collector && code ? " • " : ""}${code}</span>
        <span>${card.artist ? `Illus. ${card.artist}` : ""}</span>
      </footer>
    </div>
  <//>`;
}
