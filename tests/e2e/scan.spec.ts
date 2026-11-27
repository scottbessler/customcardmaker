import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { expectImage } from "./rendering";
import { registerUser } from "./session";

/**
 * A "scan" for the test: one of our own cards, rendered and photographed, so it is a
 * whole printed card whose words are not the ones the card in the editor will have.
 */
async function scanOfAnotherCard(page): Promise<Buffer> {
  await page.goto("/dev/frames?template=modern");
  const card = page.locator(".frame-sample .mtg-card").first();
  await expect(card).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  return card.screenshot();
}

test("a whole-card picture fills the card, with our text laid over it", async ({ page }) => {
  const scan = await scanOfAnotherCard(page);
  await registerUser(page, "scanner");
  await page.fill('.new-set input[name="name"]', "Scans");
  await page.fill('.new-set input[name="code"]', "SCN");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await page.fill('input[name="name"]', "Overlaid Name");
  await page.fill('input[name="mana_cost"]', "{1}{W}");
  await page.fill('input[name="type_line"]', "Creature — Human Knight");
  await page.fill('textarea[name="rules_text"]', "Vigilance");

  const preview = page.locator(".preview .mtg-card");
  await page.setInputFiles('input[name="art"]', { name: "scan.png", mimeType: "image/png", buffer: scan });
  await expect(page.locator(".art-thumb")).toHaveCSS("background-image", /api\/art/);
  // Used as art first: the picture sits in the art box.
  await expect(preview).not.toHaveClass(/art-card/);

  await page.check('input[name="art_is_card"]');
  await expect(preview).toHaveClass(/art-card/);
  await expect(page.locator(".look-option")).toHaveCount(0);
  await expect(preview.locator(".mtg-title")).toHaveText("Overlaid Name");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.reload();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/art-card/);
  await expect(page.locator('input[name="art_is_card"]')).toBeChecked();

  await expectLayout(page, "scan", [".preview .mtg-card", ".preview .mtg-name", ".preview .mtg-type", ".preview .mtg-box"]);
  await expectImage(page.locator(".preview .mtg-card"), "scan.png");

  // Taking the picture away takes the flag with it.
  await page.click(".remove-art");
  await expect(page.locator(".preview .mtg-card")).not.toHaveClass(/art-card/);
  await expect(page.locator(".look-option")).toHaveCount(4);
});
