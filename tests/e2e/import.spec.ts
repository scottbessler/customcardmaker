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

test("Preview waits for the import form to be ready", async ({ page }) => {
  await registerUser(page, "preview");
  let release!: () => void;
  const scriptReady = new Promise<void>((resolve) => { release = resolve; });
  await page.route("**/public/import.js*", async (route) => {
    await scriptReady;
    await route.continue();
  });
  await page.goto("/import", { waitUntil: "commit" });
  const preview = page.getByRole("button", { name: "Preview", exact: true });
  try {
    await expect(preview).toBeDisabled();
  } finally {
    release();
  }
  await expect(preview).toBeEnabled();
});

/** One card of the slow-import file, with its picture at `url`. */
const slowCard = (n: number, url: string) =>
  `<card><name>Slow ${n}</name><text>Flying</text><prop><type>Creature — Bird</type><manacost>{1}{W}</manacost><pt>1/1</pt></prop><set rarity="common" picurl="${url}">SLW</set></card>`;

test("a large import shows progress, and names the pictures it could not fetch", async ({ page }) => {
  // Pictures that answer slowly, so the import is still running when the page
  // looks; and one that answers not at all.
  const slow = "http://127.0.0.1:18081/v1/images/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa/thumb/?delay=400";
  const xml = `<cockatrice_carddatabase><cards>${[1, 2, 3, 4, 5, 6]
    .map((n) => slowCard(n, slow))
    .join("")}${slowCard(7, "http://127.0.0.1:9/nothing.png")}</cards></cockatrice_carddatabase>`;

  await registerUser(page, "slowporter");
  await page.click('a[href="/import"]');
  await page.fill('#import-form input[name="set_code"]', "SLW");
  await page.setInputFiles('#import-form input[name="file"]', {
    name: "slow.xml",
    mimeType: "text/xml",
    buffer: Buffer.from(xml),
  });
  await page.click('#import-form button[type="submit"]');
  await expect(page.locator("#import-preview")).toContainText("7 cards would be created");
  await page.click("#import-preview .import-confirm");

  // It runs in the background, so the page can show it getting on.
  const progress = page.locator("#import-progress");
  await expect(progress).toBeVisible();
  await expect(progress).toContainText("of 7 pictures");

  await expect(page.locator("#import-result")).toContainText("Imported 7 cards with 6 pictures", {
    timeout: 30_000,
  });
  await expect(progress).toBeHidden();
  // The one that would not download is named, so it can be retried.
  await expect(page.locator("#import-result")).toContainText("No picture: Slow 7");
});

test("the import page asks a visitor to sign in first", async ({ page }) => {
  await page.goto("/import");
  await expect(page.locator("#register-form")).toBeVisible();
  await expect(page.locator("#import-form")).toHaveCount(0);
});
