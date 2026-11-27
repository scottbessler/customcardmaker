import { html, useEffect, useRef, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";

/**
 * Search Creative Commons images and pick one for a card. The server does the
 * searching and the fetching; this only shows results and says which was chosen.
 * What comes back is `{ id, credit }`: the new upload, and who made it.
 */
export function ArtSearchDialog({ open, onPick, onCancel }) {
  const ref = useRef(null);
  const [query, setQuery] = useState("");
  const [nc, setNc] = useState(false);
  const [found, setFound] = useState(null);
  const [busy, setBusy] = useState(false);
  const [picking, setPicking] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    const dialog = ref.current;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  async function search(page = 1) {
    setBusy(true);
    setError("");
    try {
      const params = new URLSearchParams({ q: query, page: String(page), ...(nc ? { nc: "true" } : {}) });
      const next = await api("GET", `/api/art/search?${params}`);
      setFound((was) => (page > 1 && was ? { ...next, results: [...was.results, ...next.results] } : next));
    } catch (e) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  async function pick(result) {
    setPicking(result.id);
    setError("");
    try {
      onPick(await api("POST", "/api/art/openverse", { id: result.id }));
    } catch (e) {
      setError(e.message);
    } finally {
      setPicking("");
    }
  }

  const more = found && found.page < found.page_count;
  return html`<dialog ref=${ref} class="confirm-dialog art-search" onCancel=${onCancel}>
    <div>
      <h2>Find free art</h2>
      <form class="art-search-form" onSubmit=${(e) => { e.preventDefault(); search(1); }}>
        <div class="row">
          <label>Search<input type="search" name="art-query" required maxlength="100" autocomplete="off" placeholder="misty forest" value=${query} onInput=${(e) => setQuery(e.currentTarget.value)} /></label>
          <button type="submit" class="art-search-go" aria-busy=${String(busy && !picking)} disabled=${busy}>Search</button>
        </div>
        <label class="public-toggle">
          <input type="checkbox" name="art-nc" checked=${nc} onChange=${(e) => setNc(e.currentTarget.checked)} />
          <span>Include non-commercial licences</span>
        </label>
      </form>
      <p class="muted art-search-note">From Openverse. Crediting the artist is part of the licence, so it is kept with the art and shown with it.</p>
      <p class="error" role="alert">${error}</p>
      ${found && found.results.length === 0 ? html`<p class="muted art-empty">Nothing matched. Try another word.</p>` : null}
      <ul class="art-results">
        ${(found?.results ?? []).map((r) => html`<li key=${r.id}>
          <button type="button" class="art-result" disabled=${picking !== ""} onClick=${() => pick(r)} aria-label=${`Use “${r.title}” by ${r.creator}`}>
            <img src=${r.thumbnail} alt="" loading="lazy" referrerpolicy="no-referrer" />
            <span class="art-result-title">${picking === r.id ? "Using…" : r.title}</span>
            <span class="art-result-by">${r.creator}</span>
            <span class="art-result-license">${r.license_label}</span>
          </button>
        </li>`)}
      </ul>
      ${more ? html`<button type="button" class="quiet art-more" disabled=${busy} onClick=${() => search(found.page + 1)}>More results</button>` : null}
      <footer><button type="button" class="quiet art-search-close" onClick=${onCancel}>Close</button></footer>
    </div>
  </dialog>`;
}

/** “Title” by Creator · CC BY 2.0 · source — the credit a licence asks for. */
export function Credit({ credit }) {
  if (!credit) return null;
  // The separators are explicit strings: a template drops the space around an element.
  return html`<span class="art-credit">${`“${credit.title}” by ${credit.creator}`}${" · "}<a href=${credit.license_url} rel="noopener noreferrer" target="_blank">${credit.license_label}</a>${credit.source_url ? html`${" · "}<a href=${credit.source_url} rel="noopener noreferrer" target="_blank">source</a>` : null}</span>`;
}
