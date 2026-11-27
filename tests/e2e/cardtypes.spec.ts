import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function newCard(page) {
  await registerUser(page, "typer");
  await page.fill('.new-set input[name="name"]', "Features");
  await page.fill('.new-set input[name="code"]', "FTR");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
}

const feature = (page, id: string) => page.locator(`input[name="feature-${id}"]`);

test("the features a card has say which fields it has and which it must have", async ({ page }) => {
  await newCard(page);
  const missing = page.locator(".missing");
  // A new card has a mana cost: a name and a type line are what it needs.
  await expect(feature(page, "cost")).toBeChecked();
  await expect(missing).toHaveText("To finish this card: name, type line.");
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await page.fill('input[name="name"]', "Bolt");
  await page.fill('input[name="type_line"]', "Instant");
  await expect(missing).toHaveText("Every required field is filled in.");

  // Power/toughness is another thing to fill in once it is on; the shared fields stay.
  await feature(page, "power_toughness").check();
  await expect(missing).toHaveText("To finish this card: power / toughness.");
  await expect(page.locator('input[name="name"]')).toHaveValue("Bolt");
  await page.fill('input[name="power_toughness"]', "2/2");
  await expect(missing).toHaveText("Every required field is filled in.");

  // Features combine: a creature planeswalker needs both stats.
  await feature(page, "loyalty").check();
  await expect(missing).toHaveText("To finish this card: loyalty.");
  await page.fill('input[name="loyalty"]', "3");

  // Turning a feature off drops its field with it.
  await feature(page, "power_toughness").uncheck();
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await page.fill('textarea[name="rules_text"]', "+1: Draw a card");
  await page.fill('input[name="mana_cost"]', "{2}{R}");
  await feature(page, "cost").uncheck();
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
  await expect(page.locator('textarea[name="rules_text"]')).toHaveValue("+1: Draw a card");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  // It is saved with those features, and the fields come back as they allow.
  await page.reload();
  await expect(feature(page, "loyalty")).toBeChecked();
  await expect(feature(page, "cost")).not.toBeChecked();
  await expect(feature(page, "power_toughness")).not.toBeChecked();
  await expect(page.locator('input[name="name"]')).toHaveValue("Bolt");
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
});

test("every layout is offered, and one the card cannot wear says why", async ({ page }) => {
  await newCard(page);
  const option = (id: string) => page.locator(`.look-option:has(input[name="template"][value="${id}"])`);
  await expect(page.locator(".look-option")).toHaveCount(15);
  // A card with a cost can be the look or a saga; not a token, and not a battle without a defense.
  await expect(option("saga").locator("input")).toBeEnabled();
  await expect(option("token").locator("input")).toBeDisabled();
  await expect(option("token").locator(".look-needs")).toHaveText("not with mana cost");
  await expect(option("battle").locator("input")).toBeDisabled();

  // A feature a chosen template cannot draw puts the card back on the set's look.
  await feature(page, "power_toughness").check();
  await expect(option("saga").locator("input")).toBeDisabled();
  await feature(page, "power_toughness").uncheck();
  await option("saga").click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-saga/);
  await feature(page, "loyalty").check();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-planeswalker/);
  await expect(page.locator(".look-option.chosen")).toContainText("Modern");

  // The set's default can only be a look every card can wear.
  await page.locator(".set-settings summary").click();
  await expect(page.locator('select[name="default-template"] option')).toHaveText(["Modern"]);
});
