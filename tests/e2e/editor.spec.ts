import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { expectImage, uncoveredGlyphs } from "./rendering";
import { registerUser } from "./session";

// Both projects share one server and one database, so every run names its own set.
let sets = 0;
function uniqueSet() {
  sets += 1;
  const tag = `${Date.now().toString(36)}${sets}${Math.random().toString(36).slice(2, 5)}`;
  return { name: `Test set ${tag}`, code: "TST" };
}

async function createSet(page) {
  const set = uniqueSet();
  await registerUser(page);
  await page.fill('.new-set input[name="name"]', set.name);
  await page.fill('.new-set input[name="code"]', set.code);
  await page.click(".new-set button");
  await expect(page).toHaveURL(/\/sets\//);
  await expect(page.locator("h1")).toHaveText(set.name);
  return set;
}

/** The Nth card exists and is the one being edited, so the next `fill` lands on it. */
async function addCard(page, count: number) {
  await page.click(".add-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(count);
  await expect(page.locator(".filmstrip .mtg-card").nth(count - 1)).toHaveAttribute("aria-pressed", "true");
}

/** Types into a field and waits for the save that follows the pause. */
async function fillAndSave(page, selector: string, value: string) {
  await Promise.all([
    page.waitForResponse((r) => r.request().method() === "PUT" && r.ok()),
    page.fill(selector, value),
  ]);
}

test("pins every non-ASCII glyph to a vendored font", async ({ page }) => {
  await page.goto("/");
  expect(await uncoveredGlyphs(page)).toEqual([]);
});

test("a new set starts empty and takes cards", async ({ page }) => {
  await createSet(page);
  await expect(page.locator(".empty")).toBeVisible();
  await addCard(page, 1);
  await addCard(page, 2);
  // Collector numbers run 1..n in the order cards were added.
  await expect(page.locator(".filmstrip .mtg-card .mtg-foot span").first()).toHaveText(/^001\/002 • TST$/);
  await expect(page.locator(".set-meta")).toContainText("2 cards");
});

test("the preview follows the form and the frame follows the mana cost", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/frame-colorless/);

  await page.fill('input[name="name"]', "Mist Warden");
  await page.fill('input[name="mana_cost"]', "{1}{U}");
  await page.fill('input[name="type_line"]', "Creature — Merfolk Wizard");
  await page.fill('textarea[name="rules_text"]', "Flying\n{T}: Draw a card.");
  await page.fill('input[name="power_toughness"]', "2/3");
  await page.locator(".rarity-option", { hasText: "rare" }).click();

  await expect(preview).toHaveClass(/frame-u/);
  await expect(preview).toHaveClass(/rarity-rare/);
  await expect(preview.locator(".mtg-title")).toHaveText("Mist Warden");
  await expect(preview.locator(".mtg-cost .mana")).toHaveCount(2);
  await expect(preview.locator(".mtg-rules p")).toHaveCount(2);
  await expect(preview.locator(".mtg-rules .ms-tap")).toHaveCount(1);
  await expect(preview.locator(".mtg-pt")).toHaveText("2/3");

  await page.fill('input[name="mana_cost"]', "{W}{U}");
  await expect(preview).toHaveClass(/frame-gold/);
});

test("the symbol palette types into whichever field you were in", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  const cost = page.locator('input[name="mana_cost"]');
  const rules = page.locator('textarea[name="rules_text"]');
  const key = (symbol: string) => page.locator(`.symbol-key[data-symbol="${symbol}"]`);

  await cost.focus();
  await expect(page.locator(".palette-hint")).toContainText("mana cost");
  await expect(key("T"), "a cost cannot tap").toBeDisabled();
  await key("2").click();
  await key("U").click();
  await key("U").click();
  await expect(cost).toHaveValue("{2}{U}{U}");

  // Typing carries on after the symbol, and a symbol lands at the caret.
  await cost.press("Home");
  await cost.press("ArrowRight");
  await cost.press("ArrowRight");
  await cost.press("ArrowRight");
  await key("X").click();
  await expect(cost).toHaveValue("{2}{X}{U}{U}");

  await rules.focus();
  await expect(page.locator(".palette-hint")).toContainText("rules text");
  await expect(key("T")).toBeEnabled();
  await rules.fill("Tap: ");
  await key("T").click();
  await expect(rules).toHaveValue("Tap: {T}");
  await expect(page.locator(".preview .mtg-rules .ms-tap")).toHaveCount(1);
});

test("a type button swaps the card type and keeps the subtypes", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  const line = page.locator('input[name="type_line"]');
  const pick = (type: string) => page.locator(".type-pick", { hasText: type });

  await pick("Creature").click();
  await expect(line).toHaveValue("Creature");
  await expect(pick("Creature")).toHaveAttribute("aria-pressed", "true");
  await line.fill("Creature — Elf Druid");
  await pick("Instant").click();
  await expect(line).toHaveValue("Instant — Elf Druid");
  await expect(pick("Creature")).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator(".preview .mtg-type")).toContainText("Instant — Elf Druid");
});

