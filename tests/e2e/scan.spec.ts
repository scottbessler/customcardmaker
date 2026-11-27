import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { expectImage } from "./rendering";
import { registerUser } from "./session";

/**
 * A stand-in for an MTG Nexus render: a 400x560 card with its plates, art and printed words in
 * the places a real one keeps them (measured off real exports), so the overlay has something
 * to hide and a place to be wrong about.
 */
async function nexusLikeScan(page): Promise<Buffer> {
  await page.setContent(`<body style="margin:0;background:#000"><div id="scan" style="position:relative;width:400px;height:560px;background:#0b0b0b;font:20px serif;color:#111">
    <div style="position:absolute;left:20px;top:25px;width:360px;height:38px;background:#e9e3d0;border-radius:10px;line-height:38px;padding-left:14px;box-sizing:border-box">PRINTED NAME</div>
    <div style="position:absolute;left:31px;top:62px;width:338px;height:248px;background:#5b7a52"></div>
    <div style="position:absolute;left:20px;top:312px;width:360px;height:36px;background:#e9e3d0;border-radius:10px;line-height:36px;padding-left:14px;box-sizing:border-box">PRINTED TYPE LINE</div>
    <div style="position:absolute;left:28px;top:349px;width:344px;height:151px;background:#f3eedf;padding:14px;box-sizing:border-box">printed rules words<br>more printed words</div>
    <div style="position:absolute;left:314px;top:500px;width:64px;height:34px;background:#e9e3d0;line-height:34px;text-align:center">9/9</div>
  </div></body>`);
  return page.locator("#scan").screenshot();
}

test("a whole-card picture fills the card, with our text laid over it", async ({ page }) => {
  const scan = await nexusLikeScan(page);
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
  // The filmstrip is above the preview, so it must be drawn before the boxes are read.
  await expect(page.locator(".filmstrip .strip-item")).toHaveCount(1);

  // Boxes are measured from the top of the page, wherever the checkbox clicks left it scrolled.
  await page.evaluate(() => window.scrollTo(0, 0));
  await expectLayout(page, "scan", [".preview .mtg-card", ".preview .mtg-name", ".preview .mtg-type", ".preview .mtg-box"]);
  await expectImage(page.locator(".preview .mtg-card"), "scan.png");

  // Or keep what the picture says: none of ours is drawn.
  await page.check('input[name="keep_printed_text"]');
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/art-card-plain/);
  await expect(page.locator(".preview .mtg-frame")).toBeHidden();
  await page.uncheck('input[name="keep_printed_text"]');
  await expect(page.locator(".preview .mtg-frame")).toBeVisible();

  // Taking the picture away takes the flags with it.
  await page.click(".remove-art");
  await expect(page.locator(".preview .mtg-card")).not.toHaveClass(/art-card/);
  await expect(page.locator(".look-option")).toHaveCount(6);
});
