import { html, render, useCallback, useEffect, useLayoutEffect, useRef, useState } from "/public/vendor/htm-preact.js";
import { api, uploadArt } from "/public/api.js";
import { artBackground, artUrl, prepareArt } from "/public/art.js";
import { ArtSearchDialog, Credit } from "/public/art-search.js";
import { CardFace, Mana } from "/public/card-face.js";
import { BulkBar, TransferDialog } from "/public/bulk.js";
import { TypeBuilder } from "/public/type-builder.js";
import { frameOf } from "/public/frame.js";
import { allTemplates, isLook, resolveTemplate } from "/public/templates.js";

const SAVE_DELAY_MS = 500;
const RARITIES = ["common", "uncommon", "rare", "mythic"];
// Left to right as they are printed on the cards.
const PALETTE = ["W", "U", "B", "R", "G", "C", "X", "0", "1", "2", "3", "4", "5", "6", "T"];
const FIELD_LABELS = { mana_cost: "mana cost", rules_text: "rules text" };
/** The fields a card with these features on has, base first. */
const fieldsFor = (features) => [...choices.base, ...choices.features.filter((f) => features.includes(f.id)).flatMap((f) => f.fields)];

/** Keep shared fields and normalize every face before sending the document. */
function cleanCard(card) {
  return {
    features: card.features,
    template: card.template ?? "",
    frame: card.frame ?? "",
    rarity: card.rarity ?? "common",
    art: card.art ?? null,
    art_position: card.art_position ?? null,
    art_zoom: card.art_zoom ?? null,
    art_is_card: card.art_is_card ?? false,
    keep_printed_text: card.keep_printed_text ?? false,
    back: card.back ? cleanCard(card.back) : null,
    half: card.half ? cleanCard(card.half) : null,
    ...Object.fromEntries(fieldsFor(card.features).map((f) => [f.key, card[f.key] ?? ""])),
  };
}

const blankFace = (rarity) => cleanCard({ rarity, features: choices.features.filter((f) => f.default).map((f) => f.id) });
const faceOf = (card, path) => path.reduce((face, key) => face?.[key], card);
const replaceFace = (card, [key, ...rest], face) => key ? { ...card, [key]: replaceFace(card[key], rest, face) } : face;
const sameSide = (a, b) => a.join("/") === b.join("/");

function fitTemplate(card, template) {
  const next = {
    ...card,
    template: template.id,
    features: choices.features.map(({ id }) => id).filter((id) =>
      template.requires.includes(id) || (card.features.includes(id) && !template.excludes.includes(id))),
  };
  for (const key of ["back", "half"]) {
    if (template.requires.includes(key)) next[key] ??= blankFace(card.rarity);
    if (template.excludes.includes(key)) next[key] = null;
  }
  return next;
}

function cardSides(card) {
  return [
    { path: [], label: "Front" },
    ...(card.half ? [{ path: ["half"], label: "2nd half" }] : []),
    ...(card.back ? [{ path: ["back"], label: "Back" }] : []),
    ...(card.back?.half ? [{ path: ["back", "half"], label: "Back 2nd half" }] : []),
  ];
}

function SideTabs({ card, side, onSide }) {
  const sides = cardSides(card);
  if (sides.length === 1) return null;
  return html`<div class="faces" role="group" aria-label="Faces">
    ${sides.map(({ path, label }) => html`<button type="button" class=${`face-tab${sameSide(side, path) ? " chosen" : ""}`}
      aria-pressed=${String(sameSide(side, path))} onClick=${() => onSide(path)}>${label}</button>`)}
  </div>`;
}

