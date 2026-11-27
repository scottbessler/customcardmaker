import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function newCard(page) {
  await registerUser(page, "typer");
  await page.fill('.new-set input[name="name"]', "Types");
  await page.fill('.new-set input[name="code"]', "TYP");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
}

const pick = (page, label: string) => page.locator(".cardtypes .tint-option", { hasText: label }).click();

test("a card type says which fields a card has and which it must have", async ({ page }) => {
  await newCard(page);
  const missing = page.locator(".missing");
  // A new card is a spell: a name and a type line are what it needs.
  await expect(page.locator('.cardtypes input[name="cardtype"]:checked')).toHaveValue("spell");
  await expect(missing).toHaveText("To finish this card: name, type line.");
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await page.fill('input[name="name"]', "Bolt");
  await page.fill('input[name="type_line"]', "Instant");
  await expect(missing).toHaveText("Every required field is filled in.");

  // A creature also needs power/toughness; the shared fields come with it.
  await pick(page, "Creature");
  await expect(missing).toHaveText("To finish this card: power / toughness.");
  await expect(page.locator('input[name="name"]')).toHaveValue("Bolt");
  await page.fill('input[name="power_toughness"]', "2/2");
  await expect(missing).toHaveText("Every required field is filled in.");

  // A planeswalker has loyalty instead; the creature's stat is dropped with its field.
  await pick(page, "Planeswalker");
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await expect(missing).toHaveText("To finish this card: loyalty.");
  await page.fill('textarea[name="rules_text"]', "+1: Draw a card");

  // A token has no mana cost, and cannot be double-faced.
  await page.fill('input[name="mana_cost"]', "{2}{R}");
  await pick(page, "Token");
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
  await expect(page.locator(".add-back")).toHaveCount(0);
  await expect(page.locator('textarea[name="rules_text"]')).toHaveValue("+1: Draw a card");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  // It is saved as that type, and fields come back as the type lists them.
  await page.reload();
  await expect(page.locator('.cardtypes input[name="cardtype"]:checked')).toHaveValue("token");
  await expect(page.locator('input[name="name"]')).toHaveValue("Bolt");
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
});

test("a template is offered only for the types it works with", async ({ page }) => {
  await newCard(page);
  const looks = page.locator(".look-option");
  // A spell can be drawn as a saga; a creature cannot.
  await expect(looks).toHaveCount(5);
  await expect(looks.filter({ hasText: "Saga" })).toHaveCount(1);
  await pick(page, "Creature");
  await expect(looks).toHaveCount(4);
  await expect(looks.filter({ hasText: "Saga" })).toHaveCount(0);

  // Switching to a type the card's template cannot be used with puts it back on the set's.
  await pick(page, "Spell");
  await looks.filter({ hasText: "Saga" }).click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-saga/);
  await pick(page, "Creature");
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-normal/);
  await expect(page.locator(".look-option.chosen")).toContainText("Modern");

  // And the set's default can only be a look every type can wear.
  await page.locator(".set-settings summary").click();
  await expect(page.locator('select[name="default-template"] option')).toHaveText(["Modern", "Classic", "Borderless", "Full art"]);
});
