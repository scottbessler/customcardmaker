import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { registerUser, uniqueName } from "./session";

/** Everyone shares one gallery, so a test finds its own set by a name nobody else has. */
function uniqueTitle(prefix: string): string {
  return `${prefix} ${Math.random().toString(36).slice(2, 8)}`;
}

/** Runs the steps one at a time, in order (collector numbers follow creation order). */
function inOrder(steps: (() => Promise<unknown>)[]): Promise<unknown> {
  return steps.reduce<Promise<unknown>>((done, step) => done.then(step), Promise.resolve());
}

async function setWithCards(page, name: string, cards = ["Ash", "Bog", "Cove"]) {
  const set = await (await page.request.post("/api/sets", { data: { name, code: "PUB" } })).json();
  await inOrder(cards.map((card) => () => page.request.post(`/api/sets/${set.id}/cards`, { data: { name: card, mana_cost: "{1}{U}", type_line: "Creature — Elf", rules_text: "Flying" } })));
  return set as { id: string };
}

/** A visitor with no account and no cookies, on the same server. */
async function stranger(browser, page) {
  const context = await browser.newContext({ baseURL: new URL(page.url()).origin });
  return { visitor: await context.newPage(), close: () => context.close() };
}

test("a set is private until it is published, and publishing is one tick", async ({ page, browser }) => {
  const title = uniqueTitle("Showcase");
  await registerUser(page);
  const set = await setWithCards(page, title);
  await page.goto(`/sets/${set.id}`);

  const { visitor, close } = await stranger(browser, page);
  await visitor.goto("/gallery");
  await expect(visitor.getByText(title)).toHaveCount(0);
  await visitor.goto(`/gallery/${set.id}`);
  await expect(visitor.locator("h1")).toHaveText("Not found");

  await page.click(".set-settings summary");
  await expect(page.locator(".public-help")).toContainText("Anyone could then see");
  await page.check('input[name="public"]');
  await expect(page.locator(".public-badge")).toHaveText("Public");
  await expect(page.locator(".public-link")).toBeVisible();

  await visitor.goto("/gallery");
  await expect(visitor.locator(".gallery-tile", { hasText: title })).toBeVisible();
  await expect(visitor.locator(".gallery-tile", { hasText: title }).locator(".mtg-card")).toHaveCount(1);
  await visitor.locator(".gallery-tile", { hasText: title }).click();
  await expect(visitor.locator("h1")).toHaveText(title);
  await expect(visitor.locator(".public-card .mtg-card")).toHaveCount(3);
  await expect(visitor.locator(".public-card .mtg-title")).toHaveText(["Ash", "Bog", "Cove"]);
  await expect(visitor.locator(".sign-out"), "a stranger is not signed in").toHaveCount(0);
  await close();
});

test("a set can be taken down again, and is gone from the gallery at once", async ({ page, browser }) => {
  const title = uniqueTitle("Brief");
  await registerUser(page);
  const set = await setWithCards(page, title);
  await page.goto(`/sets/${set.id}`);
  await page.click(".set-settings summary");
  await page.check('input[name="public"]');
  await expect(page.locator(".public-badge")).toBeVisible();

  const { visitor, close } = await stranger(browser, page);
  await visitor.goto(`/gallery/${set.id}`);
  await expect(visitor.locator("h1")).toHaveText(title);

  await page.uncheck('input[name="public"]');
  await expect(page.locator(".public-badge")).toHaveCount(0);
  await visitor.reload();
  await expect(visitor.locator("h1")).toHaveText("Not found");
  await visitor.goto("/gallery");
  await expect(visitor.getByText(title)).toHaveCount(0);
  await close();
});

test("the sets list says which sets are public", async ({ page }) => {
  await registerUser(page);
  const title = uniqueTitle("Listed");
  const set = await setWithCards(page, title);
  await page.request.patch(`/api/sets/${set.id}`, { data: { public: true } });
  await page.goto("/");
  await expect(page.locator(".set-card", { hasText: title })).toContainText("Public");
});