function TemplateSetup({ initial, initialSide, template, creating, onApply, onCancel }) {
  const [draft, setDraft] = useState(initial);
  const [side, setSide] = useState(initialSide);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const ref = useRef(null);
  useEffect(() => { ref.current.showModal(); }, []);
  const cancel = () => { ref.current.close(); onCancel(); };
  const card = faceOf(draft, side);
  const allowed = side.includes("half") ? [] : side.length ? ["half"] : ["back", "half"];
  const selected = allTemplates().find((t) => t.id === (card.template || template));
  const change = (next) => setDraft(replaceFace(draft, side, next));
  const lock = (id) => selected.requires.includes(id) ? "Required" : selected.excludes.includes(id) ? "Unavailable" : "";
  const toggle = (id, label, checked, update) => html`<label class="tint-option">
    <input type="checkbox" name=${`feature-${id}`} checked=${checked} disabled=${Boolean(lock(id))} onChange=${(e) => update(e.currentTarget.checked)} />
    ${label}${lock(id) ? html`<small>${lock(id)}</small>` : null}
  </label>`;
  return html`<dialog ref=${ref} class="confirm-dialog template-setup" aria-labelledby="template-title"
    onCancel=${(e) => { e.preventDefault(); if (!saving) cancel(); }}>
    <div>
      <h2 id="template-title">${creating ? "Set up your card" : "Edit card template"}</h2>
      <${SideTabs} card=${draft} side=${side} onSide=${setSide} />
      <fieldset class="looks">
        <legend>Template</legend>
        ${allTemplates().filter((t) => t.requires.every((id) => !["back", "half"].includes(id) || allowed.includes(id))).map((t) => {
          const next = fitTemplate(card, t);
          return html`<label class=${`look-option${selected === t ? " chosen" : ""}`}>
            <input type="radio" name="template" value=${t.id} checked=${selected === t} onChange=${() => change(next)} />
            <span class="look-thumb"><${CardFace} card=${{ ...cleanCard(next), art_is_card: false }} template=${template} /></span>
            <span class="look-name">${t.label}</span>
          </label>`;
        })}
      </fieldset>
      <fieldset class="tints features">
        <legend>Features</legend>
        ${choices.features.map(({ id, label }) => toggle(id, label, card.features.includes(id), (on) =>
          change({ ...card, features: on ? [...card.features, id] : card.features.filter((f) => f !== id) })))}
      </fieldset>
      ${allowed.length ? html`<fieldset class="tints features">
        <legend>Additional faces</legend>
        ${allowed.map((id) => toggle(id, id === "back" ? "Back face" : "Second half", Boolean(card[id]), (on) =>
          change({ ...card, [id]: on ? faceOf(initial, [...side, id]) ?? blankFace(card.rarity) : null })))}
      </fieldset>` : null}
      ${error ? html`<p class="error" role="alert">${error}</p>` : null}
      <footer>
        <button type="button" class="quiet" disabled=${saving} onClick=${cancel}>Cancel</button>
        <button type="button" class="apply-template" disabled=${saving} onClick=${async () => {
          setSaving(true);
          setError("");
          try {
            await onApply(cleanCard(draft));
            cancel();
          } catch (e) {
            setError(e.message);
            setSaving(false);
          }
        }}>${creating ? "Start editing" : "Apply changes"}</button>
      </footer>
    </div>
  </dialog>`;
}

const app = document.getElementById("app");
const setId = app.dataset.setId;
// The server says how long each field may be, so the form never keeps its own copy.
const limits = JSON.parse(document.getElementById("limits").textContent);
// So does the list of looks and frame colours a card may name.
const choices = JSON.parse(document.getElementById("choices").textContent);

function Field({ label, count, children }) {
  return html`<label>
    <span class="field-label">${label}${count === undefined ? null : html`<small class="count">${count}</small>`}</span>
    ${children}
  </label>`;
}

