import { html } from "/public/vendor/htm-preact.js";
import { artUrl } from "/public/art.js";
import { abilities, chapters, costSymbols, frameOf, rulesSize, symbolKey, tokenize } from "/public/frame.js";

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

/** A planeswalker's abilities: a loyalty cost beside each line. */
function Abilities({ text }) {
  return html`<div class="pw-rows">${abilities(text).map(
    ({ cost, text: line }) => html`<div class=${`pw-row${cost ? "" : " pw-static"}`}>
      <span class="pw-cost">${cost}</span>
      <${Line} text=${line} />
    </div>`,
  )}</div>`;
}

/** A saga's reminder text, then each chapter beside its numeral(s). */
function Chapters({ text }) {
  const { reminder, chapters: list } = chapters(text);
  return html`<div class="saga-rows">
    ${reminder.length ? html`<div class="saga-reminder">${reminder.map((line) => html`<${Line} text=${line} />`)}</div>` : null}
    ${list.map(
      ({ marks, lines }) => html`<div class="saga-row">
        <span class="saga-marks">${marks.map((mark) => html`<i class="saga-mark">${mark}</i>`)}</span>
        <div class="saga-text">${lines.map((line) => html`<${Line} text=${line} />`)}</div>
      </div>`,
    )}
  </div>`;
}

/**
 * One printed card. Its size is its container's: every measurement inside is
 * in container-width units, so it renders the same at any scale.
 */
export function CardFace({ card, template = "modern", code = "", number = 0, total = 0, selected = false, onClick }) {
  const frame = frameOf(card);
  // A card that names a template wears it; otherwise it wears the set's.
  const look = card.template || template;
  // A whole-card picture is the card; the plates and text box go on top of it.
  const scan = card.art && card.art_is_card;
  // What is on the card and where; the template is how it is drawn.
  const layout = card.layout || "normal";
  const rules = card.rules_text ? card.rules_text.split("\n") : [];
  const flavor = card.flavor_text ? card.flavor_text.split("\n") : [];
  const collector = number ? `${String(number).padStart(3, "0")}${total ? `/${String(total).padStart(3, "0")}` : ""}` : "";
  const label = card.name || "Untitled card";
  const Tag = onClick ? "button" : "article";
  return html`<${Tag}
    class=${`mtg-card tpl-${look} lay-${layout} frame-${frame} rarity-${card.rarity}${scan ? " art-card" : ""}${scan && card.keep_printed_text ? " art-card-plain" : ""}${selected ? " selected" : ""}`}
    style=${scan ? `background-image:url(${artUrl(card.art)})` : undefined}
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
          ${layout === "planeswalker"
            ? html`<${Abilities} text=${card.rules_text} />`
            : layout === "saga"
            ? html`<${Chapters} text=${card.rules_text} />`
            : rules.map((text) => html`<${Line} text=${text} />`)}
          ${flavor.length && layout !== "planeswalker" && layout !== "saga" ? html`<div class="mtg-flavor">${flavor.map((text) => html`<p>${text}</p>`)}</div>` : null}
        </div>
      </div>
      ${layout === "planeswalker"
        ? card.loyalty ? html`<div class="mtg-pt mtg-loyalty">${card.loyalty}</div>` : null
        : layout !== "saga" && card.power_toughness ? html`<div class="mtg-pt">${card.power_toughness}</div>` : null}
      <footer class="mtg-foot">
        <span>${collector}${collector && code ? " • " : ""}${code}</span>
        <span>${card.artist ? `Illus. ${card.artist}` : ""}</span>
      </footer>
    </div>
  <//>`;
}
