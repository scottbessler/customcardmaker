import { expect, type Page } from "@playwright/test";

let accounts = 0;

/** A username no other run, project or worker has used: they all share one server. */
export function uniqueName(label: string): string {
  accounts += 1;
  return `${label}${Date.now().toString(36)}${accounts}${Math.random().toString(36).slice(2, 6)}`.slice(0, 32);
}

/** Creates an account and leaves the page signed in on the sets list. */
export async function registerUser(page: Page, label = "ada"): Promise<string> {
  const username = uniqueName(label);
  await page.goto("/");
  await page.fill('#register-form input[name="username"]', username);
  await page.click('#register-form button[type="submit"]');
  await expect(page.locator(".sets-page")).toBeVisible();
  return username;
}
