import { html, render, useCallback, useEffect, useLayoutEffect, useRef, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";
import { CardFace, Mana } from "/public/card-face.js";
import { MAIN_TYPES, mainTypeOf, withMainType } from "/public/frame.js";

const SAVE_DELAY_MS = 500;
const RARITIES = ["common", "uncommon", "rare", "mythic"];
// Left to right as they are printed on the cards.
const PALETTE = ["W", "U", "B", "R", "G", "C", "X", "0", "1", "2", "3", "4", "5", "6", "T"];
const FIELD_LABELS = { mana_cost: "mana cost", rules_text: "rules text" };
const app = document.getElementById("app");
const setId = app.dataset.setId;
// The server says how long each field may be, so the form never keeps its own copy.
const limits = JSON.parse(document.getElementById("limits").textContent);

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

function Editor({ card, onChange }) {
  const [target, setTarget] = useState("mana_cost");
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

  const mainType = mainTypeOf(card.type_line);
  return html`<form class="panel card-form" onSubmit=${(e) => e.preventDefault()}>
    <fieldset class="form-section">
      <legend>Identity</legend>
      <${Field} label="Name"><input autocomplete="off" ...${text("name")} /><//>
      <${Field} label="Mana cost"><input placeholder="{2}{U}{U}" autocomplete="off" ...${text("mana_cost")} ...${watch("mana_cost")} /><//>
      <${Field} label="Type line"><input placeholder="Creature — Merfolk Wizard" autocomplete="off" ...${text("type_line")} /><//>
      <div class="type-picks" role="group" aria-label="Card type">
        ${MAIN_TYPES.map((type) => html`<button
          type="button"
          class="chip type-pick"
          aria-pressed=${String(mainType.includes(type))}
          onClick=${() => onChange({ ...card, type_line: withMainType(card.type_line, type) })}
        >${type}</button>`)}
      </div>
      <fieldset class="rarity">
        <legend>Rarity</legend>
        ${RARITIES.map((rarity) => html`<label class=${`rarity-option rarity-${rarity}`}>
          <input type="radio" name="rarity" value=${rarity} checked=${card.rarity === rarity} onChange=${() => onChange({ ...card, rarity })} />
          <i class="rarity-pip"></i>${rarity}
        </label>`)}
      </fieldset>
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
  // Edits are held here until the pause after the last keystroke, so a fast
  // typist makes one request, not one per letter.
  const pending = useRef(new Map());
  const timer = useRef(0);

  useEffect(() => {
    api("GET", `/api/sets/${setId}`).then((d) => {
      setDetail(d);
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
  const face = (c) => html`<${CardFace} card=${c.card} code=${set.code} number=${c.position} total=${cards.length} />`;

  return html`
    <section class="set-bar">
      <div class="set-title">
        <h1>${set.name}</h1>
        <p class="muted set-meta"><span class="set-code">${set.code}</span> · ${cards.length} ${cards.length === 1 ? "card" : "cards"}</p>
      </div>
      <p class="save-state" role="status">${status}</p>
      <details class="set-settings">
        <summary>Set settings</summary>
        <div class="row">
          <label>Name<input name="set-name" defaultValue=${set.name} maxLength="64" onChange=${(e) => patchSet({ name: e.currentTarget.value })} /></label>
          <label>Code<input name="set-code" defaultValue=${set.code} maxLength="5" onChange=${(e) => patchSet({ code: e.currentTarget.value })} /></label>
          <button type="button" class="danger delete-set" onClick=${() => setConfirming("set")}>Delete set</button>
        </div>
      </details>
    </section>
    <p class="error" role="alert">${error}</p>

    <section class="filmstrip" aria-label="Cards in this set">
      ${cards.map((c) => html`<div class="strip-item" key=${c.id}>
        <${CardFace}
          card=${c.card}
          code=${set.code}
          number=${c.position}
          total=${cards.length}
          selected=${c.id === selected}
          onClick=${async () => { await flush(); setSelected(c.id); }}
        />
      </div>`)}
      <button type="button" class="add-card" onClick=${() => addCard()}><span class="plus">+</span>Add card</button>
    </section>

    ${current
      ? html`<section class="creator">
          <div class="stage">
            <div class="preview">${face(current)}</div>
            <div class="stage-actions">
              <button type="button" class="quiet duplicate-card" onClick=${() => addCard(current.card)}>Duplicate</button>
              <button type="button" class="quiet danger-quiet delete-card" onClick=${() => setConfirming("card")}>Delete</button>
            </div>
          </div>
          <${Editor} key=${current.id} card=${current.card} onChange=${(card) => edit(current.id, card)} />
        </section>`
      : html`<p class="muted empty">No cards yet. Add the first one.</p>`}

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
