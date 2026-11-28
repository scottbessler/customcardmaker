import { html, useLayoutEffect, useRef } from "/public/vendor/htm-preact.js";
import { artBackground, artUrl } from "/public/art.js";
import { abilities, chapters, costSymbols, frameOf, reminders, rulesSize, symbolKey, tokenize } from "/public/frame.js";
import { parseTypeLine } from "/public/types.js";
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
 * One field being edited in place: a real input (or textarea) rendered inside the element that
 * shows the printed text, so it takes the card's own type and sits exactly where the text is.
 * `edit` says which fields the part has, their values and labels, and how to change or close.
 */
function Inline({ edit, name, multiline = false, placeholder }) {
  const ref = useRef(null);
  const first = edit.keys[0] === name;
  // Opening a part puts the caret at the end of its first field.
  useLayoutEffect(() => {
    if (!first) return;
    const el = ref.current;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }, []);
  const props = {
    ref,
    class: `card-inline card-inline-${name}`,
    value: edit.value(name),
    maxLength: edit.max(name),
    placeholder,
    "aria-label": edit.label(name),
    onInput: (event) => edit.change(name, event.currentTarget.value),
    onKeyDown: (event) => {
      if (event.key === "Escape" || (event.key === "Enter" && !multiline)) {
        event.preventDefault();
        event.stopPropagation();
        edit.close();
      }
    },
  };
  if (!multiline) return html`<input autocomplete="off" ...${props} />`;
  return html`<textarea rows=${Math.max(2, String(props.value).split("\n").length + 1)} ...${props}></textarea>`;
}

/** The printed text of a field, or its input while its part is being edited. */
function Slot({ edit, name, multiline, placeholder, children }) {
  if (!edit?.active || !edit.keys.includes(name)) return children;
  return html`<${Inline} edit=${edit} name=${name} multiline=${multiline} placeholder=${placeholder} />`;
}

/** What a part needs to be edited in place: a click target while it is not, its inputs while it is. */
const editable = (edit) => ({
  class: edit?.active ? " is-editing" : "",
  // Leaving the part (not just moving between its own fields) ends the edit. (Attached as
  // `onfocusout`: this Preact only maps names that are native properties, and `focusout` is not one.)
  onFocusOut: edit?.active ? (event) => {
    // Opening a part swaps out the button that was clicked, which loses focus on its way out;
    // that is not leaving.
    if (event.target.classList.contains("card-edit-target")) return;
    if (!event.currentTarget.contains(event.relatedTarget)) edit.close();
  } : undefined,
});
const target = (edit) => (edit && !edit.active
  ? html`<button type="button" class="card-edit-target" aria-label=${edit.title} title=${edit.title} onClick=${edit.open}></button>`
  : null);

/**
 * One element of a template's body, drawn with this face's fields — or the second half's,
 * when the element says `face="half"` (the split and adventure layouts).
 */
