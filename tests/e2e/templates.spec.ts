import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function newSet(page, code: string) {
  await registerUser(page);
  await page.fill('.new-set input[name="name"]', `Looks ${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`);
  await page.fill('.new-set input[name="code"]', code);
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
}

test("a card can be given a layout and a frame colour, and keeps them", async ({ page }) => {
  await newSet(page, "LOK");
  await page.fill('input[name="mana_cost"]', "{R}");
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/tpl-modern/);
  await expect(preview).toHaveClass(/frame-r/);
  // One thumbnail per template the card can wear: the one look, and the saga.
  await expect(page.locator(".look-option")).toHaveCount(2);
  await expect(page.locator(".look-thumb .mtg-card.lay-saga")).toHaveCount(1);

  await page.locator(".look-option", { hasText: "Saga" }).click();
  await expect(preview).toHaveClass(/lay-saga/);
  await page.locator(".tint-option", { hasText: "Gold" }).click();
  await expect(preview).toHaveClass(/frame-gold/);
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.reload();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-saga/);
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/frame-gold/);

  // Auto goes back to following the mana cost.
  await page.locator(".tint-option.tint-auto").click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/frame-r/);
});

test("the set has one look, and a card that names its own layout keeps it", async ({ page }) => {
  await newSet(page, "DEF");
  await page.click(".add-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(2);
  await expect(page.locator(".filmstrip .mtg-card.lay-modern")).toHaveCount(2);

  // The set's default is the one look, and only a look may be chosen.
  await page.locator(".set-settings summary").click();
  await expect(page.locator('select[name="default-template"] option')).toHaveText(["Modern"]);

  // A card that names a layout is drawn by it, and stays that way across a reload.
  await page.locator(".filmstrip .mtg-card").first().click();
  await page.locator(".look-option", { hasText: "Saga" }).click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-saga/);
  await expect(page.locator(".save-state")).toHaveText(/Saved/);
  // A layout cannot be made the set's default, so the button is off.
  await expect(page.locator(".set-default-template")).toBeDisabled();

  await page.reload();
  await expect(page.locator(".filmstrip .mtg-card.lay-saga")).toHaveCount(1);
  await expect(page.locator(".filmstrip .mtg-card.lay-modern")).toHaveCount(1);
});
