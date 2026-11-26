# SPEC

The source of truth for what customcardmaker does. Code and tests follow this;
when they disagree, one of them is a bug and this says which.

## 1. What it is

A tool for designing **sets** of custom Magic cards. A designer makes a set,
adds cards to it, edits each with a live preview of the printed card, and (later)
exports the set. It is an installable PWA, used as much on a phone as on a desk.

Not a rules engine: it never interprets rules text. Not a card database: it knows
nothing about real Magic cards.

## 2. Model

- **Set** — `name` (1–64 chars), `code` (3–5 letters/digits, stored upper case),
  and an ordered list of cards.
- **Card** — one document: `name`, `mana_cost`, `type_line`, `rarity`
  (`common` | `uncommon` | `rare` | `mythic`), `rules_text`, `flavor_text`,
  `power_toughness`, `artist`. Every field may be empty: a blank card is a valid
  draft. Text fields are length-bounded (see `src/card.rs`); only `rules_text`
  and `flavor_text` may contain line breaks.
- **Collector number** — a card's 1-based position in its set. Adding appends;
  deleting closes the gap, so numbers are always `1..=n`.

## 3. Rendering

- **R1.** Mana symbols are written in braces: `{2}{U}{U}`, `{W/U}`, `{G/P}`, `{T}`,
  `{X}`. The same notation works in `mana_cost` and inside `rules_text`.
- **R2.** The frame is derived, never stored: one colour in the cost → that colour;
  two or more → gold; none → land if the type line says Land, else artifact if it
  says Artifact, else colourless. Hybrid and Phyrexian symbols count for each
  colour they name. (`public/frame.js`)
- **R3.** A card renders at any size from its container's width alone; nothing
  inside is measured in px. Rules text steps down in size with its length rather
  than measuring, so a card looks the same on every host.
- **R4.** The footer reads `NNN/TTT • CODE` and the artist credit.

## 4. Behaviour

- **B1.** The editor autosaves ~500 ms after the last keystroke, flushes before
  switching cards, adding or deleting, and on page hide. The set page shows
  `Unsaved changes` → `Saving…` → `Saved`.
- **B2.** Deleting a set deletes its cards. Deleting a card renumbers the rest.
- **B3.** `/api/*` failures are JSON `{"error": "..."}`; page failures are pages.
  Internal errors never put the database's message in a response.

## 5. Not yet built

Roughly in the order they make sense:

1. **Accounts.** Nothing is authenticated: anyone who can reach the server can
   edit every set. This blocks any public deploy. two-seven uses passkeys.
2. **Card art.** Upload (or link) an image per card; storage is an open question
   (volume vs. object storage).
3. **Export.** A printable sheet (PDF at 63×88 mm, 3×3 per page), per-card PNG,
   and a set JSON for backup/import.
4. **Richer frames.** Multicolour/hybrid blends, land colour identity, planeswalker,
   saga, split/transform, and card-specific layouts.
5. **Reordering** and sorting cards (by colour, curve, rarity).
6. **Set views.** Mana curve, colour balance and rarity counts; a draft/pack
   simulator.
7. **Sharing.** Read-only links to a set.
8. **Reminder text and keyword helpers; symbol autocomplete** in the editor.
