import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function newCard(page) {
  await registerUser(page);
  await page.fill('.new-set input[name="name"]', `Types ${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`);
  await page.fill('.new-set input[name="code"]', "TYP");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
}

const line = (page) => page.locator('input[name="type_line"]');
const supertype = (page, name: string) => page.locator(".supertype-pick", { hasText: name });
const type = (page, name: string) => page.locator(".type-pick", { hasText: name });
const subtype = (page) => page.locator('input[name="subtype"]');
const tokens = (page) => page.locator(".subtype-token");
const options = (page) => page.locator(".subtype-option");

test("supertype and type buttons write the type line", async ({ page }) => {
  await newCard(page);
  await supertype(page, "Legendary").click();
  await type(page, "Creature").click();
  await expect(line(page)).toHaveValue("Legendary Creature");
  await type(page, "Artifact").click();
  await expect(line(page)).toHaveValue("Legendary Creature Artifact");
  await expect(type(page, "Artifact")).toHaveAttribute("aria-pressed", "true");

  // Pressing a pressed button takes it back off.
  await type(page, "Creature").click();
  await supertype(page, "Legendary").click();
  await expect(line(page)).toHaveValue("Artifact");
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/frame-artifact/);
});

test("subtypes are suggested for the chosen type and added with Enter", async ({ page }) => {
  await newCard(page);
  await expect(subtype(page), "no type yet, nothing to be a subtype of").toBeDisabled();
  await type(page, "Creature").click();
  await expect(subtype(page)).toBeEnabled();

  await subtype(page).fill("elf");
  await expect(options(page).first()).toHaveText("Elf");
  await expect(subtype(page)).toHaveAttribute("aria-expanded", "true");
  await subtype(page).press("Enter");
  await expect(line(page)).toHaveValue("Creature — Elf");
  await expect(tokens(page)).toHaveText(["Elf×"]);

  await subtype(page).fill("druid");
  await subtype(page).press("ArrowDown");
  await subtype(page).press("Enter");
  await expect(line(page)).toHaveValue("Creature — Elf Druid");
  await expect(page.locator(".preview .mtg-type")).toContainText("Creature — Elf Druid");
});

test("suggestions follow the type, rank prefixes first, and skip what is already there", async ({ page }) => {
  await newCard(page);
  await type(page, "Artifact").click();
  await subtype(page).fill("equ");
  await expect(options(page).first()).toHaveText("Equipment");
  await subtype(page).fill("elf");
  await expect(options(page), "an artifact has no Elf subtype to offer").toHaveCount(0);

  await type(page, "Artifact").click();
  await type(page, "Creature").click();
  await subtype(page).fill("an");
  const names = await options(page).allTextContents();
  expect(names.length).toBeGreaterThan(1);
  expect(names[0].toLowerCase().startsWith("an"), "names that start with the query come first").toBe(true);
  await subtype(page).fill("angel");
  await subtype(page).press("Enter");
  await subtype(page).fill("angel");
  await expect(options(page), "Angel is already chosen").toHaveCount(0);
});

test("a subtype nobody listed can still be typed, and is capitalised", async ({ page }) => {
  await newCard(page);
  await type(page, "Creature").click();
  await subtype(page).fill("zorbfoo");
  await subtype(page).press("Enter");
  await expect(line(page)).toHaveValue("Creature — Zorbfoo");
  await expect(tokens(page)).toHaveText(["Zorbfoo×"]);
});

test("a subtype with a space in it stays one subtype", async ({ page }) => {
  await newCard(page);
  await line(page).fill("Creature — Time Lord Wizard");
  await expect(tokens(page)).toHaveText(["Time Lord×", "Wizard×"]);
  await tokens(page).first().getByRole("button").click();
  await expect(line(page)).toHaveValue("Creature — Wizard");
});

test("Backspace in the empty box removes the last subtype", async ({ page }) => {
  await newCard(page);
  await line(page).fill("Creature — Elf Druid");
  await subtype(page).focus();
  await subtype(page).press("Backspace");
  await expect(line(page)).toHaveValue("Creature — Elf");
  await subtype(page).press("Backspace");
  await expect(line(page)).toHaveValue("Creature");
});

test("typing the line by hand and using the buttons agree", async ({ page }) => {
  await newCard(page);
  await line(page).fill("Basic Land — Forest");
  await expect(supertype(page, "Basic")).toHaveAttribute("aria-pressed", "true");
  await expect(type(page, "Land")).toHaveAttribute("aria-pressed", "true");
  await expect(type(page, "Creature")).toHaveAttribute("aria-pressed", "false");
  await expect(tokens(page)).toHaveText(["Forest×"]);
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/frame-land/);

  // A word the buttons do not know survives being clicked around.
  await line(page).fill("Tribal Instant — Arcane");
  await type(page, "Instant").click();
  await expect(line(page)).toHaveValue("Tribal — Arcane");
});

test("the subtype box is a labelled combobox for assistive tech", async ({ page }) => {
  await newCard(page);
  await type(page, "Creature").click();
  const box = subtype(page);
  await expect(box).toHaveAttribute("role", "combobox");
  await expect(box).toHaveAttribute("aria-controls", "subtype-options");
  await box.fill("el");
  await box.press("ArrowDown");
  await expect(options(page).first()).toHaveAttribute("aria-selected", "true");
  const active = await box.getAttribute("aria-activedescendant");
  expect(active).toBe(await options(page).first().getAttribute("id"));
  await box.press("Escape");
  await expect(options(page)).toHaveCount(0);
});

test("supplemental card types can be picked and edited as text", async ({ page }) => {
  await newCard(page);
  await page.locator(".supplemental-types summary").click();
  const extras = page.getByRole("group", { name: "Supplemental card types" });
  await extras.getByRole("button", { name: "Scheme" }).click();
  await expect(line(page)).toHaveValue("Scheme");
  await line(page).fill("plane — Amonkhet");
  const plane = extras.getByRole("button", { name: "Plane", exact: true });
  await expect(plane).toHaveAttribute("aria-pressed", "true");
  await plane.click();
  await expect(line(page)).toHaveValue("Amonkhet");
});
