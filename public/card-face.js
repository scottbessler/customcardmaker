import { html } from "/public/vendor/htm-preact.js";
import { artBackground, artUrl } from "/public/art.js";
import { abilities, chapters, costSymbols, frameOf, reminders, rulesSize, symbolKey, tokenize } from "/public/frame.js";
import { resolveTemplate } from "/public/templates.js";

/**
 * One symbol, drawn by the Mana font. Something the font has no glyph for is
 * shown as typed, braces and all, so a typo is visible rather than a blank disc.
 */
export function Mana({ symbol }) {
  const key = symbolKey(symbol);
  if (!key) return html`<span class="mana-unknown">{${symbol}}</span>`;
  return html`<i class=${`mana ms ms-cost ms-shadow ms-${key}`} data-symbol=${symbol} role="img" aria-label=${symbol}></i>`;
}

/** One line of rules text: symbols drawn, reminder text in italics. */
const tokens = (text) =>
  tokenize(text).map((t) => (t.symbol ? html`<${Mana} symbol=${t.symbol} />` : t.text));

function Line({ text }) {
  return html`<p>${reminders(text).map((part) =>
    part.reminder ? html`<i class="reminder">${tokens(part.text)}</i>` : tokens(part.text),
  )}</p>`;
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
/**
 * Where a template puts an element, in card-width units from the frame's own corner.
 * An element with no w/h (a <stat> that is as big as its value) is anchored by its
 * bottom-right corner instead, which is where a printed card's P/T box grows from.
 */
const place = ({ x, y, w, h }) =>
  w == null || h == null
    ? `position:absolute;right:calc(100cqw - var(--fx) - ${x}cqw);bottom:calc(var(--card-h) - var(--fb) - ${y}cqw);left:auto;top:auto`
    : `position:absolute;left:calc(${x}cqw - var(--fx));top:calc(${y}cqw - var(--fy));width:${w}cqw;height:${h}cqw;right:auto;bottom:auto`;

/** The rules text, read the way a template says (plain, as a planeswalker's abilities, a saga's chapters). */
function Rules({ card, as }) {
  if (as === "abilities") return html`<${Abilities} text=${card.rules_text} />`;
  if (as === "chapters") return html`<${Chapters} text=${card.rules_text} />`;
  return (card.rules_text ? card.rules_text.split("\n") : []).map((text) => html`<${Line} text=${text} />`);
}

/**
 * One element of a template's body, drawn with this face's fields — or the second half's,
 * when the element says `face="half"` (the split and adventure layouts).
 */
function Part({ part, card, turned, marker }) {
  const style = place(part);
  const source = part.face === "half" && card.half ? card.half : card;
  const field = (name) => part.fields.find((f) => f.name === name);
  if (part.tag === "art") {
    return html`<div class=${`mtg-art${source.art ? " has-art" : ""}${source.art && source.art_zoom > 100 ? " art-zoomed" : ""}`} style=${`${style}${source.art ? `;${artBackground(source)}` : ""}`}></div>`;
  }
  if (part.tag === "plate" && part.role === "name") {
    return html`<header class="mtg-name" data-part-face=${part.face ?? undefined} style=${style}>
      ${marker ? html`<i class=${`mtg-dfc ${turned ? "dfc-back" : "dfc-front"}`} aria-hidden="true"></i>` : null}
      <span class="mtg-title">${source.name || html`<em>Untitled</em>`}</span>
      ${field("mana_cost") ? html`<span class="mtg-cost">${costSymbols(source.mana_cost).map((s) => html`<${Mana} symbol=${s} />`)}</span>` : null}
    </header>`;
  }
  if (part.tag === "plate") return html`<div class="mtg-type" data-part-face=${part.face ?? undefined} style=${style}><span>${source.type_line}</span><i class="mtg-pip"></i></div>`;
  if (part.tag === "box") {
    const rules = field("rules_text");
    const flavor = field("flavor_text");
    const lines = flavor && source.flavor_text ? source.flavor_text.split("\n") : [];
    return html`<div class="mtg-box" data-part-face=${part.face ?? undefined} style=${style}>
      <div class="mtg-rules" style=${`--rules-size:${rulesSize(source)}cqw`}>
        ${rules ? html`<${Rules} card=${source} as=${rules.as} />` : null}
        ${lines.length ? html`<div class="mtg-flavor">${lines.map((text) => html`<p>${text}</p>`)}</div>` : null}
      </div>
    </div>`;
  }
  // A stat: the first of its fields the card has a value for.
  const stat = part.fields.find((f) => source[f.name]);
  if (!stat) return null;
  return html`<div class=${`mtg-pt${stat.name === "loyalty" ? " mtg-loyalty" : ""}`} style=${style}>${source[stat.name]}</div>`;
}

/**
 * One printed card. Its size is its container's: every measurement inside is
 * in container-width units, so it renders the same at any scale.
 */
export function CardFace({ card: whole, flipped = false, template = "modern", code = "", number = 0, total = 0, selected = false, onClick, onArt }) {
  // A double-faced card shows one face at a time; the back is a card of its own.
  const turned = flipped && whole.back;
  const card = turned ? whole.back : whole;
  // A back with no cost of its own keeps the front's colour, as printed ones do.
  const frame = card.frame || (turned && !card.mana_cost ? frameOf(whole) : frameOf(card));
  // Which template places the parts, and which look (frame) it wears; a back wears its own.
  // A whole-card picture is the card; the plates and text box go on top of it.
  const scan = card.art && card.art_is_card;
  const { layout, look } = resolveTemplate(card.template ? card : { ...card, template: whole.template }, template);
  // A layout that places the second half draws both at once (a split card), so neither
  // gets the transform marker; one that does not is a card you turn over.
  const paired = layout.body.some((part) => part.face === "half");
  const marker = Boolean(whole.back && !paired);
  const collector = number ? `${String(number).padStart(3, "0")}${total ? `/${String(total).padStart(3, "0")}` : ""}` : "";
  const label = card.name || "Untitled card";
  const Tag = onClick ? "button" : "article";
  const artZones = scan ? [{ x: 0, y: 0, w: 100, h: 140 }] : layout.body.filter((part) => part.tag === "art");
  return html`<${Tag}
    data-face=${whole.back ? (turned ? "back" : "front") : undefined}
    class=${`mtg-card tpl-${look} lay-${layout.id} frame-${frame} rarity-${card.rarity}${whole.back ? " has-back" : ""}${scan ? " art-card" : ""}${scan && card.keep_printed_text ? " art-card-plain" : ""}${selected ? " selected" : ""}`}
    style=${scan ? `background-image:url(${artUrl(card.art)})` : undefined}
    aria-label=${label}
    aria-pressed=${onClick ? String(selected) : undefined}
    onClick=${onClick}
  >
    <div class="mtg-frame">
      ${layout.body.map((part) => html`<${Part} part=${part} card=${card} turned=${turned} marker=${marker} />`)}
      <footer class="mtg-foot">
        <span>${collector}${collector && code ? " • " : ""}${code}</span>
        <span>${card.artist ? `Illus. ${card.artist}` : ""}</span>
      </footer>
    </div>
    ${onArt ? artZones.map(({ x, y, w, h, face }) => html`<div class="card-art-zone"
      style=${`left:${x}cqw;top:${y}cqw;width:${w}cqw;height:${h}cqw`}>
      <button type="button" class="card-art-control" onClick=${() => onArt(face === "half")}
        aria-label=${face === "half" ? "Edit second half art" : "Edit card art"}>Edit art</button>
    </div>`) : null}
  <//>`;
}
