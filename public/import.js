// Wires the server-rendered Cockatrice import form. The page itself is markup
// in `render.rs`; this only posts it and reports what happened.

const form = document.getElementById("import-form");
const errorBox = document.getElementById("import-error");
const resultBox = document.getElementById("import-result");

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

function showResult(result) {
  if (!resultBox) return;
  resultBox.textContent = "";
  const line = document.createElement("span");
  line.append(
    `Imported ${result.cards_imported} ${
      result.cards_imported === 1 ? "card" : "cards"
    } into `,
  );
  const link = document.createElement("a");
  link.href = `/sets/${result.set_id}`;
  link.textContent = `${result.set_name} (${result.set_code})`;
  line.append(link);
  resultBox.append(line);
  resultBox.hidden = false;
}

if (form) {
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    clearMessages();

    const data = new FormData(form);
    const setCode = (data.get("set_code") || "").trim();
    const setName = (data.get("set_name") || "").trim();
    const xml = (data.get("xml") || "").trim();
    if (!setCode || !xml) {
      showError("A set code and the XML are both required.");
      return;
    }

    const button = form.querySelector("button[type=submit]");
    if (button) button.disabled = true;
    try {
      const response = await fetch("/api/import/cockatrice", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          xml,
          set_code: setCode,
          set_name: setName || null,
        }),
      });
      const payload = await response.json().catch(() => ({}));
      if (!response.ok) {
        throw new Error(payload.error || `Import failed (${response.status})`);
      }
      showResult(payload);
    } catch (error) {
      showError(error && error.message ? error.message : "Import failed");
    } finally {
      if (button) button.disabled = false;
    }
  });
}
