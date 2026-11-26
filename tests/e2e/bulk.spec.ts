import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { registerUser } from "./session";

// Short, fixed names, deliberately: the layout snapshot below is measured on a
// phone, where a longer or random set title wraps and shifts everything under it.
const NAMES = ["Ash", "Bog", "Cove", "Dune", "Elm"];

/** Runs the steps one at a time, in order. */
function inOrder(steps: (() => Promise<unknown>)[]): Promise<unknown> {
  return steps.reduce<Promise<unknown>>((done, step) => done.then(step), Promise.resolve());
}

/** An account with one set of five named cards, made through the API for speed. */
async function setWithCards(page, name = "Bulk") {
  await registerUser(page);
  const set = await (await page.request.post("/api/sets", { data: { name, code: "BLK" } })).json();
  // One after another: the order they are made in is their collector order.
  await inOrder(NAMES.map((card) => () => page.request.post(`/api/sets/${set.id}/cards`, { data: { name: card } })));
  await page.goto(`/sets/${set.id}`);
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(NAMES.length);
  return set;
}

async function otherSet(page, name: string, code: string) {
  return (await (await page.request.post("/api/sets", { data: { name, code } })).json()) as { id: string };
}

const strip = (page) => page.locator(".filmstrip .mtg-card");
const titles = (page) => page.locator(".filmstrip .mtg-title");
// Taps are made one at a time, as a person makes them: two at once race the re-render.
const pick = (page, ...indexes: number[]) => inOrder(indexes.map((i) => () => strip(page).nth(i).click()));
const asking = (page) => page.locator(".confirm-dialog[open]");

test("select mode picks cards instead of opening them", async ({ page }) => {
  await setWithCards(page);
  await expect(page.locator(".creator")).toBeVisible();
  await page.click(".select-cards");
  await expect(page.locator(".creator"), "the editor steps aside while selecting").toHaveCount(0);
  await expect(page.locator(".add-card")).toHaveCount(0);
  await expect(page.locator(".bulk-count")).toHaveText("0 of 5 cards selected");
  await expect(page.locator(".bulk-delete")).toBeDisabled();

  await pick(page, 0, 2);
  await expect(page.locator(".bulk-count")).toHaveText("2 of 5 cards selected");
  await expect(strip(page).nth(0)).toHaveAttribute("aria-pressed", "true");
  await expect(strip(page).nth(1)).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator(".strip-item.picked")).toHaveCount(2);

  await pick(page, 0);
  await expect(page.locator(".bulk-count")).toHaveText("1 of 5 cards selected");

  await page.click(".bulk-all");
  await expect(page.locator(".bulk-count")).toHaveText("5 of 5 cards selected");
  await expect(page.locator(".bulk-all")).toBeDisabled();
  await page.click(".bulk-none");
  await expect(page.locator(".bulk-count")).toHaveText("0 of 5 cards selected");
});

test("Done and Escape both leave select mode", async ({ page }) => {
  await setWithCards(page);
  await page.click(".select-cards");
  await pick(page, 1);
  await page.click(".select-cards");
  await expect(page.locator(".creator")).toBeVisible();
  await expect(page.locator(".bulk-bar")).toHaveCount(0);

  await page.click(".select-cards");
  await pick(page, 1);
  await page.keyboard.press("Escape");
  await expect(page.locator(".bulk-bar")).toHaveCount(0);
  // Coming back in starts from nothing picked.
  await page.click(".select-cards");
  await expect(page.locator(".bulk-count")).toHaveText("0 of 5 cards selected");
});

test("deleting several cards asks first, then renumbers the rest", async ({ page }) => {
  await setWithCards(page);
  await page.click(".select-cards");
  await pick(page, 1, 3);
  await page.click(".bulk-delete");
  await expect(asking(page)).toContainText("Delete 2 cards?");
  await asking(page).locator(".confirm-cancel").click();
  await expect(strip(page)).toHaveCount(5);

  await page.click(".bulk-delete");
  await asking(page).locator(".confirm-accept").click();
  await expect(strip(page)).toHaveCount(3);
  await expect(titles(page)).toHaveText(["Ash", "Cove", "Elm"]);
  await expect(page.locator(".filmstrip .mtg-foot span").filter({ hasText: "BLK" })).toHaveText(["001/003 • BLK", "002/003 • BLK", "003/003 • BLK"]);
  await expect(page.locator(".notice")).toHaveText("Deleted 2 cards.");
  await expect(page.locator(".creator"), "back to editing").toBeVisible();

  await page.reload();
  await expect(titles(page)).toHaveText(["Ash", "Cove", "Elm"]);
});