function Part({ part, card, turned, marker, symbol, editPart }) {
  const style = place(part);
  const source = part.face === "half" && card.half ? card.half : card;
  const field = (name) => part.fields.find((f) => f.name === name);
  if (part.tag === "art") {
    return html`<div class=${`mtg-art${source.art ? " has-art" : ""}${source.art && source.art_zoom > 100 ? " art-zoomed" : ""}`} style=${`${style}${source.art ? `;${artBackground(source)}` : ""}`}></div>`;
  }
  const edit = editPart?.(source, part.fields.map(({ name }) => name), part.face === "half");
  const { class: editing, onFocusOut } = editable(edit);
  const control = target(edit);
  if (part.tag === "plate" && part.role === "name") {
    return html`<header class=${`mtg-name${editing}`} data-part-face=${part.face ?? undefined} style=${style} onfocusout=${onFocusOut}>
      ${marker ? html`<i class=${`mtg-dfc ${turned ? "dfc-back" : "dfc-front"}`} aria-hidden="true"></i>` : null}
      <span class="mtg-title"><${Slot} edit=${edit} name="name" placeholder="Name">${source.name || html`<em>Untitled</em>`}<//></span>
      ${field("mana_cost") ? html`<span class="mtg-cost"><${Slot} edit=${edit} name="mana_cost" placeholder="{2}{U}">${costSymbols(source.mana_cost).map((s) => html`<${Mana} symbol=${s} />`)}<//></span>` : null}
      ${control}
    </header>`;
  }
  if (part.tag === "plate") return html`<div class=${`mtg-type${editing}`} data-part-face=${part.face ?? undefined} style=${style} onfocusout=${onFocusOut}><span><${Slot} edit=${edit} name="type_line" placeholder="Type line">${source.type_line}<//></span><i class="mtg-pip" data-symbol=${symbol} aria-hidden="true"></i>${control}</div>`;
  if (part.tag === "box") {
    const rules = field("rules_text");
    const flavor = field("flavor_text");
    const lines = flavor && source.flavor_text ? source.flavor_text.split("\n") : [];
    return html`<div class=${`mtg-box${editing}`} data-part-face=${part.face ?? undefined} style=${style} onfocusout=${onFocusOut}>
      <div class="mtg-rules" style=${`--rules-size:${rulesSize(source)}cqw`}>
        ${rules ? html`<${Slot} edit=${edit} name="rules_text" multiline placeholder="Rules text"><${Rules} card=${source} as=${rules.as} /><//>` : null}
        ${edit?.active && flavor
          ? html`<${Inline} edit=${edit} name="flavor_text" multiline placeholder="Flavor text" />`
          : lines.length ? html`<div class="mtg-flavor">${lines.map((text) => html`<p>${text}</p>`)}</div>` : null}
      </div>
      ${control}
    </div>`;
  }
  // A stat: the first of its fields the card has a value for. Its input stands in for it, and
  // an empty stat is shown only so there is something to click.
  const stat = part.fields.find((f) => source[f.name]);
  if (!stat && !control && !edit?.active) return null;
  const named = stat ?? part.fields.find((f) => edit?.keys.includes(f.name));
  const loyalty = named ? named.name === "loyalty" : field("loyalty");
  return html`<div class=${`mtg-pt${loyalty ? " mtg-loyalty" : ""}${stat ? "" : " card-stat-empty"}${editing}`} style=${style} onfocusout=${onFocusOut}>
    <${Slot} edit=${edit} name=${named?.name} placeholder="—">${stat ? source[stat.name] : html`<span aria-hidden="true">—</span>`}<//>${control}
  </div>`;
}

/**
 * One printed card. Its size is its container's: every measurement inside is
 * in container-width units, so it renders the same at any scale.
 */
export function CardFace({ card: whole, flipped = false, template = "modern", code = "", symbol = "circle", number = 0, total = 0, selected = false, onClick, onArt, editPart }) {
  // A double-faced card shows one face at a time; the back is a card of its own.
  const turned = flipped && whole.back;
  const card = turned ? whole.back : whole;
  // A back with no cost of its own keeps the front's colour, as printed ones do.
  const frame = card.frame || (turned && !card.mana_cost ? frameOf(whole) : frameOf(card));
  // Which template places the parts, and which look (frame) it wears; a back wears its own.
  // A whole-card picture is the card; the plates and text box go on top of it.
  const scan = card.art && card.art_is_card;
  const { supertypes, types } = parseTypeLine(card.type_line);
  const treatments = scan ? "" : [
    supertypes.includes("Legendary") ? " treatment-legendary" : "",
    types.includes("Enchantment") ? " treatment-enchantment" : "",
    types.includes("Artifact") ? " treatment-artifact" : "",
  ].join("");
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
    class=${`mtg-card tpl-${look} lay-${layout.id} frame-${frame} rarity-${card.rarity}${treatments}${whole.back ? " has-back" : ""}${scan ? " art-card" : ""}${scan && card.keep_printed_text ? " art-card-plain" : ""}${selected ? " selected" : ""}`}
    style=${scan ? `background-image:url(${artUrl(card.art)})` : undefined}
    aria-label=${label}
    aria-pressed=${onClick ? String(selected) : undefined}
    onClick=${onClick}
  >
    <div class="mtg-frame">
      ${layout.body.map((part) => html`<${Part} part=${part} card=${card} turned=${turned} marker=${marker} symbol=${symbol} editPart=${editPart} />`)}
      ${(() => {
        const edit = editPart?.(card, ["artist"], false);
        const { class: editing, onFocusOut } = editable(edit);
        return html`<footer class=${`mtg-foot${editing}`} onfocusout=${onFocusOut}>
          <span>${collector}${collector && code ? " • " : ""}${code}</span>
          <span><${Slot} edit=${edit} name="artist" placeholder="Artist">${card.artist ? `Illus. ${card.artist}` : ""}<//></span>
          ${target(edit)}
        </footer>`;
      })()}
    </div>
    ${onArt ? artZones.map(({ x, y, w, h, face }) => html`<div class="card-art-zone"
      style=${`left:${x}cqw;top:${y}cqw;width:${w}cqw;height:${h}cqw`}>
      <button type="button" class="card-art-control" onClick=${() => onArt(face === "half")}
        aria-label=${face === "half" ? "Edit second half art" : "Edit card art"}>Edit art</button>
    </div>`) : null}
  <//>`;
}
