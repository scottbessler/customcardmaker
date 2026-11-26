import { html, render, useCallback, useEffect, useRef, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";
import { CardFace } from "/public/card-face.js";

const SAVE_DELAY_MS = 500;
const RARITIES = ["common", "uncommon", "rare", "mythic"];
const setId = document.getElementById("app").dataset.setId;

function Field({ label, children }) {
  return html`<label>${label}${children}</label>`;
}

function Editor({ card, onChange, onDelete }) {
  const text = (key) => ({
    value: card[key],
    onInput: (e) => onChange({ ...card, [key]: e.currentTarget.value }),
  });
  return html`<form class="panel card-form" onSubmit=${(e) => e.preventDefault()}>
    <${Field} label="Name"><input name="name" maxlength="80" autocomplete="off" ...${text("name")} /><//>
    <div class="row">
      <${Field} label="Mana cost"><input name="mana_cost" maxlength="40" placeholder="{2}{U}{U}" autocomplete="off" ...${text("mana_cost")} /><//>
      <${Field} label="Rarity">
        <select name="rarity" value=${card.rarity} onChange=${(e) => onChange({ ...card, rarity: e.currentTarget.value })}>
          ${RARITIES.map((r) => html`<option value=${r}>${r}</option>`)}
        </select>
      <//>
    </div>
    <${Field} label="Type line"><input name="type_line" maxlength="80" placeholder="Creature — Merfolk Wizard" autocomplete="off" ...${text("type_line")} /><//>
    <${Field} label="Rules text"><textarea name="rules_text" rows="5" maxlength="1500" placeholder="Flying\n{T}: Draw a card." ...${text("rules_text")}></textarea><//>
    <${Field} label="Flavor text"><textarea name="flavor_text" rows="2" maxlength="600" ...${text("flavor_text")}></textarea><//>
    <div class="row">
      <${Field} label="Power / toughness"><input name="power_toughness" maxlength="12" placeholder="2/3" autocomplete="off" ...${text("power_toughness")} /><//>
      <${Field} label="Artist"><input name="artist" maxlength="80" autocomplete="off" ...${text("artist")} /><//>
    </div>
    <div class="row"><button type="button" class="danger delete-card" onClick=${onDelete}>Delete card</button></div>
  </form>`;
}

function SetPage() {
  const [detail, setDetail] = useState(null);
  const [selected, setSelected] = useState(null);
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
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

  function edit(id, card) {
    setDetail((d) => ({ ...d, cards: d.cards.map((c) => (c.id === id ? { ...c, card } : c)) }));
    pending.current.set(id, card);
    setStatus("Unsaved changes");
    clearTimeout(timer.current);
    timer.current = setTimeout(flush, SAVE_DELAY_MS);
  }

  async function addCard() {
    await flush();
    try {
      const card = await api("POST", `/api/sets/${setId}/cards`, {});
      setDetail((d) => ({ ...d, set: { ...d.set, card_count: d.set.card_count + 1 }, cards: [...d.cards, card] }));
      setSelected(card.id);
    } catch (e) {
      setError(e.message);
    }
  }

  async function deleteCard(id) {
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

  return html`
    <section class="set-head">
      <h1>${set.name}</h1>
      <div class="row">
        <label>Name<input name="set-name" defaultValue=${set.name} maxlength="64" onChange=${(e) => patchSet({ name: e.currentTarget.value })} /></label>
        <label>Code<input name="set-code" defaultValue=${set.code} maxlength="5" onChange=${(e) => patchSet({ code: e.currentTarget.value })} /></label>
        <button type="button" class="add-card" onClick=${addCard}>Add card</button>
      </div>
      <p class="save-state" role="status">${status}</p>
      <p class="error" role="alert">${error}</p>
    </section>
    <section class="set-cards">
      ${cards.length === 0 ? html`<p class="muted empty">No cards yet. Add the first one.</p>` : null}
      <div class="card-grid">
        ${cards.map((c) => html`<${CardFace}
          key=${c.id}
          card=${c.card}
          code=${set.code}
          number=${c.position}
          total=${cards.length}
          selected=${c.id === selected}
          onClick=${async () => { await flush(); setSelected(c.id); }}
        />`)}
      </div>
    </section>
    ${current
      ? html`<section class="set-editor">
          <div class="preview"><${CardFace} card=${current.card} code=${set.code} number=${current.position} total=${cards.length} /></div>
          <${Editor} key=${current.id} card=${current.card} onChange=${(card) => edit(current.id, card)} onDelete=${() => deleteCard(current.id)} />
        </section>`
      : null}
  `;
}

render(html`<${SetPage} />`, document.getElementById("app"));
