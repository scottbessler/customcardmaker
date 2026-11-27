import { expect, test } from "./fixtures";
import { registerUser } from "./session";

const XML = `<?xml version="1.0" encoding="UTF-8"?>
<cockatrice_carddatabase version="4">
  <cards>
    <card>
      <name>Test Bird</name>
      <text>Flying</text>
      <prop>
        <type>Creature — Bird</type>
        <manacost>{1}{W}</manacost>
        <pt>1/1</pt>
      </prop>
      <set rarity="rare">IMP</set>
    </card>
    <card>
      <name>Test Charm</name>
      <text>Draw a card.</text>
      <prop>
        <type>Instant</type>
        <manacost>{U}</manacost>
      </prop>
      <set rarity="common">IMP</set>
    </card>
  </cards>
</cockatrice_carddatabase>`;

test("a Cockatrice file is previewed, then becomes a set you can open", async ({ page }) => {
  await registerUser(page, "importer");
  await page.click('a[href="/import"]');
  await expect(page.locator("#import-form")).toBeVisible();

  await page.fill('#import-form input[name="set_code"]', "IMP");
  await page.fill('#import-form input[name="set_name"]', "Imported Test Set");
  await page.setInputFiles('#import-form input[name="file"]', {
    name: "cards.xml",
    mimeType: "text/xml",
    buffer: Buffer.from(XML),
  });
  // No picture URLs in this file, so nothing is downloaded.
  await page.uncheck('#import-form input[name="images"]');
  await page.click('#import-form button[type="submit"]');

  // The preview says what would be made, and nothing has been made yet.
  const preview = page.locator("#import-preview");
  await expect(preview).toBeVisible();
  await expect(preview).toContainText("Imported Test Set (IMP)");
  await expect(preview).toContainText("2 cards would be created");
  await expect(preview.locator(".import-cards li")).toHaveCount(2);
  await expect(preview.locator(".import-cards li").first()).toContainText("Test Bird");
  await expect(preview.locator(".import-cards li").first()).toContainText("Creature — Bird");
  await expect(page.locator("#import-result")).toBeHidden();

  await preview.locator(".import-confirm").click();
  await expect(page.locator("#import-result")).toContainText("Imported 2 cards");
  await expect(preview).toBeHidden();
  await page.click("#import-result a");
  await expect(page.locator("h1")).toHaveText("Imported Test Set");
  await expect(page.locator(".strip-item")).toHaveCount(2);
});

test("the import page asks a visitor to sign in first", async ({ page }) => {
  await page.goto("/import");
  await expect(page.locator("#register-form")).toBeVisible();
  await expect(page.locator("#import-form")).toHaveCount(0);
});
