# Mana (vendored)

`mana-v1-18-0.woff2` and `mana-v1-18-0.css` are from the **Mana** icon font by
Andrew Gioia, version 1.18.0 (https://mana.andrewgioia.com,
https://github.com/andrewgioia/mana), taken from the `mana-font` npm package.

| Part | Licence |
| --- | --- |
| The Mana font | SIL Open Font License 1.1 |
| Mana CSS | MIT |
| The mana, tap and card-type symbols it draws | **Copyright Wizards of the Coast** |

The font's licence does not cover the symbols themselves: they are Wizards of
the Coast's artwork. This app is for personal, non-commercial use (see the
licensing note in `TODO.md`), is an unofficial fan tool, and says so in its
footer.

## Updating

Fetch the new release, replace both files under a **new version in the
filename**, and change the `@font-face` `src` inside the CSS, the entry in
`STYLESHEETS` in `src/render.rs`, and `SYMBOL_KEYS` in `public/frame.js` (the
`every symbol has a glyph` e2e test fails if a key is missing from the font).
