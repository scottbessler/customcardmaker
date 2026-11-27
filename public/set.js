import { html, render, useCallback, useEffect, useLayoutEffect, useRef, useState } from "/public/vendor/htm-preact.js";
import { api, uploadArt } from "/public/api.js";
import { artUrl, prepareArt } from "/public/art.js";
import { ArtSearchDialog, Credit } from "/public/art-search.js";
import { CardFace, Mana } from "/public/card-face.js";
import { BulkBar, TransferDialog } from "/public/bulk.js";
import { TypeBuilder } from "/public/type-builder.js";
import { frameOf } from "/public/frame.js";

const SAVE_DELAY_MS = 500;
const RARITIES = ["common", "uncommon", "rare", "mythic"];
// Left to right as they are printed on the cards.
const PALETTE = ["W", "U", "B", "R", "G", "C", "X", "0", "1", "2", "3", "4", "5", "6", "T"];
const FIELD_LABELS = { mana_cost: "mana cost", rules_text: "rules text" };
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

function Editor({ card, template, credit, onChange, onSetDefault, onCredit }) {
  const [target, setTarget] = useState("mana_cost");
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
    onChange({ ...latest.current, art: id, artist });
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
      onChange({ ...latest.current, art: id });
    } catch (e) {
      setArtError(e.message);
    } finally {
      setUploading(false);
    }
  }

  return html`<form class="panel card-form" onSubmit=${(e) => e.preventDefault()}>
    <fieldset class="form-section">
      <legend>Identity</legend>
      <${Field} label="Name"><input autocomplete="off" ...${text("name")} /><//>
      <${Field} label="Mana cost"><input placeholder="{2}{U}{U}" autocomplete="off" ...${text("mana_cost")} ...${watch("mana_cost")} /><//>
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
      ${card.art && card.art_is_card
        ? html`<p class="muted looks-note">A whole-card picture is drawn as it is, so it has no template. The colour below tints our text plates over it.</p>`
        : html`<fieldset class="looks">
        <legend>Template</legend>
        ${choices.templates.map(([id, label]) => html`<label class=${`look-option${(card.template || template) === id ? " chosen" : ""}`}>
          <input type="radio" name="template" value=${id} checked=${(card.template || template) === id} onChange=${() => onChange({ ...card, template: id === template ? "" : id })} />
          <span class="look-thumb"><${CardFace} card=${{ ...card, template: id }} /></span>
          <span class="look-name">${label}${id === template ? html` <small>set default</small>` : null}</span>
        </label>`)}
      </fieldset>`}
      <fieldset class="tints">
        <legend>Colour</legend>
        ${[["", "Auto"], ...choices.frames].map(([id, label]) => html`<label class=${`tint-option tint-${id || "auto"}`}>
          <input type="radio" name="frame" value=${id} checked=${card.frame === id} onChange=${() => onChange({ ...card, frame: id })} />
          <i class=${`tint-chip mtg-card frame-${id || frameOf({ ...card, frame: "" })}`}></i>${label}${id === "" ? html` <small>(${choices.frames.find(([f]) => f === frameOf({ ...card, frame: "" }))?.[1]})</small>` : null}
        </label>`)}
      </fieldset>
      <button type="button" class="quiet set-default-template" disabled=${!card.template || card.template === template} onClick=${() => onSetDefault(card.template)}>Use this look for every card in the set</button>
    </fieldset>

    <fieldset class="form-section">
      <legend>Text</legend>
      <${Field} label="Rules text" count=${`${card.rules_text.length}/${limits.rules_text}`}>
        <textarea rows="5" placeholder="Flying\n{T}: Draw a card." ...${text("rules_text")} ...${watch("rules_text")}></textarea>
      <//>
      <${Palette} target=${target} onInsert=${insert} />
      <${Field} label="Flavor text" count=${`${card.flavor_text.length}/${limits.flavor_text}`}>
        <textarea rows="2" ...${text("flavor_text")}></textarea>
      <//>
    </fieldset>

    <fieldset class="form-section">
      <legend>Art</legend>
      <div class="art-row">
        <div class="art-thumb" style=${card.art ? `background-image:url(${artUrl(card.art)})` : undefined}></div>
        <div class="art-actions">
          <label class="file-button" aria-busy=${String(uploading)}>
            ${uploading ? "Uploading…" : card.art ? "Replace image" : "Upload image"}
            <input type="file" name="art" accept="image/png,image/jpeg,image/webp" disabled=${uploading} onChange=${chooseArt} />
          </label>
          <button type="button" class="quiet search-art" onClick=${() => setSearching(true)}>Find free art…</button>
          <button type="button" class="quiet remove-art" disabled=${!card.art} onClick=${() => onChange({ ...card, art: null, art_is_card: false })}>Remove</button>
          <p class="art-hint muted">PNG, JPEG or WebP. It is shrunk to fit before it is sent.</p>
        </div>
      </div>
      ${card.art
        ? html`<label class="art-is-card">
            <input type="checkbox" name="art_is_card" checked=${card.art_is_card} onChange=${(e) => onChange({ ...card, art_is_card: e.currentTarget.checked })} />
            <span>This picture is a whole card, not just art. It fills the card, with the text laid over it.</span>
          </label>`
        : null}
      ${card.art && credit ? html`<p class="art-credit-line"><${Credit} credit=${credit} /></p>` : null}
      <${ArtSearchDialog} open=${searching} onPick=${pickedArt} onCancel=${() => setSearching(false)} />
      <p class="error art-error" role="alert">${artError}</p>
    </fieldset>

    <fieldset class="form-section">
      <legend>Details</legend>
      <div class="row">
        <${Field} label="Power / toughness"><input placeholder="2/3" autocomplete="off" ...${text("power_toughness")} /><//>
        <${Field} label="Artist"><input autocomplete="off" ...${text("artist")} /><//>
      </div>
    </fieldset>
  </form>`;
}

