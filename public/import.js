// Cockatrice Import Frontend
import { html, render, useCallback, useEffect, useState } from "/public/vendor/htm-preact.js";
import { api } from "/public/api.js";

const app = document.getElementById("app");

// Check if user is signed in - show appropriate UI
async function checkAuth() {
  try {
    const user = await api("GET", "/healthcheck");
    // Healthcheck just returns "OK", but we can use it to verify the server is up
    showImportUI();
  } catch (e) {
    app.innerHTML = html`
      <div class="error">
        <p>Not signed in. <a href="/">Sign in first</a>.</p>
      </div>
    `;
  }
}

checkAuth();

// Import types from the Cockatrice XML
const IMPORT_TYPES = {
  mythic: "Mythic",
  rare: "Rare", 
  uncommon: "Uncommon",
  common: "Common"
};

// Show the import UI
function showImportUI() {
  app.innerHTML = html`
    <section class="panel">
      <h2>Import from Cockatrice</h2>
      <p>Paste or upload a Cockatrice carddatabase XML file to import cards into a new set.</p>
      
      <form id="import-form" class="import-form">
        <div class="form-group">
          <label for="set-code">Set Code <span class="required">*</span></label>
          <input type="text" id="set-code" name="set-code" required
            placeholder="e.g., RND, MYSET" maxlength="5"
            pattern="[A-Za-z0-9]{3,5}"
            value="RND">
        </div>
        
        <div class="form-group">
          <label for="xml-input">Cockatrice XML <span class="required">*</span></label>
          <textarea id="xml-input" name="xml-input" rows="20" cols="100" required></textarea>
          <small>Paste the full Cockatrice carddatabase XML content here.</small>
        </div>
        
        <div class="form-actions">
          <button type="submit" class="btn-primary">Import Cards</button>
          <button type="button" class="btn-secondary" onClick="window.location.href='/'">Cancel</button>
        </div>
      </form>
      
      <div id="import-result" class="import-result" style="margin-top: 2rem; display: none;"></div>
    </section>
  `;
  
  setupImportForm();
}

function setupImportForm() {
  const form = document.getElementById("import-form");
  const resultDiv = document.getElementById("import-result");
  const setCodeInput = document.getElementById("set-code");
  const xmlTextarea = document.getElementById("xml-input");
  
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    
    const setCode = setCodeInput.value.trim().toUpperCase();
    const xmlContent = xmlTextarea.value.trim();
    
    if (!setCode || !xmlContent) {
      showError("Please fill in all fields");
      return;
    }
    
    // Validate set code length
    if (setCode.length < 3 || setCode.length > 5) {
      showError("Set code must be 3-5 characters");
      return;
    }
    
    // Show loading state
    showLoading(true);
    resultDiv.style.display = "none";
    resultDiv.innerHTML = "";
    
    try {
      const result = await api("POST", "/api/import/cockatrice", {
        xml: xmlContent,
        set_code: setCode
      });
      
      showResult(result);
    } catch (error) {
      showError(error.message || "Import failed");
    } finally {
      showLoading(false);
    }
  });
}

function showLoading(loading) {
  const submitBtn = document.querySelector('#import-form button[type="submit"]');
  if (loading) {
    submitBtn.disabled = true;
    submitBtn.textContent = "Importing...";
  } else {
    submitBtn.disabled = false;
    submitBtn.textContent = "Import Cards";
  }
}

function showResult(result) {
  const resultDiv = document.getElementById("import-result");
  resultDiv.innerHTML = html`
    <div class="import-result-success">
      <h3>✅ Import Complete</h3>
      <p>Imported <strong>${result.cards_imported || 0}</strong> cards into set "<strong>${result.set_code}</strong>"</p>
      <p><a href="/sets/${result.set_id}">View your set</a> | <a href="/">Back home</a></p>
    </div>
  `;
  resultDiv.style.display = "block";
}

function showError(message) {
  const resultDiv = document.getElementById("import-result");
  resultDiv.innerHTML = html`
    <div class="import-result-error">
      <h3>❌ Import Failed</h3>
      <p>${message}</p>
      <p><a href="javascript:history.back()">Go Back</a></p>
    </div>
  `;
  resultDiv.style.display = "block";
}

// Initialize on load
checkAuth();