test("cards move to another set, in order, and leave no gap behind", async ({ page }) => {
  const from = await setWithCards(page);
  const to = await otherSet(page, "Elsewhere", "ELS");
  await page.request.post(`/api/sets/${to.id}/cards`, { data: { name: "Zed" } });
  await page.reload();

  await page.click(".select-cards");
  await pick(page, 3, 0);
  await page.click(".bulk-move");
  const dialog = asking(page);
  await expect(dialog).toContainText("Move 2 cards");
  await expect(dialog.locator("select option"), "the set you are in is not offered as a place to move to").toHaveText(["ELS · Elsewhere", "A new set…"]);
  await dialog.locator("select").selectOption(to.id);
  await dialog.locator(".transfer-accept").click();

  await expect(page.locator(".notice")).toContainText("Moved 2 cards to Elsewhere");
  await expect(titles(page)).toHaveText(["Bog", "Cove", "Elm"]);
  await expect(page.locator(".filmstrip .mtg-foot span").filter({ hasText: "BLK" })).toHaveText(["001/003 • BLK", "002/003 • BLK", "003/003 • BLK"]);

  await page.locator(".notice a").click();
  await expect(page).toHaveURL(new RegExp(`/sets/${to.id}$`));
  await expect(titles(page)).toHaveText(["Zed", "Ash", "Dune"]);
  expect(from.id).not.toBe(to.id);
});

test("cards can be copied, and the originals stay", async ({ page }) => {
  await setWithCards(page);
  const to = await otherSet(page, "Copies", "CPY");
  await page.reload();
  await page.click(".select-cards");
  await pick(page, 0, 1);
  await page.click(".bulk-copy");
  await expect(asking(page)).toContainText("Copy 2 cards");
  await asking(page).locator("select").selectOption(to.id);
  await asking(page).locator(".transfer-accept").click();
  await expect(page.locator(".notice")).toContainText("Copied 2 cards to Copies");
  await expect(titles(page)).toHaveText(NAMES);
  await page.locator(".notice a").click();
  await expect(titles(page)).toHaveText(["Ash", "Bog"]);
});

test("cards can be moved into a set made on the spot", async ({ page }) => {
  await setWithCards(page);
  await page.click(".select-cards");
  await pick(page, 4);
  await page.click(".bulk-move");
  const dialog = asking(page);
  // No other sets exist, so making one is the only choice.
  await expect(dialog.locator('input[name="new-name"]')).toBeVisible();
  await dialog.locator('input[name="new-name"]').fill("Spare");
  await dialog.locator('input[name="new-code"]').fill("spr");
  await dialog.locator(".transfer-accept").click();
  await expect(page.locator(".notice")).toContainText("Moved 1 card to Spare");
  await expect(titles(page)).toHaveText(["Ash", "Bog", "Cove", "Dune"]);
  await page.locator(".notice a").click();
  await expect(page.locator("h1")).toHaveText("Spare");
  await expect(page.locator(".set-code")).toHaveText("SPR");
  await expect(titles(page)).toHaveText(["Elm"]);
});

test("a problem is shown in the dialog, which stays open to try again", async ({ page }) => {
  await setWithCards(page);
  await page.click(".select-cards");
  await pick(page, 0);
  await page.click(".bulk-move");
  const dialog = asking(page);
  await dialog.locator('input[name="new-name"]').fill("Spare");
  await dialog.locator('input[name="new-code"]').fill("abc!");
  await dialog.locator(".transfer-accept").click();
  await expect(dialog.locator(".error")).toContainText("A set code is 3 to 5 letters or digits");
  await expect(dialog).toBeVisible();
  await expect(strip(page), "nothing moved").toHaveCount(5);
  await dialog.locator('input[name="new-code"]').fill("abc");
  await dialog.locator(".transfer-accept").click();
  await expect(page.locator(".notice")).toContainText("Moved 1 card to Spare");
});

test("the bulk bar fits a phone and holds its layout", async ({ page }) => {
  await setWithCards(page);
  await page.click(".select-cards");
  await pick(page, 0, 1);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow, "the page must not scroll sideways").toBeLessThanOrEqual(0);
  await page.evaluate(() => window.scrollTo(0, 0));
  await expectLayout(page, "bulk-bar", [".site-header", ".set-bar", ".bulk-bar", ".bulk-count", ".bulk-actions", ".bulk-move", ".bulk-delete", ".filmstrip", ".strip-item.picked"]);
});
