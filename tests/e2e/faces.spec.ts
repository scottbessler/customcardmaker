import { expect, test } from "./fixtures";
import { registerUser } from "./session";

async function newSet(page, code: string) {
  await registerUser(page, "faces");
  await page.fill('.new-set input[name="name"]', "Faces");
  await page.fill('.new-set input[name="code"]', code);
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
}

test("a card can be given a back face, turned over, and keeps both", async ({ page }) => {
  await newSet(page, "DFC");
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
  await page.fill('input[name="name"]', "Day Side");
  await page.fill('input[name="mana_cost"]', "{R}");
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).not.toHaveAttribute("data-face", /.*/);

  await page.click(".edit-template");
  await page.check('input[name="feature-back"]');
  await page.getByRole("dialog").getByRole("button", { name: "Back", exact: true }).click();
  await page.check('input[name="feature-power_toughness"]');
  await page.click(".apply-template");
  await page.getByRole("button", { name: "Back", exact: true }).click();
  await expect(preview).toHaveAttribute("data-face", "back");
  await expect(page.locator(".face-tab.chosen")).toHaveText("Back");
  // The back is a blank card of its own, and keeps the front's colour while it has no cost.
  await expect(preview.locator(".mtg-title")).toContainText("Untitled");
  await expect(preview).toHaveClass(/frame-r/);
  await preview.getByRole("button", { name: "Edit name and mana cost" }).click();
  const backDialog = page.getByRole("dialog", { name: "Edit name and mana cost" });
  await backDialog.getByLabel("Name").fill("Night Side");
  await backDialog.getByRole("button", { name: "Done" }).click();
  await page.fill('input[name="power_toughness"]', "3/3");
  await expect(preview.locator(".mtg-title")).toHaveText("Night Side");
  await expect(preview.locator(".mtg-dfc")).toHaveClass(/dfc-back/);
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.click(".face-tab >> text=Front");
  await expect(preview).toHaveAttribute("data-face", "front");
  await expect(page.locator('input[name="name"]')).toHaveValue("Day Side");
  await expect(preview.locator(".mtg-dfc")).toHaveClass(/dfc-front/);
  // The filmstrip shows the front.
  await expect(page.locator(".filmstrip .mtg-card .mtg-title")).toHaveText("Day Side");

  await page.click(".flip-card");
  await expect(preview).toHaveAttribute("data-face", "back");
  await page.click(".flip-card");
  await expect(preview).toHaveAttribute("data-face", "front");

  await page.reload();
  await expect(page.locator(".face-tab")).toHaveCount(2);
  await page.click(".flip-card");
  await expect(page.locator(".preview .mtg-title")).toHaveText("Night Side");
  await expect(page.locator(".preview .mtg-pt")).toHaveText("3/3");

  await page.click(".edit-template");
  await page.getByRole("dialog").getByRole("button", { name: "Front", exact: true }).click();
  await page.uncheck('input[name="feature-back"]');
  await page.click(".apply-template");
  await expect(page.locator(".face-tab")).toHaveCount(0);
  await expect(page.locator(".flip-card")).toHaveCount(0);
  await expect(page.locator(".preview .mtg-title")).toHaveText("Day Side");
});

test("an imported double-faced card is one card with both faces", async ({ page }) => {
  const xml = `<cockatrice_carddatabase><cards>
<card><name>Day Face</name><text>Flip me.</text><prop><layout>transform</layout><side>front</side><type>Creature — Human</type><manacost>{1}{W}</manacost><pt>1/1</pt></prop><set rarity="rare">DIM</set><related attach="transform">Night Face</related></card>
<card><name>Night Face</name><text>Flipped.</text><prop><layout>transform</layout><side>back</side><type>Creature — Werewolf</type><pt>2/2</pt></prop><set rarity="rare">DIM</set><related attach="transform">Day Face</related></card>
</cards></cockatrice_carddatabase>`;
  await registerUser(page, "dimporter");
  await page.click('a[href="/import"]');
  await page.fill('#import-form input[name="set_code"]', "DIM");
  await page.fill('#import-form input[name="set_name"]', "Two Faced");
  await page.setInputFiles('#import-form input[name="file"]', { name: "c.xml", mimeType: "text/xml", buffer: Buffer.from(xml) });
  await page.uncheck('#import-form input[name="images"]');
  await page.click('#import-form button[type="submit"]');
  await expect(page.locator("#import-preview")).toContainText("1 card would be created");
  await page.click("#import-preview .import-confirm");
  await expect(page.locator("#import-result")).toContainText("Imported 1 card");
  await page.click("#import-result a");
  await expect(page.locator(".strip-item")).toHaveCount(1);
  await page.click(".flip-card");
  await expect(page.locator(".preview .mtg-title")).toHaveText("Night Face");
  // The back has no cost, so it wears the front's white.
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/frame-w/);
});

test("a public set's double-faced card can be turned over by a visitor", async ({ page }) => {
  await newSet(page, "PUB");
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
  await page.fill('input[name="name"]', "Public Day");
  await page.click(".edit-template");
  await page.check('input[name="feature-back"]');
  await page.getByRole("dialog").getByRole("button", { name: "Back", exact: true }).click();
  await page.check('input[name="feature-power_toughness"]');
  await page.click(".apply-template");
  await page.getByRole("button", { name: "Back", exact: true }).click();
  await page.fill('input[name="name"]', "Public Night");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);
  await page.locator(".set-settings summary").click();
  await page.check('input[name="public"]');
  await expect(page.locator(".public-badge")).toBeVisible();
  await page.click(".public-link");
  await expect(page.locator(".public-card .mtg-title")).toHaveText("Public Day");
  await page.click(".public-card .flip-card");
  await expect(page.locator(".public-card .mtg-title")).toHaveText("Public Night");
});