test("long fields count against the limit the server set", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  await page.fill('textarea[name="rules_text"]', "Flying");
  await expect(page.locator("label", { has: page.locator('textarea[name="rules_text"]') }).locator(".count")).toHaveText("6/1500");
  await expect(page.locator('input[name="name"]')).toHaveAttribute("maxlength", "80");
});

test("edits survive a reload", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  await fillAndSave(page, 'input[name="name"]', "Persistent Pup");
  await page.reload();
  await expect(page.locator(".filmstrip .mtg-title")).toHaveText("Persistent Pup");
  await expect(page.locator('input[name="name"]')).toHaveValue("Persistent Pup");
});

test("duplicating a card copies it to the end of the set", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  await fillAndSave(page, 'input[name="name"]', "Twin");
  await page.click(".duplicate-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(2);
  await expect(page.locator(".filmstrip .mtg-card").nth(1)).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator('input[name="name"]')).toHaveValue("Twin");
  await expect(page.locator(".filmstrip .mtg-foot span").nth(2)).toHaveText(/^002\/002/);
});

test("deleting a card asks first, then renumbers the rest", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  await fillAndSave(page, 'input[name="name"]', "First");
  await addCard(page, 2);
  await fillAndSave(page, 'input[name="name"]', "Second");
  await page.locator(".filmstrip .mtg-card").first().click();

  await page.click(".delete-card");
  // Both dialogs live in the page; only the one that is asking is open.
  const asking = page.locator(".confirm-dialog[open]");
  await expect(asking).toBeVisible();
  await expect(asking).toContainText("First");
  await asking.locator(".confirm-cancel").click();
  await expect(asking).toHaveCount(0);
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(2);

  await page.click(".delete-card");
  await asking.locator(".confirm-accept").click();
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
  await expect(page.locator(".filmstrip .mtg-title")).toHaveText("Second");
  await expect(page.locator(".filmstrip .mtg-foot span").first()).toHaveText(/^001\/001/);
});

test("a set can be renamed and deleted from its settings", async ({ page }) => {
  const set = await createSet(page);
  await page.click(".set-settings summary");
  await page.fill('input[name="set-name"]', `${set.name} II`);
  await page.locator('input[name="set-name"]').blur();
  await expect(page.locator("h1")).toHaveText(`${set.name} II`);

  await page.click(".delete-set");
  await page.locator(".confirm-dialog[open] .confirm-accept").click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.locator(".sets-page")).toBeVisible();
  await expect(page.getByText(`${set.name} II`)).toHaveCount(0);
});

test("a finished card holds its layout", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  await page.fill('input[name="name"]', "Mist Warden");
  await page.fill('input[name="mana_cost"]', "{1}{U}");
  await page.fill('input[name="type_line"]', "Creature — Merfolk Wizard");
  await page.fill('textarea[name="rules_text"]', "Flying\n{T}: Draw a card.");
  await page.fill('textarea[name="flavor_text"]', "It sees the tide before the moon does.");
  await page.fill('input[name="power_toughness"]', "2/3");
  await fillAndSave(page, 'input[name="artist"]', "A. Painter");
  // Boxes are viewport-relative, so measure from a fixed scroll position:
  // wherever the last `fill` left the page would otherwise leak into every `y`.
  await page.evaluate(() => window.scrollTo(0, 0));
  await expectLayout(page, "finished-card", [
    ".site-header",
    ".filmstrip",
    ".filmstrip .strip-item",
    ".add-card",
    ".stage",
    ".preview .mtg-card",
    ".preview .mtg-name",
    ".preview .mtg-art",
    ".preview .mtg-type",
    ".preview .mtg-box",
    ".preview .mtg-pt",
    ".preview .mtg-foot",
    ".card-form",
    ".form-section",
    ".palette",
    ".symbol-key",
  ]);
  await expectImage(page.locator(".preview .mtg-card"), "finished-card.png");
  await expectImage(page.locator(".card-form"), "card-form.png");
});

test("the creator fits a phone: nothing scrolls sideways but the filmstrip", async ({ page }, info) => {
  test.skip(info.project.name !== "chromium-mobile");
  await createSet(page);
  await addCard(page, 1);
  await addCard(page, 2);
  await addCard(page, 3);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow, "the page must not scroll sideways").toBeLessThanOrEqual(0);
  // The form stacks under the preview on a narrow screen.
  const stage = await page.locator(".stage").boundingBox();
  const form = await page.locator(".card-form").boundingBox();
  expect(form!.y).toBeGreaterThan(stage!.y + stage!.height - 1);
});
