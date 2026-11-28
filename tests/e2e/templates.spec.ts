import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function newSet(page, code: string, add = true) {
  await registerUser(page);
  await page.fill('.new-set input[name="name"]', `Looks ${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`);
  await page.fill('.new-set input[name="code"]', code);
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  if (!add) return;
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
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
  await page.click(".edit-template");
  // Every layout is listed; the card can wear the look or be a saga.
  await expect(page.locator(".look-option")).toHaveCount(15);
  await expect(option(page, "saga").locator("input")).toBeEnabled();
  await expect(option(page, "saga").locator(".look-thumb .mtg-card")).toHaveClass(/lay-saga/);

  await option(page, "saga").click();
  await page.click(".apply-template");
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

test("setup adds and locks required features while switching preserves shared fields", async ({ page }) => {
  await newSet(page, "NEED");
  await page.fill('input[name="name"]', "Siege");
  await page.fill('input[name="mana_cost"]', "{R}");
  await page.click(".edit-template");
  await option(page, "battle").click();
  const defense = page.locator('input[name="feature-defense"]');
  await expect(defense).toBeChecked();
  await expect(defense).toBeDisabled();
  await expect(page.locator('input[name="feature-power_toughness"]')).toBeDisabled();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-modern/);

  await page.click(".edit-template");
  await option(page, "battle").click();
  await page.click(".apply-template");
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-battle/);
  await expect(page.locator('input[name="name"]')).toHaveValue("Siege");
  await expect(page.locator('input[name="mana_cost"]')).toHaveValue("{R}");
  await page.locator(".preview").getByRole("button", { name: "Edit defense" }).click();
  await page.locator(".preview").getByLabel("Defense", { exact: true }).fill("5");
  await page.locator(".preview").getByLabel("Defense", { exact: true }).press("Enter");
  await expect(page.locator(".preview .mtg-pt")).toHaveText("5");
});

test("the set has one look, and a card that names its own layout keeps it", async ({ page }) => {
  await newSet(page, "DEF");
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(2);
  await expect(page.locator(".filmstrip .mtg-card.lay-modern")).toHaveCount(2);

  // The set's default is the one look, and only a look may be chosen.
  await page.locator(".set-settings summary").click();
  await expect(page.locator('select[name="default-template"] option')).toHaveText(["Modern"]);

  // A card that names a layout is drawn by it, and stays that way across a reload.
  await page.locator(".filmstrip .mtg-card").first().click();
  await page.click(".edit-template");
  await option(page, "saga").click();
  await page.click(".apply-template");
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
  await page.click(".edit-template");
  await option(page, "split").click();
  await expect(page.locator('input[name="feature-half"]')).toBeChecked();
  await expect(page.locator('input[name="feature-half"]')).toBeDisabled();
  await page.click(".apply-template");
  await page.getByRole("button", { name: "Edit second half art" }).click();
  await expect(page.getByRole("dialog", { name: "Card art" })).toBeVisible();
  await page.getByRole("button", { name: "Done", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Card art" })).toBeHidden();
  await expect(page.locator(".editing .face-tab.chosen")).toHaveText("2nd half");
  await page.locator('.editing .face-tab:has-text("Front")').click();
  await page.locator(".preview .mtg-name[data-part-face=half]").getByRole("button").click();
  const half = page.locator(".preview .mtg-name[data-part-face=half]").getByLabel("Name", { exact: true });
  await half.fill("Right Half");
  await half.press("Enter");
  await expect(page.locator(".preview .mtg-title").last()).toHaveText("Right Half");
  await page.fill('input[name="mana_cost"]', "{2}{U}");
  await page.locator('.face-tab:has-text("Front")').click();
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

test("creation waits for setup and starts with the selected template", async ({ page }) => {
  await newSet(page, "NEW", false);
  await page.click(".add-card");
  await option(page, "battle").click();
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(0);
  await expect(page.locator(".card-form")).toBeHidden();
  await page.getByRole("button", { name: "Start editing" }).click();
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-battle/);
  await expect(page.locator('input[name="defense"]')).toBeVisible();
});

test("a double-faced card can have a typed spell inset on each face", async ({ page }) => {
  await newSet(page, "INS", false);
  await page.click(".add-card");
  await page.check('input[name="feature-back"]');
  const setup = page.getByRole("dialog");
  await option(page, "omen").click();
  await setup.getByRole("button", { name: "Omen spell", exact: true }).click();
  await expect(setup.getByLabel("Half type")).toHaveValue("Instant — Omen");
  await expect(setup.locator(".look-option")).toHaveCount(0);
  await setup.getByLabel("Half type").selectOption("Sorcery — Omen");
  await setup.getByRole("button", { name: "Back", exact: true }).click();
  await option(page, "prepared").click();
  await setup.getByRole("button", { name: "Back Prepared spell", exact: true }).click();
  await expect(setup.getByLabel("Half type")).toHaveValue("Instant");
  await setup.getByLabel("Half type").selectOption("Sorcery");
  await setup.getByRole("button", { name: "Start editing" }).click();

  const preview = page.locator(".preview .mtg-card");
  const spell = preview.locator(".mtg-type[data-part-face=half]");
  await expect(preview).toHaveClass(/lay-omen/);
  await expect(spell).toHaveText("Sorcery — Omen");
  await expect(preview.locator(".mtg-dfc")).toHaveCount(1);
  const omenBounds = (await spell.boundingBox())!;
  const frontBounds = (await preview.boundingBox())!;
  expect(omenBounds.x).toBeLessThan(frontBounds.x + frontBounds.width / 2);
  await page.click(".flip-card");
  await expect(preview).toHaveClass(/lay-prepared/);
  await expect(spell).toHaveText("Sorcery");
  await expect(preview.locator(".mtg-dfc")).toHaveClass(/dfc-back/);
  const prepBounds = (await spell.boundingBox())!;
  const backBounds = (await preview.boundingBox())!;
  expect(prepBounds.x).toBeGreaterThanOrEqual(backBounds.x + backBounds.width / 2 - 0.5);
});
