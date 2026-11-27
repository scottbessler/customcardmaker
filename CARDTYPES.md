# Card types and templates

How a card is described, and how that description becomes a picture. `SPEC.md` says
what the app does; this says how the two halves of a card are kept apart, so that a new
kind of card, or a new look, is data and not new code in five places.

Two ideas, one rule between them:

- A **card type** says *what a card has*: which fields, and which of them a finished
  card must have. It has nothing to say about looks.
- A **template** says *how a card is drawn*: it names the card types it works with and
  puts each of their fields somewhere on the card. It has nothing to say about what is
  valid.
- A **card** picks a card type, and then a template that works with it.

## Card types

A card type is a list of fields. Each field has a **key**, a **kind** (how the editor
asks for it and what it may hold), and whether it is **required** for a finished card.
A draft may be empty or half done; "valid" means every field is allowed and in bounds,
"complete" means every required field is present, and the editor shows what is missing.

| Kind | Holds | Editor |
| --- | --- | --- |
| `line` | one line of text | text input |
| `cost` | braced mana symbols, `{2}{U}` | input + symbol palette |
| `typeline` | the type line | input + type builder |
| `text` | paragraphs, symbols allowed | textarea + symbol palette |
| `stat` | a short value: `2/3`, `4`, `X` | input |
| `art` | an upload id, or none | upload / find art |

The built-in types (`src/cardtype.rs`):

| Id | Fields (required in bold) | May have a back face |
| --- | --- | --- |
| `spell` | **name**, mana_cost, **type_line**, rules_text, flavor_text, artist | yes |
| `creature` | `spell`'s, plus **power_toughness** | yes |
| `planeswalker` | `spell`'s, plus **loyalty** | yes |
| `token` | `creature`'s without mana_cost | no |

Presence of a back face is a property of the type, not a type of its own: a card whose type
allows it may carry a `back`, which is a card of one of the single-face types and has no
`back` of its own. That, and which fields a type lists, are the "big optionalities".

A saga is not a type: it has exactly a `spell`'s fields and differs only in how they are
drawn (chapters), so it is a *template* that works with `spell`. A split or an adventure
card, when they come, will be types (`double-faced`-like, with two halves on one face).

Every card also has fields that belong to no type because every card has them:
`rarity`, `frame` (a colour forced by the author; empty means from the mana cost),
`template`, and the picture flags `art_is_card` and `keep_printed_text`.

### Wire shape

A card is still one flat JSON object, one row. Its keys are the common fields above, a
`cardtype`, and the keys its type lists:

```json
{ "cardtype": "creature", "template": "", "rarity": "rare", "frame": "",
  "name": "Day", "mana_cost": "{1}{W}", "type_line": "Creature — Human",
  "rules_text": "...", "power_toughness": "1/1", "art": "…uuid…", "back": { … } }
```

Keys the type does not list are refused (`400`), and a missing key is an empty one, so
the shape grows by adding a field to a type with no migration. A double-faced card is a card with a
`back`.

### Old documents

Documents written before types existed have no `cardtype`; they carry a `layout`
(`normal`, `token`, `planeswalker`, `saga`) and fixed fields. They are read as: `token` →
`token`; `planeswalker` → `planeswalker`; `saga` → `spell` with template `saga`;
`normal` → `creature` if it has power/toughness, else `spell`. This happens on read, so no
row is rewritten until it is next saved.

## Templates

A template is a file in `public/templates/`: restricted XML, so the browser and the server
read it with the parsers they already have, and so that one a person wrote could be
accepted later by checking it against a whitelist. Built-ins only for now.

```xml
<template id="nexus-planeswalker" label="Planeswalker" cardtypes="planeswalker"
          frame="modern">
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
  its `cardtypes` all have; a template that names a field a type lacks is rejected when it
  is loaded, which is what keeps "works with" honest.
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

| Id | Works with | What it is |
| --- | --- | --- |
| `modern` | spell, creature | the modern frame (today's look) |
| `classic` | spell, creature | heavier border, flat plates |
| `borderless` | spell, creature | frame to the edge |
| `full-art` | spell, creature | picture is the card, plates float on it |
| `planeswalker` | planeswalker | tall art, abilities over its foot, loyalty badge |
| `saga` | spell | chapters down the left, art on the right |
| `token` | token | picture behind everything, centred name |
| `scan-*` | the same as above | the Nexus overlay for each of those |

### A card chooses its template

`card.template` names one, and it must work with the card's type (a `400` otherwise,
checked against the registry). Empty means the set's default *for that type*: a set keeps
a default template per card type, so a set that wants `classic` for its creatures still
gets a planeswalker frame for its planeswalkers. A picture that is a whole card picks the
matching `scan-*` template, which is what `art_is_card` selects today.

## Staging

1. **Types and the generic card.** `src/cardtype.rs`; the card carries `cardtype`; fields
   are validated against the type; the editor's form is built from the type; the import
   chooses a type; old documents read as above. Drawing is unchanged.
2. **Templates.** The XML format and its loader; one renderer that reads a template instead
   of the per-layout CSS; the built-ins above, ported from the current geometry; a card
   picks its template and the set keeps a default per type.
3. **Later.** Split / adventure / flip / modal types; custom templates per account (an
   editor, storage, and the whitelist enforced on save).
