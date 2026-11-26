import { html, useEffect, useRef, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";

const NEW_SET = "__new__";

const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** What to do with the cards that are picked. Nothing here knows how. */
export function BulkBar({ count, total, onAll, onNone, onMove, onCopy, onDelete }) {
  const none = count === 0;
  return html`<div class="bulk-bar" role="toolbar" aria-label="Actions for the selected cards">
    <span class="bulk-count" role="status">${count} of ${plural(total, "card")} selected</span>
    <div class="bulk-actions">
      <button type="button" class="quiet bulk-all" disabled=${count === total} onClick=${onAll}>Select all</button>
      <button type="button" class="quiet bulk-none" disabled=${none} onClick=${onNone}>None</button>
      <button type="button" class="bulk-move" disabled=${none} onClick=${onMove}>Move…</button>
      <button type="button" class="bulk-copy" disabled=${none} onClick=${onCopy}>Copy…</button>
      <button type="button" class="danger bulk-delete" disabled=${none} onClick=${onDelete}>Delete</button>
    </div>
  </div>`;
}

/**
 * Pick a set to move or copy the chosen cards into, or make a new one for them.
 * It only collects the answer; `onSubmit` does the work and may throw, and what
 * it throws is shown here, so the dialog stays open for another try.
 */
export function TransferDialog({ open, mode, count, currentSetId, onSubmit, onCancel }) {
  const ref = useRef(null);
  const [sets, setSets] = useState(null);
  const [choice, setChoice] = useState(NEW_SET);
  const [name, setName] = useState("");
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const verb = mode === "copy" ? "Copy" : "Move";

  useEffect(() => {
    const dialog = ref.current;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  // Which sets there are is asked fresh each time it opens, so one made in
  // another tab a moment ago is on the list.
  useEffect(() => {
    if (!open) return;
    setError("");
    setSets(null);
    api("GET", "/api/sets").then((all) => {
      // Moving into the set the cards are already in would do nothing.
      const others = mode === "move" ? all.filter((set) => set.id !== currentSetId) : all;
      setSets(others);
      setChoice(others[0]?.id ?? NEW_SET);
    }, (e) => setError(e.message));
  }, [open, mode, currentSetId]);

  async function submit(event) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      await onSubmit(choice === NEW_SET ? { kind: "new", name, code } : { kind: "existing", set: choice });
    } catch (e) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  const fresh = choice === NEW_SET;
  return html`<dialog ref=${ref} class="confirm-dialog transfer-dialog" onCancel=${onCancel}>
    <form onSubmit=${submit}>
      <h2>${verb} ${plural(count, "card")}</h2>
      <label>To
        <select name="destination" value=${choice} disabled=${sets === null} onChange=${(e) => setChoice(e.currentTarget.value)}>
          ${(sets ?? []).map((set) => html`<option value=${set.id}>${set.code} · ${set.name}</option>`)}
          <option value=${NEW_SET}>A new set…</option>
        </select>
      </label>
      ${fresh
        ? html`<div class="row">
            <label>Name<input name="new-name" required maxlength="64" autocomplete="off" value=${name} onInput=${(e) => setName(e.currentTarget.value)} /></label>
            <label>Code<input name="new-code" required minlength="3" maxlength="5" autocomplete="off" value=${code} onInput=${(e) => setCode(e.currentTarget.value)} /></label>
          </div>`
        : null}
      <p class="error" role="alert">${error}</p>
      <footer>
        <button type="button" class="quiet transfer-cancel" onClick=${onCancel}>Cancel</button>
        <button type="submit" class="transfer-accept" aria-busy=${String(busy)} disabled=${busy || sets === null}>${verb} ${plural(count, "card")}</button>
      </footer>
    </form>
  </dialog>`;
}
