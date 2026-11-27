// The built-in templates (see CARDTYPES.md): restricted XML the server embeds in every page that
// draws a card. This reads them, and decides which one draws a given card. Pure apart from reading
// the page's own JSON, so the editor, the gallery and the dev frames page agree.
import { worksWith } from "/public/frame.js";

const FRAMES = new Set(["modern", "classic", "borderless", "full-art"]);

let parsed = null;

/** The elements of a template's body: `{tag, role, x, y, w, h, fields: [{name, as}]}`. */
function body(xml) {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  return [...doc.documentElement.children].map((el) => {
    const part = { tag: el.tagName, role: el.getAttribute("role") };
    for (const side of ["x", "y", "w", "h"]) part[side] = Number(el.getAttribute(side));
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

/** A look: a frame of its own that any card can wear, so a set may default to it (the server's rule). */
export const isLook = (template) => !template.requires.length && !template.excludes.length && template.frame !== "inherit";

/**
 * How a card is drawn: the `layout` template that places its parts, and the `look` (frame) it
 * wears. A card names a template, or none. A template with a body of its own (a planeswalker, a
 * saga, a token) is a layout; a frame is a look. A card with features only a layout template
 * can draw (loyalty) gets that layout whatever it names, and wears its own look or the set's. A
 * card whose picture is a whole card (`scan`) with no layout of its own is laid out as Nexus does.
 */
export function resolveTemplate(card, setDefault = "modern", scan = false) {
  const list = allTemplates();
  const named = list.find((t) => t.id === card.template && worksWith(t, card.features));
  const defaulted = list.find((t) => t.id === setDefault) ?? list[0];
  const look = [named, defaulted].find((t) => t && FRAMES.has(t.id))?.id ?? "modern";
  const layout =
    (named?.body.length ? named : null) ??
    list.find((t) => t.requires.length && worksWith(t, card.features)) ??
    // A whole-card picture is a Nexus card, and so is laid out the way Nexus lays one out.
    (scan ? list.find((t) => t.id === "nexus") : null) ??
    list.find((t) => t.id === look);
  return { layout, look: layout?.frame && layout.frame !== "inherit" ? layout.frame : look };
}
