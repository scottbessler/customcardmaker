import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { expectImage } from "./rendering";
import { registerUser } from "./session";

test("planeswalker abilities, saga chapters and reminder text are read from the rules text", async ({ page }) => {
  await page.goto("/");
  const parsed = await page.evaluate(async () => {
    const { abilities, chapters, reminders } = await import("/public/frame.js");
    return {
      pw: abilities("+1: Add {B}\n-7: Big\n−2: Minus\n0: Draw\n: static\nPlain\nX: not a cost"),
      saga: chapters(": (Reminder)\nI: One\nII, III: Two\nmore\nIV: Four"),
      reminder: reminders("Flying (It can only be blocked by creatures with flying or reach.) Haste"),
      none: reminders("No parentheses here"),
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
  expect(parsed.reminder).toEqual([
    { text: "Flying ", reminder: false },
    { text: "It can only be blocked by creatures with flying or reach.", reminder: true },
    { text: " Haste", reminder: false },
  ]);
  expect(parsed.none).toEqual([{ text: "No parentheses here", reminder: false }]);
});

test("reminder text is printed in italics, and the rest is not", async ({ page }) => {
  await registerUser(page, "reminder");
  await page.fill('.new-set input[name="name"]', "Reminders");
  await page.fill('.new-set input[name="code"]', "REM");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await page.fill(
    'textarea[name="rules_text"]',
    "Flying (It can only be blocked by creatures with flying or reach.)",
  );
  const reminder = page.locator(".preview .mtg-rules .reminder");
  await expect(reminder).toHaveText("It can only be blocked by creatures with flying or reach.");
  await expect(reminder).toHaveCSS("font-style", "italic");
  await expect(page.locator(".preview .mtg-rules p")).toHaveCSS("font-style", "normal");
});

const feature = (page, id: string) => page.locator(`input[name="feature-${id}"]`);

test("a card becomes a planeswalker, a saga or a token by what it has and how it is drawn, and keeps it", async ({ page }) => {
  await registerUser(page, "layouter");
  await page.fill('.new-set input[name="name"]', "Layouts");
  await page.fill('.new-set input[name="code"]', "LAY");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/lay-normal/);
  // A new card has a mana cost and no stats.
  await expect(feature(page, "cost")).toBeChecked();
  await expect(page.locator('input[name="power_toughness"]')).toHaveCount(0);
  await feature(page, "power_toughness").check();
  await expect(page.locator('input[name="power_toughness"]')).toBeVisible();

  // Loyalty makes it a planeswalker; the stats combine, so power/toughness may stay.
  await feature(page, "loyalty").check();
  await expect(preview).toHaveClass(/lay-planeswalker/);
  await expect(page.locator('input[name="power_toughness"]')).toBeVisible();
  await feature(page, "power_toughness").uncheck();
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

  // A saga is any card drawn by the Saga template, which has nowhere for a stat.
  await feature(page, "loyalty").uncheck();
  await page.locator(".look-option", { hasText: "Saga" }).click();
  await page.fill('textarea[name="rules_text"]', ": (Reminder.)\nI: One\nII, III: Two");
  await expect(page.locator(".preview .saga-row")).toHaveCount(2);
  await expect(page.locator(".preview .saga-mark")).toHaveCount(3);
  await feature(page, "power_toughness").check();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-normal/);
  await expect(page.locator(".look-option", { hasText: "Saga" })).toHaveCount(0);
  await feature(page, "power_toughness").uncheck();

  // A token is a card with no mana cost, drawn by the Token template.
  await expect(page.locator(".look-option", { hasText: "Token" })).toHaveCount(0);
  await feature(page, "cost").uncheck();
  await page.locator(".look-option", { hasText: "Token" }).click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-token/);
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
  await expect(page.locator(".preview .mtg-cost")).toBeHidden();
  // Giving it a cost again takes the token look off it.
  await feature(page, "cost").check();
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

test("the Nexus template lays a normal card out the way Nexus does, with a tall art box", async ({ page }) => {
  await registerUser(page, "nexus");
  await page.fill('.new-set input[name="name"]', "Nexus");
  await page.fill('.new-set input[name="code"]', "NEX");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await page.fill('input[name="name"]', "Tall Art");
  await page.fill('input[name="mana_cost"]', "{1}{U}");
  await page.check('input[name="feature-power_toughness"]');
  await page.fill('input[name="power_toughness"]', "2/2");
  const art = page.locator(".preview .mtg-art");
  const standard = (await art.boundingBox())!.height;

  await page.locator(".look-option", { hasText: "Nexus" }).click();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-nexus/);
  // The art box is 62 card-widths tall there, against 41 in the standard layout.
  const tall = (await art.boundingBox())!.height;
  expect(tall / standard).toBeGreaterThan(1.4);
  await expect(page.locator(".preview .mtg-pt")).toHaveText("2/2");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.reload();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-nexus/);
  // Nexus takes its frame from the card's look, so the set can still choose that.
  await page.locator(".set-settings summary").click();
  await expect(page.locator('select[name="default-template"] option')).toHaveText(["Modern", "Classic", "Borderless", "Full art"]);
});
