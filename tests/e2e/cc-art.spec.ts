import { expect, test } from "./fixtures";
import { registerUser } from "./session";

const MISTY = "Misty Forest";

async function cardInEditor(page) {
  await registerUser(page);
  const set = await (await page.request.post("/api/sets", { data: { name: "Free art", code: "FRE" } })).json();
  await page.request.post(`/api/sets/${set.id}/cards`, { data: { name: "Needs a picture" } });
  await page.goto(`/sets/${set.id}`);
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
  return set as { id: string };
}

const dialog = (page) => page.locator(".art-search[open]");
const results = (page) => dialog(page).locator(".art-result");

async function search(page, query: string) {
  await dialog(page).locator('input[name="art-query"]').fill(query);
  await dialog(page).locator(".art-search-go").click();
}

test("search the free art, pick one, and it arrives on the card with its credit", async ({ page }) => {
  await cardInEditor(page);
  await page.click(".search-art");
  await expect(dialog(page)).toBeVisible();
  await search(page, "forest");
  await expect(results(page)).toHaveCount(2);
  await expect(results(page).first()).toContainText(MISTY);
  await expect(results(page).first()).toContainText("Jane Painter");
  await expect(results(page).first()).toContainText("CC BY 2.0");
  await expect(results(page).nth(1)).toContainText("CC0 1.0");
  await expect(results(page).first().locator("img")).toHaveJSProperty("complete", true);

  await results(page).first().click();
  await expect(dialog(page)).toHaveCount(0);
  await expect(page.locator(".preview .mtg-art")).toHaveClass(/has-art/);
  await expect(page.locator(".filmstrip .mtg-art")).toHaveClass(/has-art/);
  // The artist line is filled in from the credit, and the licence is shown with the art.
  await expect(page.locator('input[name="artist"]')).toHaveValue("Jane Painter");
  await expect(page.locator(".preview .mtg-foot")).toContainText("Illus. Jane Painter");
  const credit = page.locator(".art-credit-line");
  await expect(credit).toContainText(`“${MISTY}” by Jane Painter`);
  await expect(credit.getByRole("link", { name: "CC BY 2.0" })).toHaveAttribute("href", "https://creativecommons.org/licenses/by/2.0/");
  await expect(credit.getByRole("link", { name: "source" })).toHaveAttribute("rel", /noopener/);
  await expect(page.locator(".save-state")).toHaveText("Saved");

  // And it is the card's own now: still there, with its credit, after a reload.
  await page.reload();
  await expect(page.locator(".preview .mtg-art")).toHaveClass(/has-art/);
  await expect(page.locator(".art-credit-line")).toContainText("Jane Painter");
});

test("an artist already on the card is not overwritten", async ({ page }) => {
  await cardInEditor(page);
  await page.fill('input[name="artist"]', "Someone Else");
  await page.click(".search-art");
  await search(page, "forest");
  await results(page).first().click();
  await expect(page.locator(".preview .mtg-art")).toHaveClass(/has-art/);
  await expect(page.locator('input[name="artist"]')).toHaveValue("Someone Else");
  await expect(page.locator(".art-credit-line")).toContainText("Jane Painter");
});

test("non-commercial licences appear only when asked for", async ({ page }) => {
  await cardInEditor(page);
  await page.click(".search-art");
  await search(page, "forest");
  await expect(results(page)).toHaveCount(2);
  await expect(dialog(page)).not.toContainText("Night Market");
  await dialog(page).locator('input[name="art-nc"]').check();
  await dialog(page).locator(".art-search-go").click();
  await expect(results(page)).toHaveCount(3);
  await expect(dialog(page)).toContainText("Night Market");
  await expect(dialog(page)).toContainText("CC BY-NC 2.0");
});

test("an empty or busy search says so, and the dialog stays usable", async ({ page }) => {
  await cardInEditor(page);
  await page.click(".search-art");
  await search(page, "nothing");
  await expect(dialog(page).locator(".art-empty")).toHaveText("Nothing matched. Try another word.");
  await search(page, "busy");
  await expect(dialog(page).locator(".error")).toContainText("busy");
  await search(page, "forest");
  await expect(results(page)).toHaveCount(2);
  await expect(dialog(page).locator(".error")).toBeEmpty();
});

test("Escape and Close leave the card alone", async ({ page }) => {
  await cardInEditor(page);
  await page.click(".search-art");
  await search(page, "forest");
  await page.keyboard.press("Escape");
  await expect(dialog(page)).toHaveCount(0);
  await page.click(".search-art");
  await dialog(page).locator(".art-search-close").click();
  await expect(dialog(page)).toHaveCount(0);
  await expect(page.locator(".preview .mtg-art")).not.toHaveClass(/has-art/);
});

test("borrowed art is credited on the public page", async ({ page, browser }) => {
  const set = await cardInEditor(page);
  await page.click(".search-art");
  await search(page, "forest");
  await results(page).nth(1).click();
  await expect(page.locator(".art-credit-line")).toContainText("Lee Gardner");
  await expect(page.locator(".save-state")).toHaveText("Saved");
  await page.request.patch(`/api/sets/${set.id}`, { data: { public: true } });

  const context = await browser.newContext({ baseURL: new URL(page.url()).origin });
  const visitor = await context.newPage();
  await visitor.goto(`/gallery/${set.id}`);
  const credits = visitor.locator(".art-credits");
  await expect(credits).toContainText("Needs a picture");
  await expect(credits).toContainText("“Pine Ridge” by Lee Gardner");
  await expect(credits.getByRole("link", { name: "CC0 1.0" })).toBeVisible();
  await expect(visitor.locator(".public-card .mtg-art")).toHaveClass(/has-art/);
  await context.close();
});

test("the dialog fits a phone", async ({ page }, info) => {
  test.skip(info.project.name !== "chromium-mobile");
  await cardInEditor(page);
  await page.click(".search-art");
  await search(page, "forest");
  await expect(results(page)).toHaveCount(2);
  const box = await dialog(page).boundingBox();
  const viewport = page.viewportSize()!;
  expect(box!.x).toBeGreaterThanOrEqual(0);
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport.width);
  const overflow = await dialog(page).evaluate((el) => el.scrollWidth - el.clientWidth);
  expect(overflow, "the dialog must not scroll sideways").toBeLessThanOrEqual(0);
});
