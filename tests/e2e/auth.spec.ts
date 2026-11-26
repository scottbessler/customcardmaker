import { expect, test } from "./fixtures";
import { registerUser, uniqueName } from "./session";

test("a visitor is offered an account, not a list of sets", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("#register-form")).toBeVisible();
  await expect(page.locator("#login-form")).toBeVisible();
  await expect(page.locator(".sets-page")).toHaveCount(0);
  await expect(page.locator(".sign-out")).toHaveCount(0);
});

test("registering signs you in, and the session survives a reload", async ({ page }) => {
  const name = uniqueName("grace");
  await page.goto("/");
  await page.fill('#register-form input[name="username"]', name);
  await page.fill('#register-form input[name="display_name"]', "Grace H.");
  await page.click('#register-form button[type="submit"]');
  await expect(page.locator(".who")).toHaveText("Grace H.");
  await page.reload();
  await expect(page.locator(".who")).toHaveText("Grace H.");
  await expect(page.locator(".sets-page")).toBeVisible();
});

test("signing out and back in returns to the same account", async ({ page }) => {
  const name = await registerUser(page, "alan");
  await page.click(".sign-out button");
  await expect(page.locator("#login-form")).toBeVisible();
  await page.fill('#login-form input[name="username"]', name.toUpperCase());
  await page.click('#login-form button[type="submit"]');
  await expect(page.locator(".sets-page")).toBeVisible();
  await expect(page.locator(".who")).toHaveText(name);
});

test("a taken or unknown name says so", async ({ page }) => {
  const name = await registerUser(page, "kay");
  await page.click(".sign-out button");

  await page.fill('#register-form input[name="username"]', name);
  await page.click('#register-form button[type="submit"]');
  await expect(page.locator("#auth-error")).toHaveText("That username is already taken.");

  await page.fill('#login-form input[name="username"]', uniqueName("nobody"));
  await page.click('#login-form button[type="submit"]');
  await expect(page.locator("#auth-error")).toHaveText("No account has that username.");
});

test("a set belongs to its owner alone", async ({ page, browser }) => {
  await registerUser(page, "owner");
  await page.fill('.new-set input[name="name"]', "Private");
  await page.fill('.new-set input[name="code"]', "PRV");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toHaveText("Private");
  const url = page.url();

  // Separate contexts share no cookies, so each is its own browser.
  const baseURL = new URL(url).origin;
  const other = await (await browser.newContext({ baseURL })).newPage();
  await registerUser(other, "nosy");
  await other.goto(url);
  await expect(other.locator("h1")).toHaveText("Not found");
  await expect(other.locator("h1")).not.toHaveText("Private");

  const stranger = await (await browser.newContext({ baseURL })).newPage();
  await stranger.goto(url);
  await expect(stranger.locator("#register-form")).toBeVisible();
  await other.context().close();
  await stranger.context().close();
});
