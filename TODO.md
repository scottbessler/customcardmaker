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
  - [x] **Cockatrice XML** (`cockatrice_carddatabase`), which MTG Nexus can
        export: parse sets and cards into a new set
  - [x] Nexus's pictures are whole cards: an option to use them as such, with our
        text laid over them (`art_is_card`)
  - [ ] Map every field we have; keep what we cannot map and say so
  - [ ] Preview what will be created before it is, and report what was skipped
  - [ ] More requirements are expected once we have a real MTG Nexus export;
        see the note below
  - [ ] Maybe later: export *to* Cockatrice XML, so sets round-trip
- [x] **5. Bulk delete** — select several cards, delete them together.
- [x] **6. Bulk move** — move or copy selected cards to a new or different set.
  - [ ] Follow-ups: pick cards across several sets at once (the API already allows
        it); drag cards to reorder within a set
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

- [x] **8. Public sets and a gallery** — publish a set; anyone can browse them.
  - [x] A set can be marked public (and taken private again, instantly)
  - [x] `/gallery`: every public set, newest first, with a sample card; and a
        read-only page per set, for anyone, signed in or not
  - [x] The art of a card in a public set is visible to everyone; no other art is
  - [ ] Follow-ups: copy a public set into your own account; pagination and search;
        a way to report a set (needed before this is open to people you don't know)
- [x] **9. Creative Commons art search** — find art without leaving the app.
  - [x] Search Openverse from the card editor and pick a result as the card's art
  - [x] Keep the credit with the art (artist, licence, source link) and fill the
        card's artist line from it; shown in the editor and on public pages
  - [x] Only fetch what was picked, and serve it from our own storage like an upload
  - [ ] Follow-ups: Openverse OAuth credentials for a higher quota (anonymous is
        low and shared, and I hit its limit while building this); search by
        category; drag-to-reposition a borrowed image
- [ ] **10. Clean up rules text with OpenRouter** — suggest edits to match Oracle wording.
  - [ ] A "Suggest cleanup" action on the rules text; the model's answer is shown as
        a before/after the user accepts or rejects, never applied silently
  - [ ] Only the card's text is sent; the key stays on the server; a per-account rate limit
  - [ ] Works without a key configured (the button is simply absent), and is tested
        against a fake OpenRouter so no test spends money

- [ ] **11. Symbols still not quite vertically aligned** — found by eye, so measure
      *where*: the rules text, the mana cost in the name plate, the palette.
  - [ ] Decide what "centred" means for a symbol beside mixed-case text, check it
        at several sizes, and pin it with a measurement, not a screenshot
- [x] **12. Choose a frame / template, like MTG Nexus** — not only the one a card
      gets from its mana cost.
  - [x] A **frame colour** the author can set (or leave on Auto, from the cost):
        W U B R G, gold, artifact, land, colourless
  - [x] A **template**: more than one look for the same card (the modern frame we
        have, a classic old-border one, full-art, borderless...), picked from
        thumbnails of the card being edited
  - [x] Per card, with a default for the set; saved in the card document
  - [x] The `/frames` page makes no sense to a visitor: remove it from the app. Keep
        what it gave the tests (every frame rendered side by side) as a dev-only page
  - [x] Layouts: token, planeswalker (with loyalty) and saga, drawn natively and over
        a Nexus scan, in every template
  - [x] Transform / double-faced cards: a back face, Front/Back tabs and a Flip, linked on
        import by `<related>`
  - [ ] Still to do: Nexus draws a transform front with its own crown frame, which peeks out
        above our plate on a scan; measure it. Then split, adventure, flip and modal layouts
  - [ ] Still to do: match the real MTG Nexus template list (Nexus's creator refuses automated
        fetches, so ask for names or screenshots)

- [ ] **13. Progress for a large import** — a real Nexus export is ~300 cards and one
      picture download each, which can take minutes; the import page shows nothing until
      it ends.
  - [ ] Run the import in the background and report progress (cards read, pictures
        fetched of N, failures so far), e.g. a job the page polls or an event stream
  - [ ] Show it on the import page with a bar, and let the person leave and come back
  - [ ] Say at the end which pictures failed, so they can be retried
  - [ ] A cap on how long one import may run, and one import at a time per account

- [ ] **14. Card features and templates** — design in `CARDTYPES.md`. A card has the base
      fields (name, type line, rules text, flavor, artist) plus **features** it turns on
      (cost, power_toughness, loyalty, defense); a template says which features it
      needs/excludes and places the fields; a card picks a template that can draw it.
  - [x] Stage 1 (DONE): the registry (`src/cardtype.rs`), the generic card (`src/card.rs`,
        flat JSON: `features` + the fields they allow, old documents read as the card they
        were), the form built from the features, the import turning on the right ones, a
        card's template checked against its features
  - [ ] Stage 2 — IN PROGRESS (format and renderer are done; what is left is porting)
    - [x] XML format, `src/template.rs` loader/validator (closed element list, field and feature
          checks, 5 tests), files in `public/templates/`, embedded and sent to every card page
    - [x] renderer: `public/templates.js` (`allTemplates`, `resolveTemplate`), `card-face.js`
          draws a template with a body; planeswalker, saga and token are data now, their
          geometry removed from `04-cards.css` (only styling is left there)
    - [x] `nexus` template (a normal card as Nexus draws it, tall art): the scan overlay's
          geometry now comes from it (`resolveTemplate(..., scan)`), so `.art-card` in
          `04-cards.css` holds only styling; a scan of a planeswalker/saga/token already used
          their own templates. (`keep_printed_text` stays: "draw none of ours".)
    - [ ] port the four looks' standard grid layout (name plate, art, type plate, text box, P/T,
          foot; `.mtg-frame` grid in `04-cards.css` for modern/classic/borderless/full-art) into
          `<template>` bodies so a look is data too; keep the e2e image/geometry baselines green.
          Lowest value of what is left (they are our own looks, not Nexus layouts): do last
    - [ ] a set keeps a default template per kind of card (today one look for all, only a look)
    - [ ] both stats at once: a template that draws power/toughness and loyalty (creature
          planeswalker); today the planeswalker layout shows loyalty only
    - [ ] delete what the above makes dead: the `.lay-*` position-group CSS rule once the looks are
          bodies
  - [ ] Later: split / adventure / flip / modal; custom templates per account (editor,
        per-account storage, the whitelist enforced on save)

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
- **CC art (item 9).** Prefer CC0 and CC BY; CC BY-SA needs the share-alike carried
  on, and NoDerivatives forbids the cropping a card does, so leave those out of the
  results. The sets are personal and non-commercial, but still keep the credit.
- **Rules-text cleanup (item 10).** Needs an `OPENROUTER_API_KEY` secret and a chosen
  model; the model's output is advice, so it is validated (length, no symbols
  invented) before it is even offered.
- **Selection (items 5 and 6)** share one multi-select mode in the filmstrip;
  build it once for 5 and 6 reuses it.

## Backlog from earlier

Not asked for yet; here so they aren't lost.

- [ ] Deploy workflow: GitHub Actions `flyctl deploy` on push to `main`
- [ ] Account: second passkey, and a recovery path
- [ ] Back up the volume: Litestream for the database, and the art files
- [ ] Reorder cards; sort by colour, curve, rarity
- [ ] Export: printable PDF sheet, per-card PNG, set JSON