test("the gallery shows a display name and never the login name", async ({ page, browser }) => {
  const login = uniqueName("login");
  await page.goto("/");
  await page.fill('#register-form input[name="username"]', login);
  await page.fill('#register-form input[name="display_name"]', "Mx. Showcase");
  await page.click('#register-form button[type="submit"]');
  await expect(page.locator(".sets-page")).toBeVisible();
  const title = uniqueTitle("Credit");
  const set = await setWithCards(page, title);
  await page.request.patch(`/api/sets/${set.id}`, { data: { public: true } });

  const { visitor, close } = await stranger(browser, page);
  await visitor.goto("/gallery");
  await expect(visitor.locator(".gallery-tile", { hasText: title })).toContainText("by Mx. Showcase");
  await visitor.goto(`/gallery/${set.id}`);
  await expect(visitor.locator(".set-meta")).toContainText("by Mx. Showcase");
  expect(await visitor.content(), "the login name must not appear on a public page").not.toContain(login);
  await close();
});

test("the gallery is one click from every page", async ({ page }) => {
  await page.goto("/");
  await page.click(".site-nav a");
  await expect(page).toHaveURL(/\/gallery$/);
  await expect(page.locator("h1")).toHaveText("Gallery");
});

test("a public set holds its layout and fits a phone", async ({ page, browser }) => {
  // Fixed short names: on a phone a longer or random title wraps and shifts the rest.
  await registerUser(page);
  const set = await setWithCards(page, "Public set");
  await page.request.patch(`/api/sets/${set.id}`, { data: { public: true } });
  const { visitor, close } = await stranger(browser, page);
  await visitor.setViewportSize(page.viewportSize()!);
  await visitor.goto(`/gallery/${set.id}`);
  await expect(visitor.locator(".public-card .mtg-card")).toHaveCount(3);
  const overflow = await visitor.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow, "the page must not scroll sideways").toBeLessThanOrEqual(0);
  await expectLayout(visitor, "public-set", [".site-header", ".brand", ".site-nav", ".back", "h1", ".set-meta", ".public-cards", ".public-card"]);
  await close();
});

// A real 1x1 PNG; the server only checks its signature.
const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==", "base64");

test("a stranger can load a published card's art, and not before", async ({ page, browser }) => {
  await registerUser(page);
  const title = uniqueTitle("Pictured");
  const set = await (await page.request.post("/api/sets", { data: { name: title, code: "PIC" } })).json();
  const art = await (await page.request.post("/api/art", { data: PNG, headers: { "content-type": "image/png" } })).json();
  await page.request.post(`/api/sets/${set.id}/cards`, { data: { name: "With art", art: art.id } });

  const { visitor, close } = await stranger(browser, page);
  expect((await visitor.request.get(`/api/art/${art.id}`)).status(), "private art is not there for a stranger").toBe(404);

  await page.request.patch(`/api/sets/${set.id}`, { data: { public: true } });
  await visitor.goto(`/gallery/${set.id}`);
  await expect(visitor.locator(".public-card .mtg-art")).toHaveClass(/has-art/);
  const served = await visitor.request.get(`/api/art/${art.id}`);
  expect(served.status()).toBe(200);
  expect(served.headers()["x-content-type-options"]).toBe("nosniff");

  await page.request.patch(`/api/sets/${set.id}`, { data: { public: false } });
  expect((await visitor.request.get(`/api/art/${art.id}`)).status(), "taking the set down hides its art again").toBe(404);
  await close();
});

test("visitors can search the gallery and clear the search", async ({ page, browser }) => {
  await registerUser(page);
  const title = uniqueTitle("Findable");
  const set = await setWithCards(page, title);
  await page.request.patch(`/api/sets/${set.id}`, { data: { public: true } });
  const { visitor, close } = await stranger(browser, page);
  await visitor.goto("/gallery");
  const search = visitor.getByRole("searchbox", { name: "Search public sets" });
  await search.fill(title.toUpperCase());
  await visitor.getByRole("button", { name: "Search", exact: true }).click();
  await expect(visitor.locator(".gallery-tile")).toHaveCount(1);
  await expect(visitor.locator(".gallery-tile")).toContainText(title);

  await search.fill(`${title} missing`);
  await search.press("Enter");
  await expect(visitor.getByRole("status")).toHaveText("No public sets match your search.");
  await visitor.getByRole("link", { name: "Clear search" }).click();
  await expect(visitor).toHaveURL(/\/gallery$/);
  await expect(search).toHaveValue("");
  await close();
});
