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

/** The frame treatment: a colour, `gold` for several, or `land`/`artifact`/`colorless`. */
export function frameOf(card) {
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
