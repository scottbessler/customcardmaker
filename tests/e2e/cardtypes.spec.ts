import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function newCard(page) {
  await registerUser(page, "typer");
  await page.fill('.new-set input[name="name"]', "Features");
  await page.fill('.new-set input[name="code"]', "FTR");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
}

const feature = (page, id: string) => page.locator(`input[name="feature-${id}"]`);

test("the features a card has say which fields it has and which it must have", async ({ page }) => {
  await newCard(page);
  const missing = page.locator(".missing");
  // A new card has a mana cost: a name and a type line are what it needs.
  await expect(page.locator('input[name="mana_cost"]')).toBeVisible();
  await expect(missing).toHaveText("To finish this card: name, type line.");
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await page.fill('input[name="name"]', "Bolt");
  await page.fill('input[name="type_line"]', "Instant");
  await expect(missing).toHaveText("Every required field is filled in.");

  // Power/toughness is another thing to fill in once it is on; the shared fields stay.
  await page.click(".edit-template");
  await feature(page, "power_toughness").check();
  await page.click(".apply-template");
  await expect(missing).toHaveText("To finish this card: power / toughness.");
  await expect(page.locator('input[name="name"]')).toHaveValue("Bolt");
  await page.fill('input[name="power_toughness"]', "2/2");
  await expect(missing).toHaveText("Every required field is filled in.");

  // Features combine: a creature planeswalker needs both stats.
  await page.click(".edit-template");
  await feature(page, "loyalty").check();
  await page.click(".apply-template");
  await expect(missing).toHaveText("To finish this card: loyalty.");
  await page.fill('input[name="loyalty"]', "3");

  // Turning a feature off drops its field with it.
  await page.click(".edit-template");
  await feature(page, "power_toughness").uncheck();
  await page.click(".apply-template");
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await page.fill('textarea[name="rules_text"]', "+1: Draw a card");
  await page.fill('input[name="mana_cost"]', "{2}{R}");
  await page.click(".edit-template");
  await feature(page, "cost").uncheck();
  await page.click(".apply-template");
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
  await expect(page.locator('textarea[name="rules_text"]')).toHaveValue("+1: Draw a card");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  // It is saved with those features, and the fields come back as they allow.
  await page.reload();
  await page.click(".edit-template");
  await expect(feature(page, "loyalty")).toBeChecked();
  await expect(feature(page, "cost")).not.toBeChecked();
  await expect(feature(page, "power_toughness")).not.toBeChecked();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.locator('input[name="name"]')).toHaveValue("Bolt");
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
});
