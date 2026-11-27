# Card features and templates

How a card is described, and how that description becomes a picture. `SPEC.md` says
what the app does; this says how the two halves of a card are kept apart, so that a new
kind of card, or a new look, is data and not new code in five places.

Two ideas, one rule between them:

- A card's **fields and features** say *what a card has*: which fields, and which of them
  a finished card must have. They have nothing to say about looks.
- A **template** says *how a card is drawn*: it names the features it needs and the ones it
  has nowhere to draw, and puts each field somewhere on the card. It has nothing to say about
  what is valid.
- A **card** turns on the features it has, and picks a template that can draw them.

## Fields and features

Every card has the **base fields**. On top of them a card turns on **features**: groups of
fields a card may or may not have. A card's "type", in the sense of what it has, is just the
set of features it has on, so a creature planeswalker is a card with both stat features and
needs no type of its own.

Each field has a **key**, a **kind** (how the editor asks for it and what it may hold), a
bound on its length, and whether it is **required** for a finished card. A draft may be empty
or half done; "valid" means every field is allowed and in bounds, "complete" means every
required field is present, and the editor shows what is missing.

| Kind | Holds | Editor |
| --- | --- | --- |
| `line` | one line of text | text input |
| `cost` | braced mana symbols, `{2}{U}` | input + symbol palette |
| `typeline` | the type line | input + type builder |
| `text` | paragraphs, symbols allowed | textarea + symbol palette |
| `stat` | a short value: `2/3`, `4`, `X` | input |

Defined once, in `src/cardtype.rs`:

| Where | Fields (required in bold) |
| --- | --- |
| base | **name**, **type_line**, rules_text, flavor_text, artist |
| feature `cost` (on for a new card) | mana_cost |
| feature `power_toughness` | **power_toughness** |
| feature `loyalty` | **loyalty** |
| feature `defense` | **defense** |

A token is a card with `cost` off; a creature is one with `power_toughness` on; a
planeswalker has `loyalty`; a battle has `defense`. A saga or a token is also a *template*:
they differ from other cards in how they are drawn, not in what they have. A back face is not
a feature but a card's own optional `back`, which is a whole card with its own features.

Every card also has fields that belong to no feature because every card has them:
`rarity`, `frame` (a colour forced by the author; empty means from the mana cost),
`template`, and the picture flags `art_is_card` and `keep_printed_text`.

### Wire shape

A card is still one flat JSON object, one row. Its keys are the common ones above, `features`,
and the keys its features allow:

```json
{ "features": ["cost", "power_toughness"], "template": "", "rarity": "rare", "frame": "",
  "name": "Day", "mana_cost": "{1}{W}", "type_line": "Creature — Human",
  "rules_text": "...", "power_toughness": "1/1", "art": "…uuid…", "back": { … } }
```

A key the features do not allow is refused (`400`), and a missing key is an empty one, so the
shape grows by adding a field to a feature, or a feature, with no migration. A back face is a
card of its own with no `back` of its own.

### Old documents

Documents written before features existed have no `features`; they carry a `layout`
(`normal`, `token`, `planeswalker`, `saga`) and every field whether the card used it or not.
They are read as: `cost` unless the layout was `token`; `power_toughness` if it was not empty;
`loyalty` if the layout was `planeswalker` or it was not empty; the `saga` and `token` layouts
become those templates. Fields the card has no place for are dropped. This happens on read, so
no row is rewritten until it is next saved.

## Templates

A template is a file in `public/templates/`: restricted XML, so the browser and the server
read it with the parsers they already have, and so that one a person wrote could be
accepted later by checking it against a whitelist. Built-ins only for now. The server embeds
the files (`src/template.rs`), checks every one when it is loaded, and sends them to each page
that draws a card (`<script id="templates">`); `public/templates.js` parses them there.

```xml
<template id="planeswalker" label="Planeswalker" requires="loyalty" frame="inherit">
  <plate role="name" x="4.6" y="5.4" w="90.8" h="10">
    <field name="name"/>
    <field name="mana_cost"/>
  </plate>
  <art x="7.5" y="15" w="85" h="63"/>
  <plate role="type" x="4.6" y="77.8" w="90.8" h="9.8">
    <field name="type_line"/>
  </plate>
  <box x="5.4" y="88" w="87.6" h="38.4">
    <field name="rules_text" as="abilities"/>
  </box>
  <stat x="79.5" y="124.4" w="15" h="9.8">
    <field name="loyalty"/>
  </stat>
</template>
```

- **Units.** Every `x`, `y`, `w`, `h` is in card-width units: the card is 100 wide and
  140 tall. Nothing is in pixels, so a card renders the same at any size. A place is from the
  card's own corner; the renderer takes the frame's offset off it, so one geometry serves every
  frame (and a scan, which has none).
