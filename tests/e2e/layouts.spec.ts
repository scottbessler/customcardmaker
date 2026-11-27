import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { expectImage } from "./rendering";
import { registerUser } from "./session";

test("planeswalker abilities and saga chapters are read from the rules text", async ({ page }) => {
  await page.goto("/");
  const parsed = await page.evaluate(async () => {
    const { abilities, chapters } = await import("/public/frame.js");
    return {
      pw: abilities("+1: Add {B}\n-7: Big\n−2: Minus\n0: Draw\n: static\nPlain\nX: not a cost"),
      saga: chapters(": (Reminder)\nI: One\nII, III: Two\nmore\nIV: Four"),
    };
  });
  expect(parsed.pw).toEqual([
    { cost: "+1", text: "Add {B}" },
    { cost: "−7", text: "Big" },
    { cost: "−2", text: "Minus" },
    { cost: "0", text: "Draw" },
    { cost: "", text: "static" },
    { cost: "", text: "Plain" },
    { cost: "", text: "X: not a cost" },
  ]);
  expect(parsed.saga).toEqual({
    reminder: ["(Reminder)"],
    chapters: [
      { marks: ["I"], lines: ["One"] },
      { marks: ["II", "III"], lines: ["Two", "more"] },
      { marks: ["IV"], lines: ["Four"] },
    ],
  });
});

test("a card can be made a planeswalker, a saga or a token, and keeps it", async ({ page }) => {
  await registerUser(page, "layouter");
  await page.fill('.new-set input[name="name"]', "Layouts");
  await page.fill('.new-set input[name="code"]', "LAY");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/lay-normal/);
  await expect(page.locator('input[name="power_toughness"]')).toBeVisible();

  await page.locator(".layouts .tint-option", { hasText: "Planeswalker" }).click();
  await expect(preview).toHaveClass(/lay-planeswalker/);
  // Loyalty replaces power/toughness.
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await page.fill('input[name="loyalty"]', "4");
  await page.fill('textarea[name="rules_text"]', "+1: Draw a card\n−3: Destroy target creature\n: Static ability");
  await expect(preview.locator(".pw-row")).toHaveCount(3);
  await expect(preview.locator(".pw-cost").first()).toHaveText("+1");
  await expect(preview.locator(".mtg-loyalty")).toHaveText("4");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.reload();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-planeswalker/);
  await expect(page.locator(".preview .pw-row")).toHaveCount(3);

  await page.locator(".layouts .tint-option", { hasText: "Saga" }).click();
  await page.fill('textarea[name="rules_text"]', ": (Reminder.)\nI: One\nII, III: Two");
  await expect(page.locator(".preview .saga-row")).toHaveCount(2);
  await expect(page.locator(".preview .saga-mark")).toHaveCount(3);
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await expect(page.locator('input[name="loyalty"]')).toHaveCount(0);

  await page.locator(".layouts .tint-option", { hasText: "Token" }).click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-token/);
  await expect(page.locator(".preview .mtg-cost")).toBeHidden();
  await page.locator(".layouts .tint-option", { hasText: "Normal" }).click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-normal/);
});

for (const template of ["modern", "classic", "borderless", "full-art"]) {
  test(`${template}: every layout keeps its parts on the card`, async ({ page }) => {
    await page.goto(`/dev/frames?layouts=1&template=${template}`);
    await expect(page.locator(".frame-sample .mtg-card")).toHaveCount(5);
    await page.evaluate(() => document.fonts.ready);
    const outside = await page.evaluate(() =>
      [...document.querySelectorAll(".frame-sample .mtg-card")].flatMap((card) => {
        const bounds = card.getBoundingClientRect();
        return [".mtg-name", ".mtg-type", ".mtg-box"].flatMap((part) => {
          const box = card.querySelector(part)?.getBoundingClientRect();
          if (!box) return [`${card.className}: no ${part}`];
          const inside = box.left >= bounds.left - 0.5 && box.right <= bounds.right + 0.5 && box.top >= bounds.top - 0.5 && box.bottom <= bounds.bottom + 0.5;
          return inside ? [] : [`${card.getAttribute("aria-label")}: ${part} leaves the card`];
        });
      }),
    );
    expect(outside).toEqual([]);
    await expectLayout(page, `layouts-${template}`, [
      ".frame-sample:nth-child(1) .mtg-name",
      ".frame-sample:nth-child(1) .mtg-box",
      ".frame-sample:nth-child(3) .mtg-box",
      ".frame-sample:nth-child(4) .mtg-art",
    ]);
    await expectImage(page.locator(".frames-grid"), `layouts-${template}.png`);
  });
}
