// Wires the server-rendered Cockatrice import form. The page itself is markup
// in `render.rs`; this reads the chosen file, asks the server what the import
// would create, shows that, then runs it in the background and follows its
// progress.

const form = document.getElementById("import-form");
const errorBox = document.getElementById("import-error");
const resultBox = document.getElementById("import-result");
const previewBox = document.getElementById("import-preview");
const progressBox = document.getElementById("import-progress");

// The job being followed, so a page left and come back to picks it up again.
const JOB_KEY = "ccm-import-job";
const POLL_MS = 700;

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

/** A list of plain sentences, as a `<ul>`. */
function noteList(className, lines) {
  const list = document.createElement("ul");
  list.className = className;
  for (const line of lines) {
    const item = document.createElement("li");
    item.textContent = line;
    list.append(item);
  }
  return list;
}

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
  if (result.failures && result.failures.length) {
    resultBox.append(
      noteList("import-notes", result.failures.map((failure) => `No picture: ${failure}`)),
    );
  }
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

/** Posts the file, asking for a preview, a background import, or neither. */
async function post({ preview = false, background = false } = {}) {
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
      background,
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
    previewBox.append(noteList("import-notes", preview.notes));
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
  confirm.addEventListener("click", () => startImport(confirm));
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

/** How a running import is getting on. */
function showProgress(progress) {
  if (!progressBox) return;
  progressBox.textContent = "";

  const heading = document.createElement("h2");
  heading.textContent = `${progress.set_name} (${progress.set_code})`;
  progressBox.append(heading);

  const bar = document.createElement("progress");
  bar.max = Math.max(1, progress.pictures_total || progress.cards_total);
  bar.value = progress.pictures_total ? progress.pictures_tried : progress.cards_done;
  progressBox.append(bar);

  const text = document.createElement("p");
  text.className = "muted";
  text.textContent = progress.pictures_total
    ? `Fetched ${progress.pictures_done} of ${progress.pictures_total} pictures.`
    : `Read ${progress.cards_done} of ${progress.cards_total} cards.`;
  progressBox.append(text);

  if (progress.failures.length) {
    progressBox.append(
      noteList("import-notes", progress.failures.map((failure) => `No picture: ${failure}`)),
    );
  }
  progressBox.hidden = false;
}

/** Follows a job until it ends, then shows the set (or why there is none). */
async function follow(jobId) {
  localStorage.setItem(JOB_KEY, jobId);
  // Polling is the point: each turn waits for the last.
  /* oxlint-disable no-await-in-loop */
  for (;;) {
    const response = await fetch(`/api/import/cockatrice/${jobId}`);
    if (!response.ok) {
      localStorage.removeItem(JOB_KEY);
      throw new Error("That import is no longer here.");
    }
    const progress = await response.json();
    showProgress(progress);
    if (progress.state === "done") {
      localStorage.removeItem(JOB_KEY);
      progressBox.hidden = true;
      showResult({
        set_id: progress.set_id,
        set_name: progress.set_name,
        set_code: progress.set_code,
        cards_imported: progress.cards_total,
        images_imported: progress.pictures_done,
        failures: progress.failures,
      });
      return;
    }
    if (progress.state === "failed") {
      localStorage.removeItem(JOB_KEY);
      progressBox.hidden = true;
      showError(progress.error || "Import failed");
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
  /* oxlint-enable no-await-in-loop */
}

/** Starts the import in the background and follows it. */
async function startImport(button) {
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
    const { job_id: jobId } = await post({ background: true });
    previewBox.hidden = true;
    await follow(jobId);
  } catch (error) {
    showError(error && error.message ? error.message : "Import failed");
  } finally {
    if (button) button.disabled = false;
  }
}

/** Previews what the form would import. */
async function showPreview(button) {
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
    renderPreview(await post({ preview: true }));
  } catch (error) {
    showError(error && error.message ? error.message : "Import failed");
  } finally {
    if (button) button.disabled = false;
  }
}

if (form) {
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    showPreview(form.querySelector("button[type=submit]"));
  });
  // Changing an option makes the shown preview out of date.
  form.addEventListener("change", () => {
    if (previewBox) previewBox.hidden = true;
  });

  // A page left while an import ran comes back to it.
  const saved = localStorage.getItem(JOB_KEY);
  if (saved) {
    follow(saved).catch(() => localStorage.removeItem(JOB_KEY));
  }
}
