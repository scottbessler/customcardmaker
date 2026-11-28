import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function openCard(page) {
  await registerUser(page);
  const set = await (await page.request.post("/api/sets", { data: { name: "AI tools", code: "AIT" } })).json();
  await page.request.post(`/api/sets/${set.id}/cards`, { data: { name: "Draft", rules_text: "{T}: draw a card" } });
  await page.goto(`/sets/${set.id}`);
}

test("rules cleanup previews wording and only applies it when accepted", async ({ page }) => {
  await openCard(page);
  const rules = page.locator('textarea[name="rules_text"]');
  const suggest = page.getByRole("button", { name: "Suggest cleanup" });
  await suggest.click();
  const review = page.locator(".ai-review");
  await expect(review).toContainText("{T}: Draw a card.");
  await expect(rules).toHaveValue("{T}: draw a card");

  await review.getByRole("button", { name: "Discard" }).click();
  await expect(rules).toHaveValue("{T}: draw a card");
  await suggest.click();
  await review.getByRole("button", { name: "Apply wording" }).click();
  await expect(rules).toHaveValue("{T}: Draw a card.");
  await expect(page.locator(".save-state")).toHaveText("Saved");
  await page.reload();
  await expect(rules).toHaveValue("{T}: Draw a card.");
});

test("preview text cleanup rejects a suggestion that changes a symbol", async ({ page }) => {
  await openCard(page);
  await page.locator(".preview").getByRole("button", { name: "Edit rules text and flavor text" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Rules text").fill("{T}: add mana");
  await dialog.getByRole("button", { name: "Suggest cleanup" }).click();
  await expect(dialog.getByRole("status")).toHaveText("The AI returned a suggestion that could not be used.");
  await expect(dialog.getByLabel("Rules text")).toHaveValue("{T}: add mana");
});

const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

test("extended artwork is reviewed before it replaces the original", async ({ page, browser }) => {
  await openCard(page);
  await page.locator(".preview .card-art-control").click();
  await page.setInputFiles('input[name="art"]', { name: "art.png", mimeType: "image/png", buffer: PNG });
  const preview = page.locator(".preview .mtg-art");
  await expect(preview).toHaveCSS("background-image", /api\/art/);
  const original = (await preview.getAttribute("style"))!.match(/\/api\/art\/[0-9a-f-]+/)![0];
  await page.getByRole("button", { name: "Extend background" }).click();
  const proposed = page.getByRole("img", { name: "Extended art" });
  await expect(proposed).toBeVisible();
  const replacement = (await proposed.getAttribute("src"))!;
  expect(replacement).not.toBe(original);
  await expect(preview).toHaveCSS("background-image", new RegExp(original));
  await page.getByRole("button", { name: "Use extended art" }).click();
  await expect(preview).toHaveCSS("background-image", new RegExp(replacement));
  expect((await page.request.get(original)).ok()).toBe(true);

  const other = await browser.newContext({ baseURL: new URL(page.url()).origin });
  try {
    const visitor = await other.newPage();
    await registerUser(visitor, "other");
    const denied = await visitor.request.post("/api/ai/extend-art", { data: { id: original.split("/").at(-1), aspect: "16:9" } });
    expect(denied.status()).toBe(404);
  } finally {
    await other.close();
  }
});
