import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { expectImage, uncoveredGlyphs } from "./rendering";

// Both projects share one server and one database, so every run names its own set.
let sets = 0;
function uniqueSet() {
  sets += 1;
  const tag = `${Date.now().toString(36)}${sets}${Math.random().toString(36).slice(2, 5)}`;
  return { name: `Test set ${tag}`, code: "TST" };
}

async function createSet(page) {
  const set = uniqueSet();
  await page.goto("/");
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
  await expect(page.locator(".card-grid .mtg-card")).toHaveCount(count);
  await expect(page.locator(".card-grid .mtg-card").nth(count - 1)).toHaveAttribute("aria-pressed", "true");
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
  await expect(page.locator(".card-grid .mtg-card .mtg-foot span").first()).toHaveText(/^001\/002 • TST$/);
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
  await page.selectOption('select[name="rarity"]', "rare");

  await expect(preview).toHaveClass(/frame-u/);
  await expect(preview).toHaveClass(/rarity-rare/);
  await expect(preview.locator(".mtg-title")).toHaveText("Mist Warden");
  await expect(preview.locator(".mtg-cost .mana")).toHaveCount(2);
  await expect(preview.locator(".mtg-rules p")).toHaveCount(2);
  await expect(preview.locator(".mtg-rules .mana-t")).toHaveCount(1);
  await expect(preview.locator(".mtg-pt")).toHaveText("2/3");

  await page.fill('input[name="mana_cost"]', "{W}{U}");
  await expect(preview).toHaveClass(/frame-gold/);
});

test("edits survive a reload", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  await fillAndSave(page, 'input[name="name"]', "Persistent Pup");
  await page.reload();
  await expect(page.locator(".card-grid .mtg-title")).toHaveText("Persistent Pup");
  await expect(page.locator('input[name="name"]')).toHaveValue("Persistent Pup");
});

test("deleting a card renumbers the rest", async ({ page }) => {
  await createSet(page);
  await addCard(page, 1);
  await fillAndSave(page, 'input[name="name"]', "First");
  await addCard(page, 2);
  await fillAndSave(page, 'input[name="name"]', "Second");
  await page.locator(".card-grid .mtg-card").first().click();
  await page.click(".delete-card");
  await expect(page.locator(".card-grid .mtg-card")).toHaveCount(1);
  await expect(page.locator(".card-grid .mtg-title")).toHaveText("Second");
  await expect(page.locator(".card-grid .mtg-foot span").first()).toHaveText(/^001\/001/);
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
  await expectLayout(page, "finished-card", [
    ".preview .mtg-card",
    ".preview .mtg-name",
    ".preview .mtg-art",
    ".preview .mtg-type",
    ".preview .mtg-box",
    ".preview .mtg-pt",
    ".preview .mtg-foot",
  ]);
  await expectImage(page.locator(".preview .mtg-card"), "finished-card.png");
});

test("no control sits under the notch or home indicator on a phone", async ({ page }, info) => {
  test.skip(info.project.name !== "chromium-mobile");
  await createSet(page);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow, "the page must not scroll sideways").toBeLessThanOrEqual(0);
});
