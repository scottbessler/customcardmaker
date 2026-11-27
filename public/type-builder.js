import { html, useRef, useState } from "/public/vendor/htm-preact.js";
import { CARD_TYPES, SUPERTYPES, SUPPLEMENTAL_TYPES, buildTypeLine, parseTypeLine, suggestSubtypes, titleCase, toggled } from "/public/types.js";

/**
 * Picks for the type line: supertype and card-type buttons, and a typeahead for
 * subtypes. It holds no state of its own about the line -- it reads the text and
 * hands back new text -- so typing in the plain field and using these stay one.
 */
export function TypeBuilder({ typeLine, onChange }) {
  const parts = parseTypeLine(typeLine);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const input = useRef(null);

  const change = (next) => onChange(buildTypeLine({ ...parts, ...next }));
  const hasType = parts.types.length > 0 || parts.supertypes.length > 0;
  const options = open && hasType ? suggestSubtypes(parts.types, query, parts.subtypes) : [];

  function add(name) {
    const subtype = titleCase(name);
    if (!subtype) return;
    if (!parts.subtypes.some((chosen) => chosen.toLowerCase() === subtype.toLowerCase())) {
      change({ subtypes: [...parts.subtypes, subtype] });
    }
    setQuery("");
    setActive(-1);
    input.current?.focus();
  }

  function onKeyDown(event) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setOpen(true);
      const step = event.key === "ArrowDown" ? 1 : -1;
      setActive((index) => (options.length ? (index + step + options.length) % options.length : -1));
    } else if (event.key === "Enter" || event.key === ",") {
      // A highlighted suggestion wins; otherwise what was typed is the subtype.
      const pick = options[active] ?? query;
      if (pick.trim()) {
        event.preventDefault();
        add(pick);
      }
    } else if (event.key === "Escape") {
      setOpen(false);
      setActive(-1);
    } else if (event.key === "Backspace" && !query && parts.subtypes.length) {
      change({ subtypes: parts.subtypes.slice(0, -1) });
    }
  }

  const typeButton = (name) => html`<button
        type="button"
        class="chip type-pick"
        aria-pressed=${String(parts.types.includes(name))}
        onClick=${() => change({ types: toggled(parts.types, name) })}
      >${name}</button>`;

  return html`<div class="type-builder">
    <div class="type-picks" role="group" aria-label="Supertype">
      ${SUPERTYPES.map((name) => html`<button
        type="button"
        class="chip supertype-pick"
        aria-pressed=${String(parts.supertypes.includes(name))}
        onClick=${() => change({ supertypes: toggled(parts.supertypes, name) })}
      >${name}</button>`)}
    </div>
    <div class="type-picks" role="group" aria-label="Card type">
      ${CARD_TYPES.map(typeButton)}
    </div>
    <details class="supplemental-types">
      <summary>Supplemental card types</summary>
      <div class="type-picks" role="group" aria-label="Supplemental card types">
        ${SUPPLEMENTAL_TYPES.map(typeButton)}
      </div>
    </details>
    <div class="subtype-field">
      <span class="field-label">Subtypes</span>
      <div class="subtype-box">
        ${parts.subtypes.map((name) => html`<span class="subtype-token" key=${name}>${name}<button
          type="button"
          class="subtype-remove"
          aria-label=${`Remove ${name}`}
          onClick=${() => change({ subtypes: parts.subtypes.filter((chosen) => chosen !== name) })}
        >×</button></span>`)}
        <input
          ref=${input}
          class="subtype-input"
          name="subtype"
          role="combobox"
          aria-label="Add a subtype"
          aria-expanded=${String(options.length > 0)}
          aria-controls="subtype-options"
          aria-autocomplete="list"
          aria-activedescendant=${active >= 0 && options[active] ? `subtype-option-${active}` : undefined}
          autocomplete="off"
          disabled=${!hasType}
          placeholder=${hasType ? "Elf, Druid, Equipment…" : "Choose a card type first"}
          value=${query}
          onInput=${(e) => { setQuery(e.currentTarget.value); setOpen(true); setActive(-1); }}
          onFocus=${() => setOpen(true)}
          onBlur=${() => { setOpen(false); setActive(-1); }}
          onKeyDown=${onKeyDown}
        />
      </div>
      <ul id="subtype-options" class="subtype-options" role="listbox" aria-label="Subtype suggestions" hidden=${options.length === 0}>
        ${options.map((name, index) => html`<li
          id=${`subtype-option-${index}`}
          role="option"
          aria-selected=${String(index === active)}
          class="subtype-option"
          onMouseDown=${(e) => e.preventDefault()}
          onClick=${() => add(name)}
        >${name}</li>`)}
      </ul>
    </div>
  </div>`;
}
