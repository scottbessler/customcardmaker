# TODO

Worked top to bottom, one pull request per item; a big item (2) is split into
one pull request per sub-item. An item is done when it is in
`main` with its tests; tick it and move on. `SPEC.md` is updated by the item
that changes behaviour, not here.

## Queue

- [x] **1. Image upload** — art on a card.
  - [x] Upload from the creator; downscaled in the browser before it is sent
  - [x] Stored on the volume under `DATA_PATH/art/`, row in `art`, owner-only
  - [x] Shown in the card's art box (preview, filmstrip)
  - [x] Remove art from a card
  - [ ] Follow-ups: crop / focal point, garbage-collect orphaned files, back up
        the art files (they are not replicated; see `STORAGE.md`)
- [ ] **2. Official-looking cards** — mana icons and the rest of the frame.
  - [x] Mana / tap symbols from the Mana font (see licensing note)
  - [x] Real frame art per colour: borders, textured boxes, name/type plates
        (`/frames` shows every frame; collector line and P/T box are placed as
        on a printed card)
  - [ ] Rules text: reminder text in italics, symbol sizing, line spacing
  - [ ] Set symbol by rarity in the type line; collector footer layout
  - [ ] Proper P/T box, and layouts for planeswalker, saga, split, transform
- [x] **3. Type line help** — dropdowns / typeahead for each part.
  - [x] Supertype, card type(s), and subtypes as separate parts
  - [x] Subtype typeahead filtered by the chosen card type
  - [x] Still editable as free text; round-trips with `type_line`
  - [ ] Follow-up: planeswalker-style cards need a loyalty field; and the
        non-standard card types (Conspiracy, Scheme, Plane...) are typed, not offered
- [ ] **4. Import** — bring an existing set in, from MTG Nexus.
  - [ ] **Cockatrice XML** (`cockatrice_carddatabase`), which MTG Nexus can
        export: parse sets and cards into a new set
  - [ ] Map every field we have; keep what we cannot map and say so
  - [ ] Preview what will be created before it is, and report what was skipped
  - [ ] More requirements are expected once we have a real MTG Nexus export;
        see the note below
  - [ ] Maybe later: export *to* Cockatrice XML, so sets round-trip
- [ ] **5. Bulk delete** — select several cards, delete them together.
- [ ] **6. Bulk move** — move or copy selected cards to a new or different set.
- [ ] **7. Export for proxy printing** — MakePlayingCards (MPC).
  - [ ] Render each card to a print-resolution PNG (and the card back)
  - [ ] Output MPC can take, or that
        [MPC Autofill](https://mpcfill.com/editor) can: use whichever is less
        work for the user; read both sites' current requirements first
  - [ ] Dimensions and bleed per MPC's own template, confirmed against it
  - [ ] Open question: render server-side or in the browser (the card is HTML/CSS,
        so the browser is where it is already drawn correctly)
  - [ ] Depends on 1 and 2: art and official frames are what make a proxy worth
        printing

## Notes

- **Licensing (item 2).** The Magic symbols are Wizards of the Coast's artwork.
  The *Mana* font (Andrew Gioia; font SIL OFL, CSS MIT) draws them and is what
  most fan tools use; the symbols themselves are not covered by its licence.
  The sets are for personal, non-commercial use, which is the footing the owner
  has taken from Wizards' proxies policy
  (https://magic.wizards.com/en/news/announcements/proxies-policy-and-communication-2016-01-14).
  Keep it that way: nothing here should sell or monetise cards. Frames are our
  own drawings in the same style, not scans.
- **Import (item 4).** Cockatrice's card database XML is a public format, so
  that part can be built without a sample. MTG Nexus quirks (how it writes
  custom sets, rarities, split/double-faced cards, art) will not be known until
  we have one real export; ask for one before finishing the item.
- **Proxy export (item 7).** Do not assume MPC's or MPC Autofill's required
  sizes, bleed, or file naming from memory; they change. Check them at build time.
- **Selection (items 5 and 6)** share one multi-select mode in the filmstrip;
  build it once for 5 and 6 reuses it.

## Backlog from earlier

Not asked for yet; here so they aren't lost.

- [ ] Deploy workflow: GitHub Actions `flyctl deploy` on push to `main`
- [ ] Account: second passkey, and a recovery path
- [ ] Back up the volume: Litestream for the database, and the art files
- [ ] Reorder cards; sort by colour, curve, rarity
- [ ] Export: printable PDF sheet, per-card PNG, set JSON
