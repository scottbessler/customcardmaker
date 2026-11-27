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

- **Account** — a username (2–32 characters, unique ignoring case), an optional
  display name, and one or more passkeys. No passwords.
- **Set** — `name` (1–64 chars), `code` (3–5 letters/digits, stored upper case),
  an owner, and an ordered list of cards. A set belongs to exactly one account.
- **Card** — one document: `name`, `mana_cost`, `type_line`, `rarity`
  (`common` | `uncommon` | `rare` | `mythic`), `rules_text`, `flavor_text`,
  `power_toughness`, `artist`, `art` (an uploaded image, or none). Every field may be empty: a blank card is a valid
  draft. Text fields are length-bounded (see `src/card.rs`); only `rules_text`
  and `flavor_text` may contain line breaks.
- **Collector number** — a card's 1-based position in its set. Adding appends;
  deleting closes the gap, so numbers are always `1..=n`.

## 3. Accounts

- **A1.** Registering creates the account and signs you in; signing in needs the
  username and a passkey. A taken name (any case) is refused, an unknown one on
  sign-in is refused, both with a message and no side effects.
- **A2.** The session is a signed cookie. Signing out clears it. A cookie for an
  account that no longer exists is a signed-out visitor.
- **A3.** A signed-out visitor to `/` sees the two forms; to a set page, is sent
  to `/`; to `/api/*`, gets `401` with `{"error": ...}`.
- **A4.** An account sees and changes only its own sets. Anything that would
  reveal another account's set — read, edit, delete, add a card — answers `404`,
  never `403`, so a set's existence is not disclosed.
- **A5.** With `PASSKEY_DISABLED` set, the ceremony is skipped and the username
  alone signs in. Development and tests only.

## 4. Rendering

- **R1.** Mana symbols are written in braces: `{2}{U}{U}`, `{W/U}`, `{G/P}`, `{T}`,
  `{X}`. The same notation works in `mana_cost` and inside `rules_text`. Supported:
  generic 0–20, `W U B R G C`, `X Y Z`, `S` (snow), `E` (energy), `P`, `CHAOS`,
  `T` (tap) and `Q` (untap), hybrid (`W/U`, either order), two-colour generic
  (`2/W`), colourless hybrid (`C/W`), Phyrexian (`W/P`) and Phyrexian hybrid
  (`G/U/P`, either order). They are drawn with the Mana font (R6).
- **R6.** Symbols are the Mana font's, vendored under `public/vendor/`. Something
  typed in braces that the font has no symbol for is shown as typed, never as a
  blank disc. The footer says this is an unofficial fan tool and whose symbols
  they are; the sets are for personal, non-commercial use.
- **R2.** The frame defaults to what the cost says, and the author may override it with
  any of the nine (`card.frame`; empty means Auto). From the cost: one colour in the cost → that colour;
  two or more → gold; none → land if the type line says Land, else artifact if it
  says Artifact, else colourless. Hybrid and Phyrexian symbols count for each
  colour they name. (`public/frame.js`)
- **R3.** A card renders at any size from its container's width alone; nothing
  inside is measured in px. Rules text steps down in size with its length rather
  than measuring, so a card looks the same on every host.
- **R4.** The footer sits on the black border, bottom left, as two lines: `NNN/TTT • CODE`
  and the artist credit. The P/T box straddles the frame's lower right corner.
- **R7.** Each frame is drawn from three colours (a light edge, a dark edge, and a
  tint for the text box): bevelled plates for name and type, a tinted text box,
  and a hairline round each piece. Ink on every plate and text box keeps a WCAG
  contrast of at least 7. A development build serves `/dev/frames?template=<id>`, one card
  per frame; a production build has no such page.
- **R8.** A card wears a **template**: `modern`, `classic`, `borderless` or `full-art`
  (`card.template`; empty means the set's `default_template`, itself `modern` until
  changed). The editor shows a thumbnail of the card in each, and a colour chip for each
  frame colour plus Auto. An unknown template or frame is a 400. The gallery draws a
  public set with its default. (`src/card.rs`, `public/card-face.js`)
