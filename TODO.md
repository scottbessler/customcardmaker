# TODO

Worked top to bottom, one item per pull request. An item is done when it is in
`main` with its tests; tick it and move on. `SPEC.md` is updated by the item
that changes behaviour, not here.

## Queue

- [ ] **1. Image upload** — art on a card.
  - [ ] Upload from the creator; downscaled in the browser before it is sent
  - [ ] Stored on the volume under `DATA_PATH/art/`, row in `art`, owner-only
  - [ ] Shown in the card's art box (preview, filmstrip)
  - [ ] Remove art from a card
  - [ ] Follow-ups: crop / focal point, garbage-collect orphaned files
- [ ] **2. Official-looking cards** — mana icons and the rest of the frame.
  - [ ] Mana / tap symbols from an icon font or SVG set (see licensing note)
  - [ ] Real frame art per colour: borders, textured boxes, name/type plates
  - [ ] Rules text: reminder text in italics, symbol sizing, line spacing
  - [ ] Set symbol by rarity in the type line; collector footer layout
  - [ ] Proper P/T box, and layouts for planeswalker, saga, split, transform
- [ ] **3. Type line help** — dropdowns / typeahead for each part.
  - [ ] Supertype, card type(s), and subtypes as separate parts
  - [ ] Subtype typeahead filtered by the chosen card type
  - [ ] Still editable as free text; round-trips with `type_line`
- [ ] **4. MTG Nexus import** — bring an existing set in.
  - [ ] Blocked on a sample export; see the note below
  - [ ] Preview what will be created before it is, and report what was skipped
- [ ] **5. Bulk delete** — select several cards, delete them together.
- [ ] **6. Bulk move** — move or copy selected cards to a new or different set.

## Notes

- **Licensing (item 2).** The Magic symbols and frame art are Wizards of the
  Coast's. The community font *Mana* (Andrew Gioia, SIL OFL) redraws the
  symbols and is what most fan tools use; check its licence file is vendored
  with it. Frames are our own drawings in the same style, not scans.
- **Import (item 4).** MTG Nexus's export format isn't documented anywhere we
  can reach. Need one real exported set (any size) before this can be built.
- **Selection (items 5 and 6)** share one multi-select mode in the filmstrip;
  build it once for 5 and 6 reuses it.

## Backlog from earlier

Not asked for yet; here so they aren't lost.

- [ ] Deploy workflow: GitHub Actions `flyctl deploy` on push to `main`
- [ ] Account: second passkey, and a recovery path
- [ ] Back up the volume: Litestream for the database, and the art files
- [ ] Reorder cards; sort by colour, curve, rarity
- [ ] Export: printable PDF sheet, per-card PNG, set JSON
