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
  (`G/U/P`, either order). They are drawn with the Mana font (R6). A symbol beside
  text is centred on the middle of the capitals beside it, not on its line box (which
  has room for descenders and sits low); a palette key centres its symbol in the key.
  Both are pinned by measurement, not by a screenshot (`tests/e2e/symbols.spec.ts`).
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
- **R8.** A card wears a **template**: `modern` (the one look so far), or a layout — `saga`,
  `token`, `textless-token`, `game-marker`, `planeswalker`, `battle`, `adventure`, `prepared`,
  `omen`, `split`, `fuse`, `aftermath`, `semi-split`, `landscape` (`card.template`; empty means
  the set's `default_template`, itself `modern`, and only ever a look every card can wear). A
  template names what the card needs and what it cannot draw, and a card may not name one that
  cannot draw what it has (400). A two-halved layout (a split, adventure or prepared card) draws
  the card's second **half** beside its own, so the card is one document with a `half` — which is
  not a `back` (R13). The editor shows a thumbnail of the card in
  each, and a colour chip for each frame colour plus Auto. An unknown template or frame is a
  400. The gallery draws a public set with its default. (`src/card.rs`, `public/card-face.js`)
- **R12.** The standard layout is a real card's: the art box is the top half of the card
  and the text box the bottom third, with the name plate above the art and the type line
  between, all in card-width units so it renders at any size. (Its places are in
  `public/templates/modern.xml`.)
- **R5.** A card's art fills the art box, cropped to cover it and centred; a card
  with none shows the striped placeholder.
- **R11.** Reminder text — the words in parentheses in `rules_text` — is printed in
  italics; the rest of the line is not. Parentheses do not nest, so the first `)`
  closes the run. (`reminders` in `public/frame.js`)

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
  (B14). A token (by its type line), planeswalker or saga (by its type line or layout)
  is turned into the features and template that draw it (R9) and its loyalty is read; a layout we have no geometry for
  (split, flip, adventure...) gets `keep_printed_text`, so its picture shows as printed.
  A transform's two `<card>`s (the back marked `side=back`, each naming the other in
  `<related>`) become one card with a back face (R10), each face with its own picture; a
  back whose front is not in the file stays a card of its own. Rules text drops blank lines
  between paragraphs, and Nexus's `special` rarity is read as mythic. The set and its cards are written together or not at all,
  and only into the signed-in account. A signed-out visitor to `/import` is sent
  to `/`. The page previews first: it says what would be created, and what the file had
  that we could not keep as it was (a layout we cannot draw, a back face with no front, a
  rarity we do not have, a card with no picture), and creates nothing until the import is
  confirmed. A confirmed import runs in the background: the page is given a job to poll and
  shows a bar (cards read, pictures fetched of N), names the pictures that would not
  download so they can be retried, and a page left and come back to picks the job up
  again. One import runs at a time per account, and one that outlives ten minutes is
  stopped.

- **R9.** Every card has the base fields (name, type line, rules text, flavor text, artist)
  and turns on **features** (`card.features`: `cost`, `power_toughness`, `loyalty`,
  `defense`): groups of fields it may or may not have. A new card has `cost`. Which fields a
  card has, and which a finished card must have (name and type line; and each stat once it
  is on), come from its features alone (`src/cardtype.rs`, described in `CARDTYPES.md`), so a
  creature planeswalker is a card with two stats on and needs no type of its own. The
  editor's form shows exactly those fields and says what is still missing. A card is one flat
  object whose keys are the common ones and the fields its features allow; a key they do not
  allow is a 400, as is an unknown feature. Changing a card's features keeps what the two
  share and drops the rest. A document with no `features` is read as the card it was
  (`cost` unless it was a token; a stat if it had a value; `loyalty` for a planeswalker; a
  saga or token layout becomes that template). How a card is drawn is its **template** (R8),
  which must be able to draw its features. For now the layout follows from the features and
  template: a card with `loyalty` has tall art, abilities over its foot and a loyalty badge. With `power_toughness`, draw a
  second badge beside loyalty;
  the `token` template (no cost) puts the art behind everything; the `saga` template (no
  stats) has chapters down the left and art on the right. The places are those of a real MTG
  Nexus card, in card-width units, so a card drawn natively and a scan with our text laid
  over it (B14) share one geometry. A planeswalker's rules text is one ability per line,
  `+1: ...`, `−3: ...`, `0: ...`; a line with no cost, or a bare `: ...`, is a static
  ability. A saga's is reminder text, then `I: ...` / `II, III: ...` per chapter.
  (`public/frame.js`: `abilities`, `chapters`, `worksWith`.)
- **R10.** A card may have a **back face** (`card.back`), making it a double-faced (transform)
  card. The back is a card of its own — its own features, fields, art and template — but has
  no back of its own, and is checked like a front (a bad back is a 400). Its art follows the
  front's rules: only the owner's uploads may go on it, and it is public with the set. The
  editor has Front / Back tabs and Add / Remove back face; a card shows one face at a time
  and a Flip turns it, in the editor and in the public gallery. A back with no cost of its
  own wears the front's colour. A name plate carries an open ring on the front and a solid
  disc on the back (not over a scan, which has its own).
- **R13.** A card may have a **second half** (`card.half`), the other half of a split, adventure
  or prepared card. A half is a card of its own — its own features, fields, art and template —
  but it has no back and no half of its own, because it is read at once rather than turned to.
  A layout asks for one with the reserved requirement `half` (never with a feature), and the
  two-halved layouts draw the card and its half together; neither gets the transform marker.
  A **back** and a **half** are different things: a back (`card.back`) is a whole card you turn
  to (R10) and may itself have a half; a half is a whole card drawn beside this one.
- **B14.** `card.art_is_card` says the card's picture is a whole printed card, not
  just art. Such a card is drawn with the picture edge to edge and our name plate,
  type plate and text box laid over it, in a Nexus card's places and opaque, so
  they hide the words printed on the picture and show the card's own. It has no
  template (the picture brings its border); the frame colour still tints the plates.
  The places are those of a normal MTG Nexus card (400x560, measured off real exports),
  not our modern frame's. With `keep_printed_text` nothing of ours is drawn, for layouts
  we have no overlay for. The editor has a checkbox for each under the art, and removing
  the art clears both.

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
