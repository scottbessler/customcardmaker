// The built-in templates (see CARDTYPES.md): restricted XML the server embeds in every page that
// draws a card. This reads them, and decides which one draws a given card. Pure apart from reading
// the page's own JSON, so the editor, the gallery and the dev frames page agree.
import { worksWith } from "/public/frame.js";

const FRAMES = new Set(["modern"]);

let parsed = null;

/** The elements of a template's body: `{tag, role, x, y, w, h, fields: [{name, as}]}`. */
function body(xml) {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  return [...doc.documentElement.children].map((el) => {
    const part = { tag: el.tagName, role: el.getAttribute("role"), face: el.getAttribute("face") };
    // A <stat> may leave out w/h: null means "as big as its value".
    for (const side of ["x", "y", "w", "h"]) part[side] = el.hasAttribute(side) ? Number(el.getAttribute(side)) : null;
    part.fields = [...el.children].map((field) => ({ name: field.getAttribute("name"), as: field.getAttribute("as") }));
    return part;
  });
}

/** Every template: `{id, label, requires, excludes, frame, xml, body}`; `body` is empty for a frame. */
export function allTemplates() {
  if (!parsed) {
    const script = document.getElementById("templates");
    parsed = JSON.parse(script?.textContent ?? "[]");
    for (const template of parsed) template.body = body(template.xml);
  }
  return parsed;
}

/**
 * What a card has, for a template's `requires`: the features it turned on, plus a second
 * face when it has one (the split, adventure and prepared layouts draw both at once).
 */
export const hasAll = (card) => (card.back ? [...card.features, "back"] : card.features);

/** A look: a frame of its own that any card can wear, so a set may default to it (the server's rule). */
export const isLook = (template) => !template.requires.length && !template.excludes.length && template.frame !== "inherit";

/**
 * How a card is drawn: the `layout` template that places its parts, and the `look` (frame) it
 * wears. A card names a template, or none. A template with a body of its own (a planeswalker, a
 * saga, a token) is a layout; a look is a frame of its own. A card with features only a layout
 * template can draw (loyalty) gets that layout whatever it names, and wears its own look or the
 * set's. A whole-card picture is drawn by the same layout as any card, over its picture.
 */
export function resolveTemplate(card, setDefault = "modern") {
  const list = allTemplates();
  const has = hasAll(card);
  const named = list.find((t) => t.id === card.template && worksWith(t, has));
  const defaulted = list.find((t) => t.id === setDefault) ?? list[0];
  const look = [named, defaulted].find((t) => t && FRAMES.has(t.id))?.id ?? "modern";
  const layout =
    (named?.body.length ? named : null) ??
    // Features only decide a layout for you (a card with loyalty is a planeswalker); a
    // layout that wants a second face is the author's to pick, never automatic.
    list.find((t) => t.requires.length && !t.requires.includes("back") && worksWith(t, has)) ??
    list.find((t) => t.id === look);
  return { layout, look: layout?.frame && layout.frame !== "inherit" ? layout.frame : look };
}