- **R5.** A card's art fills the art box, cropped to cover it and centred; a card
  with none shows the striped placeholder.

## 5. Art

- **U1.** An image is chosen in the creator, shrunk (longest side 1400px) and
  re-encoded as a JPEG in the browser, then sent as the request body. The
  browser's re-draw also drops metadata such as location and applies the
  photo's rotation.
- **U2.** The server believes the file's own signature, never the claimed type:
  only PNG, JPEG and WebP are accepted, up to 5 MB; anything else is `400`
  (`413` over the size). The response always carries `nosniff`.
- **U3.** An upload belongs to the account that made it. Its owner can always fetch
  it; anyone else, signed in or not, gets `404` unless a card in a *public* set
  uses it (G3). A card can only reference its owner's uploads, otherwise `400`.
- **U4.** An upload's id never changes what it names, so its response is cached
  by the owner's browser indefinitely (`private`, `immutable`).
- **U6.** *Find free art* searches Openverse through the server: signed-in accounts
  only, at most 30 searches per account per 10 minutes, identical searches answered
  from a one-minute memory, so the shared anonymous quota is not spent twice.
  Offered: CC0, the public domain mark, CC BY and CC BY-SA, plus the non-commercial
  ones only when asked for. NoDerivatives is never offered (a card crops its art),
  nor mature work, nor anything that is not a PNG or JPEG. An upstream
  "busy" or failure is reported plainly, as `429` / `502`.
- **U7.** Picking a result copies the image into our own storage as an upload (so U2
  and U3 apply to it) and keeps its **credit**: title, creator, licence and its link,
  and where it came from. The server contacts only the configured Openverse host,
  never a URL taken from a result, never follows a redirect elsewhere, and refuses
  an image over the upload cap or whose bytes are not a PNG, JPEG or WebP.
- **U8.** The credit is shown wherever borrowed art is: under the art in the editor
  and in an *Art credits* list on a public set's page. Picking art fills the card's
  *artist* from the credit only if it was empty.
- **U5.** Removing art from a card drops the reference; the file is kept
  (cleanup is a known gap, see TODO.md).

## 6. The gallery

- **G1.** A set is private when made. Its owner can mark it public from its settings,
  and take it private again at any time; both take effect at once. Only the owner can
  change it (anyone else gets `404`).
- **G2.** `/gallery` lists public sets that have at least one card, most recently
  touched first (at most 60), each with a sample card, its code, its card count and
  its maker. `/gallery/<id>` shows one public set, all its cards, read-only. Both
  are open to anyone, signed in or not. A private set's gallery page is `404`, the
  same as one that does not exist.
- **G3.** A card's art is visible to everyone exactly while a card in a public set uses
  it. Publishing a set shows its art and no other upload of the owner's, and taking
  it down hides that art again.
- **G4.** A public page shows the maker's **display name**, never their login name, and
  nothing else about them. Say so where the set is published.
- **G5.** The gallery is linked from every page's header.

## 7. Behaviour

- **B1.** The editor autosaves ~500 ms after the last keystroke, flushes before
  switching cards, adding or deleting, and on page hide. The set page shows
  `Unsaved changes` → `Saving…` → `Saved`.
- **B2.** Deleting a set deletes its cards. Deleting a card renumbers the rest.
  Both ask for confirmation first, naming what will be lost.
- **B4.** The creator shows the set as a filmstrip, the selected card as a live
  preview, and the form beside it (stacked on a phone). The strip, the preview
  and the form always show the same card.
- **B5.** The symbol palette inserts `{W}`, `{2}`, `{T}` and the rest at the
  caret of whichever of *mana cost* or *rules text* was last focused, and says
  which. `{T}` is not offered for a mana cost.
