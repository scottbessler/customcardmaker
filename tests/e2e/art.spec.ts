import { expect, test } from "./fixtures";
import { registerUser } from "./session";

// A real 1x1 red PNG. The browser decodes it and re-encodes it as a JPEG before
// it is sent, so this exercises the whole path, not just the endpoint.
const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

async function setWithCard(page) {
  await registerUser(page);
  await page.fill('.new-set input[name="name"]', `Art set ${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`);
  await page.fill('.new-set input[name="code"]', "ART");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
  await page.click(".preview .card-art-control");
}

async function upload(page) {
  await Promise.all([
    page.waitForResponse((r) => r.url().endsWith("/api/art") && r.request().method() === "POST" && r.ok()),
    page.setInputFiles('input[name="art"]', { name: "red.png", mimeType: "image/png", buffer: PNG }),
  ]);
}

/** The id out of `background-image:url(/api/art/<id>)`. */
async function artId(page, selector: string): Promise<string> {
  const style = (await page.locator(selector).getAttribute("style")) ?? "";
  const match = style.match(/\/api\/art\/([0-9a-f-]{36})/);
  expect(match, `${selector} should show uploaded art`).not.toBeNull();
  return match![1];
}

test("uploaded art shows on the card, in the strip, and survives a reload", async ({ page }) => {
  await setWithCard(page);
  await expect(page.locator(".preview .mtg-art")).not.toHaveClass(/has-art/);
  await expect(page.locator(".remove-art")).toBeDisabled();

  await upload(page);
  await expect(page.locator(".preview .mtg-art")).toHaveClass(/has-art/);
  await expect(page.locator(".filmstrip .mtg-art")).toHaveClass(/has-art/);
  await expect(page.locator(".art-thumb")).toHaveAttribute("style", /\/api\/art\//);
  await expect(page.locator(".file-button")).toContainText("Replace image");
  await expect(page.locator(".save-state")).toHaveText("Saved");

  await page.reload();
  await expect(page.locator(".preview .mtg-art")).toHaveClass(/has-art/);
  const id = await artId(page, ".preview .mtg-art");

  // What was stored is the shrunk JPEG, served as what it is.
  const served = await page.request.get(`/api/art/${id}`);
  expect(served.status()).toBe(200);
  expect(served.headers()["content-type"]).toBe("image/jpeg");
  expect(served.headers()["x-content-type-options"]).toBe("nosniff");
});

test("art can be removed from a card", async ({ page }) => {
  await setWithCard(page);
  await upload(page);
  await expect(page.locator(".preview .mtg-art")).toHaveClass(/has-art/);
  await page.click(".remove-art");
  await expect(page.locator(".preview .mtg-art")).not.toHaveClass(/has-art/);
  await expect(page.locator(".filmstrip .mtg-art")).not.toHaveClass(/has-art/);
  await expect(page.locator(".remove-art")).toBeDisabled();
  await expect(page.locator(".save-state")).toHaveText("Saved");
  await page.reload();
  await expect(page.locator(".preview .mtg-art")).not.toHaveClass(/has-art/);
});

test("something that is not an image is turned away with a reason", async ({ page }) => {
  await setWithCard(page);
  await page.setInputFiles('input[name="art"]', { name: "notes.txt", mimeType: "text/plain", buffer: Buffer.from("hello") });
  await expect(page.locator(".art-error")).toContainText("Choose an image");
  await expect(page.locator(".preview .mtg-art")).not.toHaveClass(/has-art/);
});

test("another account cannot fetch your art", async ({ page, browser }) => {
  await setWithCard(page);
  await upload(page);
  await expect(page.locator(".preview .mtg-art")).toHaveClass(/has-art/);
  const id = await artId(page, ".preview .mtg-art");

  const baseURL = new URL(page.url()).origin;
  const other = await (await browser.newContext({ baseURL })).newPage();
  await registerUser(other, "nosy");
  expect((await other.request.get(`/api/art/${id}`)).status()).toBe(404);
  await other.context().close();

  const stranger = await browser.newContext({ baseURL });
  // A stranger is told no more than another account: it is simply not there.
  expect((await stranger.request.get(`/api/art/${id}`)).status()).toBe(404);
  await stranger.close();
});
