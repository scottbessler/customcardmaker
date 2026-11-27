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
accepted later by checking it against a whitelist. Built-ins only for now.

```xml
<template id="nexus-planeswalker" label="Planeswalker" requires="loyalty" frame="modern">
  <plate  x="4.6" y="5.4" w="90.8" h="10" role="name">
    <field name="name" align="start"/>
    <field name="mana_cost"/>
  </plate>
  <art    x="7.5" y="15" w="85" h="63"/>
  <plate  x="4.6" y="77.8" w="90.8" h="9.8" role="type">
    <field name="type_line"/>
  </plate>
  <box    x="5.4" y="88" w="87.6" h="38.4">
    <field name="rules_text" as="abilities"/>
  </box>
  <stat   x="79.5" y="124.4" w="15" h="9.8"><field name="loyalty"/></stat>
</template>
```

- **Units.** Every `x`, `y`, `w`, `h` is in card-width units: the card is 100 wide and
  its height is 140. Nothing is in pixels, so a card renders the same at any size.
- **Elements** (a closed list): `plate` (the tinted name/type plates), `box` (the text
  box), `art`, `stat` (the P/T or loyalty badge), `foot` (the collector line), `group`
  (a container that places children relative to itself). Each has `x y w h`; unknown
  elements and attributes are an error, not ignored.
- **`<field name="…"/>`** draws one field of the card, in the element that holds it.
  `as` picks a reader for it: `as="abilities"` (loyalty costs beside each line),
  `as="chapters"` (a saga's numerals), plain by default. A template can only name fields
  of the base and of the features it `requires`; it lists in `excludes` the features it has
  nowhere to draw. A template that names any other field is rejected when it is loaded, which
  is what keeps "works with" honest.
- **`frame`** names the chrome (the bevelled border, or none): `modern`, `classic`,
  `borderless`, `full-art`, or `scan` (nothing; the card's picture is the frame).
- **Style** is by `role` and `class` from the card stylesheet, not by raw CSS: a template
  says *where* things go and which part each is, and the stylesheet (with its tokens)
  says how a plate or a text box looks. The few things a template may tune directly are a
  whitelist of properties on `style` (`font-size`, `text-align`, `padding`, `opacity`), all
  in card units.
- A **scan** template is the same thing with `frame="scan"`: opaque plates and a box laid
  where a Nexus card keeps its own, over the picture.

### Built-in templates

`requires` / `excludes` are features; a card can wear a template when it has every `requires`
and none of the `excludes`.

| Id | Requires | Excludes | What it is |
| --- | --- | --- | --- |
| `modern` | | | the modern frame (today's look) |
| `classic` | | | heavier border, flat plates |
| `borderless` | | | frame to the edge |
| `full-art` | | | picture is the card, plates float on it |
| `saga` | | stats | chapters down the left, art on the right |
| `token` | | cost | picture behind everything, centred name |
| `planeswalker` | loyalty | | tall art, abilities over its foot, loyalty badge |
| `scan-*` | the same as above | | the Nexus overlay for each of those |

Today a card with `loyalty` is drawn as a planeswalker whatever its template, and wears the
set's look for its frame; `planeswalker` becomes a template of its own in stage 2. A card with
both stats draws the planeswalker layout (loyalty only); a template that shows both is stage 2.

### A card chooses its template

`card.template` names one, and it must be able to draw the card's features (a `400`
otherwise, checked against the registry). Empty means the set's default look: only a template
with no `requires` and no `excludes` can be a set's default, since it must suit every card.
Stage 2 lets a set keep a default per kind of card. A picture that is a whole card picks the
matching `scan-*` template, which is what `art_is_card` selects today.

## Staging

1. **Features and the generic card** (done). `src/cardtype.rs`; the card carries `features`;
   fields are validated against them; the editor's form is built from them; the import turns
   on the right ones; old documents read as above. Drawing is unchanged.
2. **Templates.** The XML format and its loader; one renderer that reads a template instead
   of the per-layout CSS; the built-ins above, ported from the current geometry; a card
   picks its template and the set keeps a default per kind of card.
3. **Later.** Split / adventure / flip / modal; custom templates per account (an editor,
   storage, and the whitelist enforced on save).