- **Elements** (a closed list): `plate` (the tinted plates; `role="name"` or `role="type"`),
  `box` (the text box), `art`, `stat` (a badge showing the first of its fields the card has a
  value for). A `stat` may leave out `w` and `h` — both or neither — and is then as big as its
  value, anchored by its bottom-right corner, which is how a printed card's P/T box grows.
  An element may say `face="back"`, and draws the second face's fields in that place — that is
  how a split, adventure or prepared card is one card with two halves. Unknown elements and
  attributes are an error, not ignored.
- **`<field name="…"/>`** draws one field of the card, in the element that holds it. `as` picks
  a reader for the rules text: `as="abilities"` (loyalty costs beside each line),
  `as="chapters"` (a saga's numerals), plain by default. A template may name any field of the
  base and of the features it has not `excludes`; a card without the feature shows it empty. A
  template that names any other field is rejected when it is loaded, which is what keeps the
  rule honest.
- **`requires` / `excludes`** are space-separated ids. An id is a feature, or the reserved
  `back` — the card has a second face — which is what the two-halved layouts need.
- **`frame`** is `modern` (a frame of its own), or `inherit` (the card's own look, else the
  set's).
- **A body is the layout.** Every built-in template has one: `modern` places the name
  plate, the art, the type plate, the text box and the P/T badge where a real card has them,
  so the layout is data. A template with no body would draw none of its own parts (no
  built-in one is like that). The footer is not a template element: its place is a look's,
  not a layout's, so the stylesheet keeps it.
- **Style** is by `role` and class from the card stylesheet, not by raw CSS: a template says
  *where* things go and which part each is, and the stylesheet (with its tokens) says how a plate
  or a text box looks.

### Built-in templates

`requires` / `excludes` are features (or `back`); a card can wear a template when it has every
`requires` and none of the `excludes`.

| Id | Requires | Excludes | What it is |
| --- | --- | --- | --- |
| `modern` | | | the standard frame, laid out as a real card is (the one look so far) |
| `saga` | | stats | chapters down the left, art on the right |
| `token` | | cost | picture behind everything, centred name |
| `textless-token` | | cost | the picture, a name and a stat, and nothing else |
| `game-marker` | | cost | a name and a box for what it does; no art |
| `planeswalker` | loyalty | | tall art, abilities over its foot, loyalty badge |
| `battle` | defense | stats | the standard frame with a defense badge |
| `adventure` | back cost | | the card, with the adventure's own name and text beside it |
| `prepared` | back | | the card, with the prepared spell's text beside it |
| `omen` | back | | as prepared, with the omen's text given the wider box |
| `split` | back cost | | two halves, each a card, the art beside its name and text |
| `fuse` | back cost | | the same two halves, each art on its outer edge |
| `aftermath` | back cost | | two halves one above the other, each across the width |
| `semi-split` | back cost | | one art column down the card, both halves' plates beside it |
| `landscape` | back cost | | two halves side by side, each a card down the card |

The two-halved ones draw both faces at once, so the card is one document with a `back`
(see `card.back` above) and neither face gets the transform marker.

The layouts are our own drawings in Nexus's arrangement, not their artwork: a place is a
fraction of the card, and any of them can be tuned later.

A **scan** (a card whose picture is a whole card, `art_is_card`) needs no templates of its own:
it is drawn by the same template as any card, over its picture with no frame of its own, so a
scan and a native card share a geometry.

A card with `loyalty` is drawn by the `planeswalker` template whatever it names, and wears its
own look or the set's for its frame (`resolveTemplate` in `public/templates.js`). A card with
both stats draws the planeswalker layout (loyalty only); a template that shows both is not
written yet.

### A card chooses its template

`card.template` names one, and it must be able to draw what the card has (a `400` otherwise,
checked against the registry). Empty means the set's default look: only a template with no
`requires` and no `excludes` can be a set's default, since it must suit every card. A card
whose features only one layout can draw is given it (a card with loyalty is a planeswalker); a
layout that wants a second face is the author's to pick, never automatic, because a card that
can be turned over is not a card that is split in two. A picture that is a whole card is drawn
by the same template as any card, over its picture.

## Staging

1. **Features and the generic card** (done). `src/cardtype.rs`; the card carries `features`;
   fields are validated against them; the editor's form is built from them; the import turns
   on the right ones; old documents read as above. Drawing is unchanged.
2. **Templates** (in progress). Done: the XML format, its loader and checker (`src/template.rs`),
   the renderer (`public/templates.js`, `card-face.js`), the standard layout as a template
   (measured off a real card, so the art is the top half and the text box the bottom third),
   one look, and MTG Nexus's layouts as data — saga, token, textless token, game marker,
   planeswalker, battle, adventure, prepared, omen, split, fuse, aftermath, semi-split and
   landscape, the two-halved ones drawing the card's second face beside its own. To do: a
   default template per kind of card in a set, a template that draws both stats at once, and
   the transform crown on a scan.
3. **Later.** Custom templates per account (an editor, storage, and the whitelist enforced on
   save).
