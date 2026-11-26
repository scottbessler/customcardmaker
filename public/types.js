// The type line as parts: supertypes, card types, subtypes. Pure and DOM-free;
// the type line text stays the one stored value, and these only read and write it.

import { TYPE_DATA } from "/public/type-data.js";

/** Offered as one-tap choices. Others can still be typed into the type line. */
export const SUPERTYPES = ["Basic", "Legendary", "Snow", "World", "Ongoing"];
export const CARD_TYPES = ["Artifact", "Battle", "Creature", "Enchantment", "Instant", "Kindred", "Land", "Planeswalker", "Sorcery"];

// A type that is not a card type is a supertype when it is one of these.
const KNOWN_SUPERTYPES = new Set([...SUPERTYPES, "Elite", "Token", "Host"].map((word) => word.toLowerCase()));

/** Which subtypes make sense after which card type. */
const SUBTYPES = {
  Creature: TYPE_DATA.creature,
  Kindred: TYPE_DATA.creature,
  Artifact: TYPE_DATA.artifact,
  Enchantment: TYPE_DATA.enchantment,
  Land: TYPE_DATA.land,
  Planeswalker: TYPE_DATA.planeswalker,
  Instant: TYPE_DATA.spell,
  Sorcery: TYPE_DATA.spell,
  Battle: ["Siege"],
};

const ALL_SUBTYPES = [...new Set(Object.values(SUBTYPES).flat())].toSorted((a, b) => a.localeCompare(b));
// Looked up by lower case, so `time lord` finds `Time Lord`.
const CANONICAL = new Map(ALL_SUBTYPES.map((name) => [name.toLowerCase(), name]));
const SEPARATOR = " — ";
const SEPARATOR_PATTERN = /\s+[—–-]\s+/;

/**
 * Reads a type line. Everything before the dash is a supertype or a card type;
 * everything after is subtypes, read greedily so `Time Lord` stays one.
 * Nothing is dropped: a word nobody recognises is kept as a type or subtype.
 */
export function parseTypeLine(line) {
  const [left = "", ...rest] = String(line ?? "").split(SEPARATOR_PATTERN);
  const supertypes = [];
  const types = [];
  for (const word of left.split(/\s+/).filter(Boolean)) {
    (KNOWN_SUPERTYPES.has(word.toLowerCase()) ? supertypes : types).push(canonicalWord(word));
  }
  const words = rest.join(" ").split(/\s+/).filter(Boolean);
  const subtypes = [];
  for (let i = 0; i < words.length; i += 1) {
    const pair = i + 1 < words.length ? `${words[i]} ${words[i + 1]}` : "";
    if (pair && CANONICAL.has(pair.toLowerCase())) {
      subtypes.push(CANONICAL.get(pair.toLowerCase()));
      i += 1;
    } else {
      subtypes.push(CANONICAL.get(words[i].toLowerCase()) ?? words[i]);
    }
  }
  return { supertypes, types, subtypes };
}

function canonicalWord(word) {
  const known = [...SUPERTYPES, ...CARD_TYPES].find((name) => name.toLowerCase() === word.toLowerCase());
  return known ?? word;
}

/** The line for some parts. Subtypes only follow a dash that something precedes. */
export function buildTypeLine({ supertypes, types, subtypes }) {
  const left = [...supertypes, ...types].join(" ");
  const right = subtypes.join(" ");
  if (!right) return left;
  return left ? `${left}${SEPARATOR}${right}` : right;
}

/** Adds or removes `name` from a list, case-insensitively. */
export function toggled(list, name) {
  const has = list.some((item) => item.toLowerCase() === name.toLowerCase());
  return has ? list.filter((item) => item.toLowerCase() !== name.toLowerCase()) : [...list, name];
}

/** `elf` -> `Elf`, `time lord` -> `Time Lord`: what a typed subtype is stored as. */
export function titleCase(text) {
  const known = CANONICAL.get(text.trim().toLowerCase());
  if (known) return known;
  return text
    .trim()
    .split(/\s+/)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

/**
 * Up to `limit` subtypes to offer for what has been typed. With card types
 * chosen, only those types' subtypes; with none, all of them. Names that start
 * with the query come before names that merely contain it. Already-chosen
 * subtypes are left out.
 */
export function suggestSubtypes(types, query, chosen, limit = 8) {
  const pool = types.length
    ? [...new Set(types.flatMap((type) => SUBTYPES[type] ?? []))].toSorted((a, b) => a.localeCompare(b))
    : ALL_SUBTYPES;
  const needle = query.trim().toLowerCase();
  const taken = new Set(chosen.map((name) => name.toLowerCase()));
  const open = pool.filter((name) => !taken.has(name.toLowerCase()));
  if (!needle) return open.slice(0, limit);
  const starts = open.filter((name) => name.toLowerCase().startsWith(needle));
  const contains = open.filter((name) => !name.toLowerCase().startsWith(needle) && name.toLowerCase().includes(needle));
  return [...starts, ...contains].slice(0, limit);
}
