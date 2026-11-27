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

test("a card can be given another template and frame colour, and keeps them", async ({ page }) => {
  await newSet(page, "LOK");
  await page.fill('input[name="mana_cost"]', "{R}");
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/tpl-modern/);
  await expect(preview).toHaveClass(/frame-r/);
  // One thumbnail per template this card can wear (the four looks and Saga), each the card itself.
  await expect(page.locator(".look-option")).toHaveCount(5);
  await expect(page.locator(".look-thumb .mtg-card.tpl-full-art")).toHaveCount(1);

  await page.locator(".look-option", { hasText: "Borderless" }).click();
  await expect(preview).toHaveClass(/tpl-borderless/);
  await page.locator(".tint-option", { hasText: "Gold" }).click();
  await expect(preview).toHaveClass(/frame-gold/);
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.reload();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/tpl-borderless/);
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/frame-gold/);

  // Auto goes back to following the mana cost.
  await page.locator(".tint-option.tint-auto").click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/frame-r/);
});

test("the set's default template applies to every card that names none", async ({ page }) => {
  await newSet(page, "DEF");
  await page.click(".add-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(2);
  await page.locator(".set-settings summary").click();
  await page.selectOption('select[name="default-template"]', "classic");
  await expect(page.locator(".filmstrip .mtg-card.tpl-classic")).toHaveCount(2);
  // A card that names its own look keeps it.
  await page.locator(".look-option", { hasText: "Full art" }).click();
  await expect(page.locator(".filmstrip .mtg-card.tpl-full-art")).toHaveCount(1);
  await expect(page.locator(".filmstrip .mtg-card.tpl-classic")).toHaveCount(1);
  await page.reload();
  await expect(page.locator(".filmstrip .mtg-card.tpl-classic")).toHaveCount(1);
  // Making a card's look the set's default restyles the rest. Open that card first: the
  // reload does not say which one is selected.
  await page.locator(".filmstrip .mtg-card.tpl-full-art").click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/tpl-full-art/);
  await page.click(".set-default-template");
  await expect(page.locator(".filmstrip .mtg-card.tpl-full-art")).toHaveCount(2);
});
