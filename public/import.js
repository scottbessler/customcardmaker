// Wires the server-rendered Cockatrice import form. The page itself is markup
// in `render.rs`; this reads the chosen file, asks the server what the import
// would create, shows that, and only then imports it.

const form = document.getElementById("import-form");
const errorBox = document.getElementById("import-error");
const resultBox = document.getElementById("import-result");
const previewBox = document.getElementById("import-preview");

function showError(message) {
  if (!errorBox) return;
  errorBox.textContent = message;
  errorBox.hidden = false;
}

function clearMessages() {
  if (errorBox) {
    errorBox.hidden = true;
    errorBox.textContent = "";
  }
  if (resultBox) {
    resultBox.hidden = true;
    resultBox.textContent = "";
  }
}

const plural = (count, word) => `${count} ${word}${count === 1 ? "" : "s"}`;

function showResult(result) {
  if (!resultBox) return;
  resultBox.textContent = "";
  const line = document.createElement("span");
  line.append(`Imported ${plural(result.cards_imported, "card")}`);
  if (result.images_imported) {
    line.append(` with ${plural(result.images_imported, "picture")}`);
  }
  line.append(" into ");
  const link = document.createElement("a");
  link.href = `/sets/${result.set_id}`;
  link.textContent = `${result.set_name} (${result.set_code})`;
  line.append(link);
  resultBox.append(line);
  resultBox.hidden = false;
}

/** What the form says now: the options, and the file itself. */
function readForm() {
  const data = new FormData(form);
  return {
    setCode: (data.get("set_code") || "").trim(),
    setName: (data.get("set_name") || "").trim(),
    file: data.get("file"),
    images: data.get("images") === "on",
    wholeCards: data.get("whole_cards") === "on",
  };
}

/** Posts the file, asking either for a preview or for the import itself. */
async function post(preview) {
  const { setCode, setName, file, images, wholeCards } = readForm();
  const response = await fetch("/api/import/cockatrice", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      xml: await file.text(),
      set_code: setCode,
      set_name: setName || null,
      images,
      whole_cards: wholeCards,
      preview,
    }),
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload.error || `Import failed (${response.status})`);
  }
  return payload;
}

/** The preview: what would be made, what the file had that we could not keep, and the cards. */
function renderPreview(preview) {
  previewBox.textContent = "";

  const heading = document.createElement("h2");
  heading.textContent = `${preview.set_name} (${preview.set_code})`;
  previewBox.append(heading);

  const summary = document.createElement("p");
  summary.className = "muted";
  summary.textContent = `${plural(preview.cards_imported, "card")} would be created${
    preview.images_imported ? `, with ${plural(preview.images_imported, "picture")}` : ""
  }.`;
  previewBox.append(summary);

  if (preview.notes.length) {
    const notes = document.createElement("ul");
    notes.className = "import-notes";
    for (const note of preview.notes) {
      const item = document.createElement("li");
      item.textContent = note;
      notes.append(item);
    }
    previewBox.append(notes);
  }

  const list = document.createElement("ul");
  list.className = "import-cards";
  for (const card of preview.cards) {
    const item = document.createElement("li");
    const name = document.createElement("strong");
    name.textContent = card.name || "Untitled";
    item.append(name);
    if (card.type_line) {
      const type = document.createElement("span");
      type.textContent = card.type_line;
      item.append(" — ", type);
    }
    const marks = [];
    if (card.back) marks.push("two faces");
    if (card.printed) marks.push("kept as printed");
    if (!card.picture) marks.push("no picture");
    if (marks.length) {
      const mark = document.createElement("small");
      mark.textContent = ` (${marks.join(", ")})`;
      item.append(mark);
    }
    list.append(item);
  }
  previewBox.append(list);

  const actions = document.createElement("div");
  actions.className = "actions";
  const confirm = document.createElement("button");
  confirm.type = "button";
  confirm.className = "import-confirm";
  confirm.textContent = `Import ${plural(preview.cards_imported, "card")}`;
  confirm.addEventListener("click", () => run(false, confirm));
  const back = document.createElement("button");
  back.type = "button";
  back.className = "quiet";
  back.textContent = "Back";
  back.addEventListener("click", () => {
    previewBox.hidden = true;
  });
  actions.append(confirm, back);
  previewBox.append(actions);

  previewBox.hidden = false;
  previewBox.scrollIntoView({ block: "nearest" });
}

/** Previews or imports, whichever `preview` says. */
async function run(preview, button) {
  clearMessages();
  const { setCode, file } = readForm();
  if (!setCode) {
    showError("A set code is required.");
    return;
  }
  if (!file || !file.size) {
    showError("Choose a Cockatrice XML file to import.");
    return;
  }

  if (button) button.disabled = true;
  try {
    const payload = await post(preview);
    if (preview) {
      renderPreview(payload);
    } else {
      previewBox.hidden = true;
      showResult(payload);
    }
  } catch (error) {
    showError(error && error.message ? error.message : "Import failed");
  } finally {
    if (button) button.disabled = false;
  }
}

if (form) {
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    run(true, form.querySelector("button[type=submit]"));
  });
  // Changing an option makes the shown preview out of date.
  form.addEventListener("change", () => {
    if (previewBox) previewBox.hidden = true;
  });
}
