import { html, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";
import { artUrl } from "/public/art.js";

export function RulesCleanup({ text, onApply }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState(null);
  const [message, setMessage] = useState("");

  async function suggest() {
    setMessage("");
    setBusy(true);
    try {
      const { text: proposed } = await api("POST", "/api/ai/rules", { text });
      if (proposed === text) setMessage("No wording changes suggested.");
      else setResult({ before: text, proposed });
    } catch (error) {
      setMessage(error.message);
    } finally {
      setBusy(false);
    }
  }

  return html`<div class="ai-tool">
    <button type="button" class="quiet" disabled=${busy || !text.trim() || Boolean(result)} onClick=${suggest}>
      ${busy ? "Suggesting…" : "Suggest cleanup"}
    </button>
    ${message ? html`<p class="muted" role="status">${message}</p>` : null}
    ${result ? html`<div class="ai-review" aria-label="Rules text suggestion">
      <div class="ai-compare">
        <div><h3>Before</h3><p>${result.before}</p></div>
        <div><h3>Suggested</h3><p>${result.proposed}</p></div>
      </div>
      ${text !== result.before ? html`<p class="muted" role="status">The rules text changed. Request a fresh suggestion to apply it.</p>` : null}
      <div class="ai-actions">
        <button type="button" class="quiet" onClick=${() => setResult(null)}>Discard</button>
        <button type="button" disabled=${text !== result.before} onClick=${() => { onApply(result.proposed); setResult(null); }}>Apply wording</button>
      </div>
    </div>` : null}
  </div>`;
}

export function ArtExtension({ art, ratio, disabled, onApply }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState(null);
  const [error, setError] = useState("");

  async function extend() {
    setError("");
    setBusy(true);
    const aspect = ratio > 1.2 ? "16:9" : ratio < 0.85 ? "3:4" : "1:1";
    try {
      const candidate = await api("POST", "/api/ai/extend-art", { id: art, aspect });
      setResult(candidate);
    } catch (failure) {
      setError(failure.message);
    } finally {
      setBusy(false);
    }
  }

  return html`<div class="ai-tool">
    <button type="button" class="quiet" disabled=${disabled || busy || Boolean(result)} onClick=${extend}>
      ${busy ? "Extending…" : "Extend background"}
    </button>
    ${error ? html`<p class="error" role="alert">${error}</p>` : null}
    ${result ? html`<div class="ai-review" aria-label="Extended art suggestion">
      <div class="ai-compare">
        <figure><figcaption>Original</figcaption><img src=${artUrl(art)} alt="Original art" /></figure>
        <figure><figcaption>Suggested</figcaption><img src=${artUrl(result.id)} alt="Extended art" /></figure>
      </div>
      <div class="ai-actions">
        <button type="button" class="quiet" onClick=${() => setResult(null)}>Discard</button>
        <button type="button" disabled=${disabled} onClick=${() => { onApply(result); setResult(null); }}>Use extended art</button>
      </div>
    </div>` : null}
  </div>`;
}
