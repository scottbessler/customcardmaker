// Pure card logic: what frame a card wears, and how its text breaks into
// symbols. Nothing here touches the DOM, so the editor and the printed view
// can never disagree about what a cost means.

const WUBRG = ["W", "U", "B", "R", "G"];

/** `{2}{U}{U}` -> ["2", "U", "U"]. Anything outside braces is ignored. */
export function costSymbols(cost) {
  return [...String(cost ?? "").matchAll(/\{([^{}]+)\}/g)].map((m) => m[1].toUpperCase());
}

/** The colours a cost pays in, in WUBRG order. Hybrid and Phyrexian count for both sides. */
export function colorsOf(cost) {
  const seen = new Set();
  for (const symbol of costSymbols(cost)) {
    for (const part of symbol.split("/")) if (WUBRG.includes(part)) seen.add(part);
  }
  return WUBRG.filter((c) => seen.has(c));
}

/** The frame treatment: the one named on the card, else from its cost: a colour, `gold` for several, or `land`/`artifact`/`colorless`. */
export function frameOf(card) {
  // A frame the author chose wins; otherwise it follows from the cost and type.
  if (card.frame) return card.frame;
  const type = String(card.type_line ?? "").toLowerCase();
  const colors = colorsOf(card.mana_cost);
  if (colors.length > 1) return "gold";
  if (colors.length === 1) return colors[0].toLowerCase();
  if (type.includes("land")) return "land";
  if (type.includes("artifact")) return "artifact";
  return "colorless";
}

/** `{T}` and `{2}{G}` inside prose -> [{text}, {symbol}, ...]. */
export function tokenize(line) {
  const tokens = [];
  let last = 0;
  for (const match of String(line).matchAll(/\{([^{}]+)\}/g)) {
    if (match.index > last) tokens.push({ text: line.slice(last, match.index) });
    tokens.push({ symbol: match[1].toUpperCase() });
    last = match.index + match[0].length;
  }
  if (last < line.length) tokens.push({ text: line.slice(last) });
  return tokens;
}

/**
 * A line split into reminder text and the rest. Magic prints reminder text — the
 * words in parentheses — in italics. Parentheses do not nest in reminder text, so
 * the first `)` closes the run.
 */
export function reminders(line) {
  const text = String(line ?? "");
  const parts = [];
  let last = 0;
  for (const match of text.matchAll(/\(([^()]*)\)/g)) {
    if (match.index > last) parts.push({ text: text.slice(last, match.index), reminder: false });
    parts.push({ text: match[1], reminder: true });
    last = match.index + match[0].length;
  }
  if (last < text.length) parts.push({ text: text.slice(last), reminder: false });
  return parts;
}

/**
 * The symbols the Mana font draws, by its own names: `w`, `2`, `wu` (hybrid),
 * `2w`, `wp` (Phyrexian), `tap`. The `every symbol has a glyph` e2e test checks
 * each one against the font, so this list cannot drift from it.
 */
export const SYMBOL_KEYS = [
  ...Array.from({ length: 21 }, (_, n) => String(n)),
  "w", "u", "b", "r", "g", "c", "x", "y", "z", "s", "e", "p", "tap", "untap", "chaos",
  "wu", "wb", "ub", "ur", "br", "bg", "rw", "rg", "gw", "gu",
  "2w", "2u", "2b", "2r", "2g", "cw", "cu", "cb", "cr", "cg",
  "wp", "up", "bp", "rp", "gp",
  "wup", "wbp", "ubp", "urp", "brp", "bgp", "rwp", "rgp", "gwp", "gup",
];
const KNOWN = new Set(SYMBOL_KEYS);
const ALIASES = { t: "tap", q: "untap" };

/**
 * The font's name for what was typed between braces, or `null` if it has none.
 * `{T}` is tap, `{W/U}` and `{U/W}` are one hybrid symbol, `{G/U/P}` and
 * `{U/G/P}` one Phyrexian hybrid.
 */
export function symbolKey(symbol) {
  const raw = String(symbol).toLowerCase().replaceAll("/", "");
  const key = ALIASES[raw] ?? raw;
  if (KNOWN.has(key)) return key;
  // Either order of a pair names the same symbol; the font only has one.
  const swapped = key.length >= 2 ? key[1] + key[0] + key.slice(2) : key;
  return KNOWN.has(swapped) ? swapped : null;
}

/**
 * Rules text shrinks in steps rather than measuring, so the same card renders
 * the same size on every host. Returns a font size in container-width units.
 */
export function rulesSize(card) {
  const weight = String(card.rules_text ?? "").length + String(card.flavor_text ?? "").length * 0.8;
  if (weight <= 140) return 4.2;
  if (weight <= 220) return 3.8;
  if (weight <= 320) return 3.4;
  if (weight <= 450) return 3.0;
  return 2.6;
}

/**
 * A planeswalker's rules text, one ability per line: `+1: Draw a card`, `−X: ...`,
 * `0: ...`. A line with no loyalty cost (a static ability, or Nexus's bare `: ...`)
 * has an empty cost. A minus may be typed `-`, `–` or `−`; it is drawn as `−`.
 */
export function abilities(text) {
  return String(text ?? "")
    .split("\n")
    .filter((line) => line.trim())
    .map((line) => {
      const match = /^\s*([+\-–−]\s*(?:\d+|X)|0)\s*:\s*(.*)$/i.exec(line);
      if (match) return { cost: match[1].replace(/\s+/g, "").replace(/[-–]/, "−").toUpperCase(), text: match[2] };
      return { cost: "", text: line.replace(/^\s*:\s*/, "") };
    });
}

/**
 * A saga's rules text: reminder text first, then one line per chapter, `I: ...` or
 * `I, II: ...`. A line after a chapter that names none belongs to that chapter.
 * Chapters are numbered in Roman numerals only.
 */
export function chapters(text) {
  const reminder = [];
  const list = [];
  for (const line of String(text ?? "").split("\n")) {
    if (!line.trim()) continue;
    const match = /^\s*([IVX]+(?:\s*[,–-]\s*[IVX]+)*)\s*:\s*(.*)$/.exec(line);
    if (match) list.push({ marks: match[1].split(/\s*[,–-]\s*/), lines: [match[2]] });
    else if (list.length) list.at(-1).lines.push(line);
    else reminder.push(line.replace(/^\s*:\s*/, ""));
  }
  return { reminder, chapters: list };
}

/**
 * Whether a template can draw a card with these features on: it needs its `requires` and has
 * nowhere to put its `excludes`. (The server checks the same rule; this is for what the editor
 * offers.)
 */
export function worksWith(template, features) {
  return template.requires.every((id) => features.includes(id)) && !template.excludes.some((id) => features.includes(id));
}