function Confirm({ open, title, body, action, onConfirm, onCancel }) {
  const ref = useRef(null);
  useEffect(() => {
    const dialog = ref.current;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);
  return html`<dialog ref=${ref} class="confirm-dialog" onCancel=${onCancel}>
    <div>
      <h2>${title}</h2>
      <p>${body}</p>
      <footer>
        <button type="button" class="quiet confirm-cancel" onClick=${onCancel}>Keep it</button>
        <button type="button" class="danger confirm-accept" onClick=${onConfirm}>${action}</button>
      </footer>
    </div>
  </dialog>`;
}

/** Mana and tap symbols, inserted where the cursor was in the cost or the rules. */
function Palette({ target, onInsert }) {
  return html`<div class="palette" role="group" aria-label=${`Insert a symbol into the ${FIELD_LABELS[target]}`}>
    <p class="palette-hint">Symbols → <strong>${FIELD_LABELS[target]}</strong></p>
    <div class="palette-keys">
      ${PALETTE.map((symbol) => html`<button
        type="button"
        class="symbol-key"
        data-symbol=${symbol}
        aria-label=${symbol === "T" ? "Tap" : symbol}
        disabled=${symbol === "T" && target === "mana_cost"}
        onMouseDown=${(e) => e.preventDefault()}
        onClick=${() => onInsert(symbol)}
      ><${Mana} symbol=${symbol} /></button>`)}
    </div>
  </div>`;
}

function Editor({ card, template, credit, onChange, onSetDefault, onCredit, artOpen, onCloseArt, artPart }) {
  const fields = fieldsFor(card.features);
  const has = (key) => fields.some((f) => f.key === key);
  const [wanted, setTarget] = useState("mana_cost");
  // Symbols go where the cursor last was; a type with no mana cost has nowhere else but the text.
  const target = has(wanted) ? wanted : "rules_text";
  const missing = fields.filter((f) => f.required && !String(card[f.key] ?? "").trim()).map((f) => f.label.toLowerCase());
  // Only a look can be a set's default (the server checks the same rule), so the
  // button that makes this card's template the set's is off for a layout.
  const chosen = card.template ? allTemplates().find((t) => t.id === card.template) : null;
  const mayDefault = Boolean(chosen && isLook(chosen) && card.template !== template);
  const artDialog = useRef(null);
  useEffect(() => {
    if (artOpen) artDialog.current.showModal();
    else artDialog.current.close();
  }, [artOpen]);
  const closeArt = () => {
    artDialog.current.close();
    onCloseArt();
  };
  const [searching, setSearching] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [artError, setArtError] = useState("");
  // An upload outlives the render that started it; apply it to the card as it
  // is when it lands, not as it was, so typing in the meantime is not lost.
  const latest = useRef(card);
  latest.current = card;
  const inputs = useRef({});
  // Where the caret goes once the next render has put the new value in the field.
  const caret = useRef(null);
  useLayoutEffect(() => {
    if (!caret.current) return;
    const { key, position } = caret.current;
    caret.current = null;
    inputs.current[key].focus();
    inputs.current[key].setSelectionRange(position, position);
  });
  const text = (key) => ({
    name: key,
    value: card[key],
    maxLength: limits[key],
    ref: (el) => { inputs.current[key] = el; },
    onInput: (e) => onChange({ ...card, [key]: e.currentTarget.value }),
  });
  const watch = (key) => ({ onFocus: () => setTarget(key) });

  function insert(symbol) {
    const input = inputs.current[target];
    const token = `{${symbol}}`;
    const start = input.selectionStart ?? input.value.length;
    const end = input.selectionEnd ?? start;
    const value = input.value.slice(0, start) + token + input.value.slice(end);
    if (value.length > limits[target]) return;
    // The controlled value resets the caret, so put it back after the token.
    caret.current = { key: target, position: start + token.length };
    onChange({ ...card, [target]: value });
  }

  /** Art found on Openverse: keep it, and fill the artist from its credit if that is empty. */
  function pickedArt({ id, credit: found }) {
    setSearching(false);
    onCredit(id, found);
    const artist = latest.current.artist || found.creator.slice(0, limits.artist);
    onChange({ ...latest.current, art: id, art_position: null, art_zoom: null, artist });
  }

  async function chooseArt(event) {
    const input = event.currentTarget;
    const file = input.files[0];
    input.value = "";
    if (!file) return;
    setUploading(true);
    setArtError("");
    try {
      const { id } = await uploadArt(await prepareArt(file));
      onChange({ ...latest.current, art: id, art_position: null, art_zoom: null });
    } catch (e) {
      setArtError(e.message);
    } finally {
      setUploading(false);
    }
  }

  const crop = !card.art_is_card && artPart;
  const zoom = card.art_zoom ?? 100;
  const [artX, artY] = card.art_position ?? [50, 50];

  return html`<form class="panel card-form" onSubmit=${(e) => e.preventDefault()}>
    <fieldset class="form-section">
      <legend>Identity</legend>
      <p class=${`muted missing${missing.length ? "" : " done"}`} role="status">${missing.length ? `To finish this card: ${missing.join(", ")}.` : "Every required field is filled in."}</p>
      <${Field} label="Name"><input autocomplete="off" ...${text("name")} /><//>
      ${has("mana_cost") ? html`<${Field} label="Mana cost"><input placeholder="{2}{U}{U}" autocomplete="off" ...${text("mana_cost")} ...${watch("mana_cost")} /><//>` : null}
      <${Field} label="Type line"><input placeholder="Creature — Merfolk Wizard" autocomplete="off" ...${text("type_line")} /><//>
      <${TypeBuilder} typeLine=${card.type_line} onChange=${(type_line) => onChange({ ...latest.current, type_line })} />
      <fieldset class="rarity">
        <legend>Rarity</legend>
        ${RARITIES.map((rarity) => html`<label class=${`rarity-option rarity-${rarity}`}>
          <input type="radio" name="rarity" value=${rarity} checked=${card.rarity === rarity} onChange=${() => onChange({ ...card, rarity })} />
          <i class="rarity-pip"></i>${rarity}
        </label>`)}
      </fieldset>
    </fieldset>

    <fieldset class="form-section frame-section">
      <legend>Frame</legend>
      <fieldset class="tints">
        <legend>Colour</legend>
        ${[["", "Auto"], ...choices.frames].map(([id, label]) => html`<label class=${`tint-option tint-${id || "auto"}`}>
          <input type="radio" name="frame" value=${id} checked=${card.frame === id} onChange=${() => onChange({ ...card, frame: id })} />
          <i class=${`tint-chip mtg-card frame-${id || frameOf({ ...card, frame: "" })}`}></i>${label}${id === "" ? html` <small>(${choices.frames.find(([f]) => f === frameOf({ ...card, frame: "" }))?.[1]})</small>` : null}
        </label>`)}
      </fieldset>
      <button type="button" class="quiet set-default-template" disabled=${!mayDefault} onClick=${() => onSetDefault(card.template)}>Use this look for every card in the set</button>
    </fieldset>

    <fieldset class="form-section">
      <legend>Text</legend>
      ${card.features.includes("loyalty")
        ? html`<p class="muted layout-hint">One ability per line: +1: Draw a card. A line with no cost, or starting with a colon, is a static ability.</p>`
        : card.template === "saga"
        ? html`<p class="muted layout-hint">Reminder text first, then one line per chapter: I: Draw a card. II, III: Gain 2 life.</p>`
        : null}

      <${Field} label="Rules text" count=${`${card.rules_text.length}/${limits.rules_text}`}>
        <textarea rows="5" placeholder="Flying\n{T}: Draw a card." ...${text("rules_text")} ...${watch("rules_text")}></textarea>
      <//>
      <${Palette} target=${target} onInsert=${insert} />
      ${has("flavor_text")
        ? html`<${Field} label="Flavor text" count=${`${card.flavor_text.length}/${limits.flavor_text}`}>
            <textarea rows="2" ...${text("flavor_text")}></textarea>
          <//>`
        : null}
    </fieldset>

    <fieldset class="form-section">
      <legend>Details</legend>
      <div class="row">
        ${has("loyalty") ? html`<${Field} label="Loyalty"><input placeholder="4" autocomplete="off" ...${text("loyalty")} /><//>` : null}
        ${has("power_toughness") ? html`<${Field} label="Power / toughness"><input placeholder="2/3" autocomplete="off" ...${text("power_toughness")} /><//>` : null}
        ${has("defense") ? html`<${Field} label="Defense"><input placeholder="5" autocomplete="off" ...${text("defense")} /><//>` : null}
        ${has("artist") ? html`<${Field} label="Artist"><input autocomplete="off" ...${text("artist")} /><//>` : null}
      </div>
    </fieldset>
  </form>
    <dialog ref=${artDialog} class="confirm-dialog art-dialog" aria-labelledby="art-title"
      onCancel=${(e) => { e.preventDefault(); if (!uploading) closeArt(); }}>
      <div>
        <h2 id="art-title">Card art</h2>
        <div class="art-row">
          <div class=${`art-thumb${card.art && crop && zoom > 100 ? " art-zoomed" : ""}`}
            style=${card.art && (crop
              ? `${artBackground(card)};aspect-ratio:${crop.w}/${crop.h}`
              : `background-image:url(${artUrl(card.art)})`)}></div>
          <div class="art-actions">
            <label class="file-button" aria-busy=${String(uploading)}>
              ${uploading ? "Uploading…" : card.art ? "Replace image" : "Upload image"}
              <input type="file" name="art" accept="image/png,image/jpeg,image/webp" disabled=${uploading} onChange=${chooseArt} />
            </label>
            <button type="button" class="quiet search-art" disabled=${uploading} onClick=${() => setSearching(true)}>Find free art…</button>
            <button type="button" class="quiet remove-art" disabled=${!card.art || uploading} onClick=${() => onChange({ ...card, art: null, art_position: null, art_zoom: null, art_is_card: false, keep_printed_text: false })}>Remove</button>
            <p class="art-hint muted">PNG, JPEG or WebP. It is shrunk to fit before it is sent.</p>
          </div>
        </div>
        ${card.art && crop ? html`<fieldset class="form-section art-crop">
          <legend>Crop</legend>
          <${Field} label="Zoom" count=${`${zoom}%`}>
            <input type="range" min="100" max=${choices.art_zoom_max} value=${zoom} disabled=${uploading}
              aria-label="Zoom" aria-valuetext=${`${zoom}%`}
              onInput=${(e) => onChange({ ...card, art_zoom: e.currentTarget.valueAsNumber })} />
          <//>
          <${Field} label="Horizontal">
            <input type="range" min="0" max="100" value=${artX} disabled=${uploading}
              onInput=${(e) => onChange({ ...card, art_position: [e.currentTarget.valueAsNumber, artY] })} />
          <//>
          <${Field} label="Vertical">
            <input type="range" min="0" max="100" value=${artY} disabled=${uploading}
              onInput=${(e) => onChange({ ...card, art_position: [artX, e.currentTarget.valueAsNumber] })} />
          <//>
          <button type="button" class="quiet" disabled=${uploading || (artX === 50 && artY === 50 && zoom === 100)}
            onClick=${() => onChange({ ...card, art_position: null, art_zoom: null })}>Reset crop</button>
        </fieldset>` : null}
        ${card.art
          ? html`<label class="art-is-card">
              <input type="checkbox" name="art_is_card" checked=${card.art_is_card} onChange=${(e) => onChange({ ...card, art_is_card: e.currentTarget.checked })} />
              <span>This picture is a whole card, not just art. It fills the card, with the text laid over it.</span>
            </label>
            ${card.art_is_card
              ? html`<label class="art-is-card keep-printed">
                  <input type="checkbox" name="keep_printed_text" checked=${card.keep_printed_text} onChange=${(e) => onChange({ ...card, keep_printed_text: e.currentTarget.checked })} />
                  <span>Keep the words printed on the picture. Draw none of ours over it.</span>
                </label>`
              : null}`
          : null}
        ${card.art && credit ? html`<p class="art-credit-line"><${Credit} credit=${credit} /></p>` : null}
        <${ArtSearchDialog} open=${searching} onPick=${pickedArt} onCancel=${() => setSearching(false)} />
        <p class="error art-error" role="alert">${artError}</p>
        <footer><button type="button" disabled=${uploading} onClick=${closeArt}>Done</button></footer>
      </div>
    </dialog>
  `;
}

function SetPage() {
  const [detail, setDetail] = useState(null);
  const [selected, setSelected] = useState(null);
  // Which face of a double-faced card is being edited.
  const [side, setSide] = useState([]);
  const [setup, setSetup] = useState(null);
  const [artOpen, setArtOpen] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const [confirming, setConfirming] = useState(null);
  // Select mode: tapping a card picks it instead of opening it, and the bulk
  // bar takes the editor's place.
  const [bulk, setBulk] = useState(false);
  const [picked, setPicked] = useState(() => new Set());
  const [transfer, setTransfer] = useState(null);
  const [notice, setNotice] = useState(null);
  // Who made each piece of borrowed art, by art id.
  const [credits, setCredits] = useState({});
  // Edits are held here until the pause after the last keystroke, so a fast
  // typist makes one request, not one per letter.
  const pending = useRef(new Map());
  const timer = useRef(0);

  useEffect(() => {
    api("GET", `/api/sets/${setId}`).then((d) => {
      setDetail(d);
      setCredits(d.credits ?? {});
      setSelected(d.cards[0]?.id ?? null);
    }, (e) => setError(e.message));
  }, []);

  const flush = useCallback(async () => {
    clearTimeout(timer.current);
    const edits = [...pending.current];
    pending.current.clear();
    if (edits.length === 0) return;
    setStatus("Saving…");
    try {
      await Promise.all(edits.map(([id, card]) => api("PUT", `/api/cards/${id}`, card)));
      setStatus("Saved");
      setError("");
    } catch (e) {
      setStatus("");
      setError(e.message);
    }
  }, []);

  useEffect(() => {
    const leaving = () => {
      for (const [id, card] of pending.current) {
        fetch(`/api/cards/${id}`, { method: "PUT", keepalive: true, headers: { "content-type": "application/json" }, body: JSON.stringify(card) });
      }
    };
    addEventListener("pagehide", leaving);
    return () => removeEventListener("pagehide", leaving);
  }, []);

  // Keep the card being edited in view as the strip grows past the screen.
  useEffect(() => {
    document.querySelector(".filmstrip .mtg-card.selected")?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [selected]);

  // Choosing another card starts on its front.
  useEffect(() => { setSide([]); setArtOpen(false); }, [selected]);

  function edit(id, card) {
    setDetail((d) => ({ ...d, cards: d.cards.map((c) => (c.id === id ? { ...c, card } : c)) }));
    pending.current.set(id, card);
    setStatus("Unsaved changes");
    clearTimeout(timer.current);
    timer.current = setTimeout(flush, SAVE_DELAY_MS);
  }

  useEffect(() => {
    if (!bulk) return undefined;
    const leave = (event) => {
      // Escape closes a dialog first; only a bare one ends select mode.
      if (event.key === "Escape" && !document.querySelector("dialog[open]")) endBulk();
    };
    addEventListener("keydown", leave);
    return () => removeEventListener("keydown", leave);
  }, [bulk]);

  async function startBulk() {
    await flush();
    setNotice(null);
    setPicked(new Set());
    setBulk(true);
  }

  function endBulk() {
    setBulk(false);
    setPicked(new Set());
  }

  function togglePicked(id) {
    setPicked((now) => {
      const next = new Set(now);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }

  /** Back from the server's idea of the set, after cards were moved or deleted. */
  async function reload() {
    const fresh = await api("GET", `/api/sets/${setId}`);
    setDetail(fresh);
    setCredits(fresh.credits ?? {});
    setSelected((was) => fresh.cards.find((c) => c.id === was)?.id ?? fresh.cards[0]?.id ?? null);
  }

  async function bulkDelete() {
    setConfirming(null);
    const ids = [...picked];
    try {
      await api("POST", "/api/cards/delete", { ids });
      await reload();
      endBulk();
      setNotice({ text: `Deleted ${ids.length} ${ids.length === 1 ? "card" : "cards"}.` });
      setError("");
    } catch (e) {
      setError(e.message);
    }
  }

  async function bulkTransfer(to) {
    const ids = [...picked];
    const copy = transfer === "copy";
    const done = await api("POST", "/api/cards/transfer", { ids, to, copy });
    setTransfer(null);
    await reload();
    endBulk();
    setNotice({ text: `${copy ? "Copied" : "Moved"} ${done.count} ${done.count === 1 ? "card" : "cards"} to`, link: { href: `/sets/${done.set.id}`, label: done.set.name } });
    setError("");
  }

  async function addCard(card) {
    await flush();
    const stored = await api("POST", `/api/sets/${setId}/cards`, card);
    setDetail((d) => ({ ...d, set: { ...d.set, card_count: d.set.card_count + 1 }, cards: [...d.cards, stored] }));
    setSelected(stored.id);
    setError("");
  }

  async function deleteCard(id) {
    setConfirming(null);
    await flush();
    try {
      await api("DELETE", `/api/cards/${id}`);
      const d = await api("GET", `/api/sets/${setId}`);
      setDetail(d);
      setSelected(d.cards.find((c) => c.id === selected)?.id ?? d.cards[0]?.id ?? null);
    } catch (e) {
      setError(e.message);
    }
  }

  async function deleteSet() {
    setConfirming(null);
    try {
      await api("DELETE", `/api/sets/${setId}`);
      location.href = "/";
    } catch (e) {
      setError(e.message);
    }
  }

  async function patchSet(patch) {
    try {
      const set = await api("PATCH", `/api/sets/${setId}`, patch);
      setDetail((d) => ({ ...d, set }));
      setError("");
    } catch (e) {
      setError(e.message);
    }
  }

  if (!detail) return html`<p class="error" role="alert">${error}</p>`;
  const { set, cards } = detail;
  const current = cards.find((c) => c.id === selected);
  // Which face of the card is being edited: its own, a back face (one you turn to), or a
  // second half (a whole card drawn beside it). A back is not a half.
  const editing = current ? faceOf(current.card, side) ?? current.card : undefined;
  const turned = side[0] === "back" && Boolean(current?.card.back);
  const visibleSide = turned ? ["back"] : [];
  const visibleFace = current && faceOf(current.card, visibleSide);
  const layout = visibleFace && resolveTemplate(
    visibleFace.template ? visibleFace : { ...visibleFace, template: current.card.template }, set.default_template,
  ).layout;
  const artParts = layout?.body.filter((part) => part.tag === "art") ?? [];
  const artPart = artParts.find((part) => sameSide(side, part.face === "half" ? [...visibleSide, "half"] : visibleSide));
  const scan = visibleFace?.art && visibleFace.art_is_card;
  const hasArtControl = scan ? sameSide(side, visibleSide) : Boolean(artPart);
  function openArt(path) {
    setSide(path);
    setArtOpen(true);
  }


  return html`
    <section class="set-bar">
      <div class="set-title">
        <h1>${set.name}</h1>
        <p class="muted set-meta"><span class="set-code">${set.code}</span> · ${cards.length} ${cards.length === 1 ? "card" : "cards"}${set.public ? html` · <span class="public-badge">Public</span>` : null}</p>
      </div>
      <div class="set-tools">
        <p class="save-state" role="status">${status}</p>
        ${cards.length
          ? html`<button type="button" class="quiet select-cards" aria-pressed=${String(bulk)} onClick=${bulk ? endBulk : startBulk}>${bulk ? "Done" : "Select"}</button>`
          : null}
      </div>
      <details class="set-settings">
        <summary>Set settings</summary>
        <div class="row">
          <label>Name<input name="set-name" defaultValue=${set.name} maxLength="64" onChange=${(e) => patchSet({ name: e.currentTarget.value })} /></label>
          <label>Code<input name="set-code" defaultValue=${set.code} maxLength="5" onChange=${(e) => patchSet({ code: e.currentTarget.value })} /></label>
          <button type="button" class="danger delete-set" onClick=${() => setConfirming("set")}>Delete set</button>
        </div>
        <label class="default-look">Default look
          <select name="default-template" value=${set.default_template} onChange=${(e) => patchSet({ default_template: e.currentTarget.value })}>
            ${allTemplates().filter(isLook).map(({ id, label }) => html`<option value=${id} selected=${id === set.default_template}>${label}</option>`)}
          </select>
        </label>
        <label class="public-toggle">
          <input type="checkbox" name="public" checked=${set.public} onChange=${(e) => patchSet({ public: e.currentTarget.checked })} />
          <span>Show this set in the public gallery</span>
        </label>
        <p class="muted public-help">${set.public
          ? html`Anyone can see this set, its cards and their art, with your display name on it. <a class="public-link" href=${`/gallery/${set.id}`}>View it as they do</a>.`
          : "Anyone could then see this set, its cards and their art, with your display name on it. You can take it down again at any time."}</p>
      </details>
    </section>
    <p class="error" role="alert">${error}</p>
    ${notice
      ? html`<p class="notice" role="status">${notice.text}${notice.link ? html` <a href=${notice.link.href}>${notice.link.label}</a>.` : null}</p>`
      : null}
    ${bulk
      ? html`<${BulkBar}
          count=${picked.size}
          total=${cards.length}
          onAll=${() => setPicked(new Set(cards.map((c) => c.id)))}
          onNone=${() => setPicked(new Set())}
          onMove=${() => setTransfer("move")}
          onCopy=${() => setTransfer("copy")}
          onDelete=${() => setConfirming("bulk")}
        />`
      : null}

    <section class="filmstrip" aria-label="Cards in this set">
      ${cards.map((c) => html`<div class=${`strip-item${bulk && picked.has(c.id) ? " picked" : ""}`} key=${c.id}>
        <${CardFace}
          card=${c.card}
          template=${set.default_template}
          code=${set.code}
          number=${c.position}
          total=${cards.length}
          selected=${bulk ? picked.has(c.id) : c.id === selected}
          onClick=${bulk ? () => togglePicked(c.id) : async () => { await flush(); setSelected(c.id); }}
        />
      </div>`)}
      ${bulk ? null : html`<button type="button" class="add-card" onClick=${() => setSetup({ initial: blankFace(), initialSide: [] })}><span class="plus">+</span>Add card</button>`}
    </section>

    ${bulk
      ? null
      : current
      ? html`<section class="creator">
          <div class="stage">
            <div class="preview"><${CardFace} card=${current.card} flipped=${turned} template=${set.default_template}
              code=${set.code} number=${current.position} total=${cards.length}
              onArt=${(half) => openArt(half ? [...visibleSide, "half"] : visibleSide)} /></div>
            <div class="stage-actions">
              ${current.card.back
                ? html`<button type="button" class="quiet flip-card" onClick=${() => setSide(turned ? [] : ["back"])}>Flip</button>`
                : null}
              <button type="button" class="quiet duplicate-card" onClick=${() => addCard(current.card).catch((e) => setError(e.message))}>Duplicate</button>
              <button type="button" class="quiet danger-quiet delete-card" onClick=${() => setConfirming("card")}>Delete</button>
            </div>
          </div>
          <div class="editing">
            <div class="faces">
              <${SideTabs} card=${current.card} side=${side} onSide=${setSide} />
              <button type="button" class="quiet edit-template" onClick=${() => setSetup({ initial: current.card, initialSide: side, id: current.id })}>Edit card template</button>
              ${hasArtControl ? null : html`<button type="button" class="quiet" onClick=${() => openArt(side)}>Edit art</button>`}
            </div>
            <${Editor}
              key=${`${current.id}-${side}`}
              card=${editing}
              template=${set.default_template}
              credit=${credits[editing?.art]}
              onChange=${(card) => edit(current.id, replaceFace(current.card, side, card))}
              artOpen=${artOpen}
              artPart=${scan ? undefined : artPart}
              onCloseArt=${() => setArtOpen(false)}
              onSetDefault=${(default_template) => patchSet({ default_template })}
              onCredit=${(id, credit) => setCredits((now) => ({ ...now, [id]: credit }))}
            />
          </div>
        </section>`
      : html`<p class="muted empty">No cards yet. Add the first one.</p>`}

    ${setup ? html`<${TemplateSetup} ...${setup} template=${set.default_template} creating=${!setup.id}
      onCancel=${() => setSetup(null)} onApply=${async (card) => {
        if (!setup.id) return addCard(card);
        if (!faceOf(card, side)) setSide([]);
        edit(setup.id, card);
      }} />` : null}
    <${Confirm}
      open=${confirming === "bulk"}
      title=${`Delete ${picked.size} ${picked.size === 1 ? "card" : "cards"}?`}
      body="They will be removed and the cards after them renumbered. This cannot be undone."
      action=${`Delete ${picked.size} ${picked.size === 1 ? "card" : "cards"}`}
      onConfirm=${bulkDelete}
      onCancel=${() => setConfirming(null)}
    />
    <${TransferDialog}
      open=${transfer !== null}
      mode=${transfer ?? "move"}
      count=${picked.size}
      currentSetId=${setId}
      onSubmit=${bulkTransfer}
      onCancel=${() => setTransfer(null)}
    />
    <${Confirm}
      open=${confirming === "card"}
      title="Delete this card?"
      body=${`"${current?.card.name || "Untitled card"}" will be removed and the cards after it renumbered.`}
      action="Delete card"
      onConfirm=${() => deleteCard(current.id)}
      onCancel=${() => setConfirming(null)}
    />
    <${Confirm}
      open=${confirming === "set"}
      title="Delete this set?"
      body=${`"${set.name}" and all ${cards.length} of its cards will be removed. This cannot be undone.`}
      action="Delete set"
      onConfirm=${deleteSet}
      onCancel=${() => setConfirming(null)}
    />
  `;
}

render(html`<${SetPage} />`, app);