- **B6.** The type line is edited as parts as well as text. Supertype and card-type
  buttons toggle words in the line; subtypes are tokens with a typeahead. Both
  read and write the one `type_line` string, so typing it by hand and using the
  buttons always agree, and a word the buttons do not know is kept, never dropped.
- **B8.** Subtype suggestions come from the chosen card types (creature types for a
  Creature or Kindred, artifact types for an Artifact, and so on; all of them if
  none is chosen), names starting with what was typed first, minus those already
  chosen. A subtype that is not listed can still be added, capitalised. Enter or
  comma adds, arrows choose, Escape closes, Backspace in the empty box removes the
  last. A subtype may contain a space (`Time Lord`). The box is disabled until a
  type is chosen. Field limits come from the server (`src/card.rs`), not the page.
- **B7.** Duplicate copies the selected card to the end of the set.
- **B9.** *Select* puts the set in select mode: tapping a card picks it instead of
  opening it, the editor steps aside, and a bar shows how many are picked with
  Select all, None, Move, Copy and Delete (all but the first two need at least one
  card). *Done* or Escape leaves it, forgetting what was picked.
- **B10.** Deleting picked cards asks first, naming how many; the rest are
  renumbered `1..n`.
- **B11.** Move and Copy ask where to: another set the account has (for a move, not
  the one the cards are in) or a new set made in the same step. Moved or copied
  cards go to the end of the destination in the order they had; a move closes the
  gaps it leaves, a copy leaves the originals alone and makes new cards. A problem
  (a bad code, say) is shown in the dialog, which stays open.
- **B12.** A bulk edit is all or nothing: if any named card, or the destination, is
  not the account's or no longer exists, nothing changes (`404`, and no new set is
  left behind). It names between 1 and 500 cards; asking to move cards to the set
  they are already in is refused. (`POST /api/cards/delete`,
  `POST /api/cards/transfer`.)
- **B3.** `/api/*` failures are JSON `{"error": "..."}`; page failures are pages.
  Internal errors never put the database's message in a response.
- **B13.** `/import` takes a Cockatrice carddatabase XML file (the kind MTG Nexus
  exports) as an upload and makes a new set: each card's name, mana cost, type
  line, rules text, P/T and rarity are taken, in file order. A set code is
  required; the name defaults to the file's long name. When asked, each card's
  `picurl` is downloaded and kept as the importer's art; a picture that will not
  download, or is not an image of a kind we serve, is skipped and the card
  imports without art. MTG Nexus's pictures are whole printed cards, so the form
  asks whether they are; if so each card with a picture is marked `art_is_card`
  (B14). The set and its cards are written together or not at all,
  and only into the signed-in account. A signed-out visitor to `/import` is sent
  to `/`.

- **B14.** `card.art_is_card` says the card's picture is a whole printed card, not
  just art. Such a card is drawn with the picture edge to edge and our name plate,
  type plate and text box laid over it, in the modern frame's places and opaque, so
  they hide the words printed on the picture and show the card's own. It has no
  template (the picture brings its border); the frame colour still tints the plates.
  The editor has a checkbox under the art, and removing the art clears it.

## 8. Not yet built

Roughly in the order they make sense:

1. **Account recovery and more passkeys.** An account has the one passkey it
   registered with; losing it loses the account. Add a second device, and a
   recovery path.
2. **Art follow-ups.** Crop / focal point, cleaning up orphaned files, and
   replicating the files off the volume.
3. **Export.** A printable sheet (PDF at 63×88 mm, 3×3 per page), per-card PNG,
   and a set JSON for backup/import.
4. **Richer frames.** Multicolour/hybrid blends, land colour identity, planeswalker,
   saga, split/transform, and card-specific layouts.
5. **Reordering** and sorting cards (by colour, curve, rarity).
6. **Set views.** Mana curve, colour balance and rarity counts; a draft/pack
   simulator.
7. **Sharing.** Read-only links to a set.
8. **Reminder text and keyword helpers; symbol autocomplete** in the editor.