function SetPage() {
  const [detail, setDetail] = useState(null);
  const [selected, setSelected] = useState(null);
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

  async function addCard(card = {}) {
    await flush();
    try {
      const stored = await api("POST", `/api/sets/${setId}/cards`, card);
      setDetail((d) => ({ ...d, set: { ...d.set, card_count: d.set.card_count + 1 }, cards: [...d.cards, stored] }));
      setSelected(stored.id);
      setError("");
    } catch (e) {
      setError(e.message);
    }
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
  const face = (c) => html`<${CardFace} card=${c.card} template=${set.default_template} code=${set.code} number=${c.position} total=${cards.length} />`;

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
            ${choices.templates.map(([id, label]) => html`<option value=${id} selected=${id === set.default_template}>${label}</option>`)}
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
      ${bulk ? null : html`<button type="button" class="add-card" onClick=${() => addCard()}><span class="plus">+</span>Add card</button>`}
    </section>

    ${bulk
      ? null
      : current
      ? html`<section class="creator">
          <div class="stage">
            <div class="preview">${face(current)}</div>
            <div class="stage-actions">
              <button type="button" class="quiet duplicate-card" onClick=${() => addCard(current.card)}>Duplicate</button>
              <button type="button" class="quiet danger-quiet delete-card" onClick=${() => setConfirming("card")}>Delete</button>
            </div>
          </div>
          <${Editor}
            key=${current.id}
            card=${current.card}
            template=${set.default_template}
            credit=${credits[current.card.art]}
            onChange=${(card) => edit(current.id, card)}
            onSetDefault=${(default_template) => patchSet({ default_template })}
            onCredit=${(id, credit) => setCredits((now) => ({ ...now, [id]: credit }))}
          />
        </section>`
      : html`<p class="muted empty">No cards yet. Add the first one.</p>`}

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
