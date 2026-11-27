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

/** The picker's option for one template. */
const option = (page, id: string) => page.locator(`.look-option:has(input[name="template"][value="${id}"])`);

test("a card can be given a layout and a frame colour, and keeps them", async ({ page }) => {
  await newSet(page, "LOK");
  await page.fill('input[name="mana_cost"]', "{R}");
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/tpl-modern/);
  await expect(preview).toHaveClass(/frame-r/);
  // Every layout is listed; the card can wear the look or be a saga.
  await expect(page.locator(".look-option")).toHaveCount(15);
  await expect(option(page, "saga").locator("input")).toBeEnabled();
  await expect(option(page, "saga").locator(".look-thumb .mtg-card")).toHaveClass(/lay-saga/);

  await option(page, "saga").click();
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

test("a layout the card cannot wear is shown, and says why", async ({ page }) => {
  await newSet(page, "NOPE");
  await page.fill('input[name="mana_cost"]', "{R}");
  // A card with a mana cost cannot be a token; a battle needs a defense.
  await expect(option(page, "token").locator(".look-needs")).toHaveText("not with mana cost");
  await expect(option(page, "token").locator("input")).toBeDisabled();
  await expect(option(page, "battle").locator(".look-needs")).toHaveText("needs defense");

  // Giving it a defense makes the battle offered, and its own thumbnail is of the battle.
  await page.check('input[name="feature-defense"]');
  await expect(option(page, "battle").locator("input")).toBeEnabled();
  await expect(option(page, "battle").locator(".look-needs")).toHaveCount(0);
  await option(page, "battle").click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-battle/);
  await page.fill('input[name="defense"]', "5");
  await expect(page.locator(".preview .mtg-pt")).toHaveText("5");

  // A two-halved layout needs a second half, and says so.
  await expect(option(page, "split").locator(".look-needs")).toHaveText("needs a second half");
  await expect(option(page, "split").locator("input")).toBeDisabled();
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
  await option(page, "saga").click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-saga/);
  await expect(page.locator(".save-state")).toHaveText(/Saved/);
  // A layout cannot be made the set's default, so the button is off.
  await expect(page.locator(".set-default-template")).toBeDisabled();

  await page.reload();
  await expect(page.locator(".filmstrip .mtg-card.lay-saga")).toHaveCount(1);
  await expect(page.locator(".filmstrip .mtg-card.lay-modern")).toHaveCount(1);
});

test("a two-halved layout draws the card and its second half at once", async ({ page }) => {
  await newSet(page, "SPL");
  await page.fill('input[name="mana_cost"]', "{1}{R}");
  await page.fill('input[name="name"]', "Left Half");
  await page.click(".add-half");
  await page.locator('.face-tab:has-text("2nd half")').click();
  await page.fill('input[name="name"]', "Right Half");
  await page.fill('input[name="mana_cost"]', "{2}{U}");
  await page.locator('.face-tab:has-text("Front")').click();

  await option(page, "split").click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-split/);
  // Both halves are drawn, and neither gets the transform marker.
  await expect(page.locator(".preview .mtg-title")).toHaveText(["Left Half", "Right Half"]);
  await expect(page.locator(".preview .mtg-box")).toHaveCount(2);
  await expect(page.locator(".preview .mtg-dfc")).toHaveCount(0);
  await expect(page.locator(".preview .flip-card")).toHaveCount(0);
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.reload();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-split/);
  await expect(page.locator(".preview .mtg-title")).toHaveText(["Left Half", "Right Half"]);
});
