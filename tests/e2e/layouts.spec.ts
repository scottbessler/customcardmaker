import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
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
  await page.getByRole("button", { name: "Start editing" }).click();
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
  await page.getByRole("button", { name: "Start editing" }).click();
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/lay-modern/);
  await page.click(".edit-template");
  await expect(feature(page, "cost")).toBeChecked();
  await page.locator('.look-option:has(input[value="planeswalker"])').click();
  await expect(feature(page, "loyalty")).toBeDisabled();
  await page.click(".apply-template");
  await page.fill('input[name="loyalty"]', "4");
  await page.fill('textarea[name="rules_text"]', "+1: Draw a card\n−3: Destroy target creature\n: Static ability");
  await expect(preview.locator(".pw-row")).toHaveCount(3);
  await expect(preview.locator(".pw-cost").first()).toHaveText("+1");
  await expect(preview.locator(".mtg-loyalty")).toHaveText("4");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  await page.reload();
  await expect(page.locator(".preview .mtg-card")).toHaveClass(/lay-planeswalker/);
  await expect(page.locator(".preview .pw-row")).toHaveCount(3);

  await page.click(".edit-template");
  await page.locator('.look-option:has(input[value="saga"])').click();
  await expect(feature(page, "power_toughness")).toBeDisabled();
  await page.click(".apply-template");
  await page.fill('textarea[name="rules_text"]', ": (Reminder.)\nI: One\nII, III: Two");
  await expect(page.locator(".preview .saga-row")).toHaveCount(2);
  await expect(page.locator(".preview .saga-mark")).toHaveCount(3);

  await page.click(".edit-template");
  await page.locator('.look-option:has(input[value="token"])').click();
  await expect(feature(page, "cost")).toBeDisabled();
  await page.click(".apply-template");
  await expect(preview).toHaveClass(/lay-token/);
  await expect(page.locator('input[name="mana_cost"]')).toHaveCount(0);
  await expect(page.locator(".preview .mtg-cost")).toBeHidden();
});

test("every layout keeps its parts on the card", async ({ page }) => {
  await page.goto("/dev/frames?layouts=1");
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
  await expectLayout(page, "layouts-modern", [
    ".frame-sample:nth-child(1) .mtg-name",
    ".frame-sample:nth-child(1) .mtg-box",
    ".frame-sample:nth-child(3) .mtg-box",
    ".frame-sample:nth-child(4) .mtg-art",
  ]);
});

test("every template keeps its parts on the card", async ({ page }) => {
  await page.goto("/dev/frames?templates=1");
  await expect(page.locator(".frame-sample .mtg-card")).toHaveCount(15);
  await page.evaluate(() => document.fonts.ready);
  const problems = await page.evaluate(() =>
    [...document.querySelectorAll(".frame-sample")].flatMap((sample) => {
      const card = sample.querySelector(".mtg-card")!;
      const bounds = card.getBoundingClientRect();
      const label = (sample as HTMLElement).dataset.template;
      // A template draws the parts it has a place for, and may have none of some.
      const parts = [...card.querySelectorAll(".mtg-name, .mtg-type, .mtg-box, .mtg-art")];
      if (!parts.length) return [`${label}: nothing drawn`];
      return parts.flatMap((box) => {
        const b = box.getBoundingClientRect();
        const inside = b.left >= bounds.left - 0.5 && b.right <= bounds.right + 0.5 && b.top >= bounds.top - 0.5 && b.bottom <= bounds.bottom + 0.5;
        return inside ? [] : [`${label}: ${box.className} leaves the card`];
      });
    }),
  );
  expect(problems).toEqual([]);
  await expectLayout(page, "templates", [
    '.frame-sample[data-template="modern"] .mtg-art',
    '.frame-sample[data-template="battle"] .mtg-pt',
    '.frame-sample[data-template="textless-token"] .mtg-name',
    '.frame-sample[data-template="split"] .mtg-art',
    '.frame-sample[data-template="split"] .mtg-box',
    '.frame-sample[data-template="aftermath"] .mtg-box',
    '.frame-sample[data-template="adventure"] .mtg-box',
    '.frame-sample[data-template="semi-split"] .mtg-art',
  ]);
});

test("a standard card is split the way a real one is", async ({ page }) => {
  await registerUser(page, "standard");
  await page.fill('.new-set input[name="name"]', "Standard");
  await page.fill('.new-set input[name="code"]', "STD");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await page.getByRole("button", { name: "Start editing" }).click();
  await page.fill('input[name="name"]', "Split Check");
  await page.fill('input[name="mana_cost"]', "{1}{U}");
  await page.click(".edit-template");
  await page.check('input[name="feature-power_toughness"]');
  await page.click(".apply-template");
  await page.fill('input[name="power_toughness"]', "2/2");

  const card = page.locator(".preview .mtg-card");
  const art = page.locator(".preview .mtg-art");
  const rules = page.locator(".preview .mtg-box");
  await expect(art).toBeVisible();
  const [c, a, r] = await Promise.all([card, art, rules].map(async (l) => (await l.boundingBox())!));
  const at = (v: number) => (v - c.y) / c.height;
  // Measured off a real card: the art is the top half (11%..56% of the card) and the
  // text box the bottom third (62%..93%), so the art is taller than the box.
  expect(at(a.y), "the art's top").toBeGreaterThan(0.09);
  expect(at(a.y), "the art's top").toBeLessThan(0.14);
  expect(at(a.y + a.height), "the art's bottom").toBeGreaterThan(0.52);
  expect(at(a.y + a.height), "the art's bottom").toBeLessThan(0.6);
  expect(at(r.y), "the text box's top").toBeGreaterThan(0.58);
  expect(at(r.y + r.height), "the text box's bottom").toBeLessThan(0.97);
  expect(a.height, "the art is taller than the text box").toBeGreaterThan(r.height);
  await expect(page.locator(".preview .mtg-pt")).toHaveText("2/2");
  await expect(page.locator(".save-state")).toHaveText(/Saved/);

  // One look so far, and only a look may be the set's default.
  await page.click(".edit-template");
  await expect(page.locator(".look-option")).toHaveCount(15);
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.locator(".set-settings summary").click();
  await expect(page.locator('select[name="default-template"] option')).toHaveText(["Modern"]);
});

test("a creature planeswalker draws both stats without overlapping badges or footer text", async ({ page }) => {
  await registerUser(page);
  const set = await (await page.request.post("/api/sets", { data: { name: "Creature planeswalkers", code: "CPW" } })).json();
  const created = await page.request.post(`/api/sets/${set.id}/cards`, { data: {
    features: ["power_toughness", "loyalty"], template: "planeswalker",
    name: "Living Planeswalker", type_line: "Creature Planeswalker", power_toughness: "2/3", loyalty: "4",
  } });
  expect(created.ok()).toBe(true);
  await page.goto(`/sets/${set.id}`);
  const preview = page.locator(".preview .mtg-card");
  const power = preview.locator(".mtg-pt:not(.mtg-loyalty)");
  const loyalty = preview.locator(".mtg-loyalty");
  await expect(power).toHaveText("2/3");
  await expect(loyalty).toHaveText("4");
  const [pt, badge, footer] = await Promise.all([power, loyalty, preview.locator(".mtg-foot")].map(async (part) => (await part.boundingBox())!));
  expect(pt.x + pt.width).toBeLessThan(badge.x);
  expect(footer.x + footer.width).toBeLessThan(pt.x);
});

test("Adventure places its spell type and rules below the art and left of the main text", async ({ page }) => {
  await page.goto("/dev/frames?templates=1");
  const adventure = page.locator('.frame-sample[data-template="adventure"]');
  const spellType = adventure.locator('.mtg-type[data-part-face="half"]');
  const spellRules = adventure.locator(".mtg-box").filter({ hasText: "Draw a card." });
  const mainRules = adventure.locator(".mtg-box").filter({ hasText: "Adventure sample." });
  await expect(spellType).toHaveText("Instant");
  await expect(spellRules).toBeVisible();
  const [art, type, spell, main] = await Promise.all(
    [adventure.locator(".mtg-art"), spellType, spellRules, mainRules].map(async (part) => (await part.boundingBox())!),
  );
  expect(art.y + art.height).toBeLessThan(type.y);
  expect(type.y).toBeLessThan(spell.y);
  expect(Math.round(spell.x + spell.width)).toBeLessThanOrEqual(Math.round(main.x));
});

test("frame treatments follow the type line and rules text starts at the top", async ({ page }) => {
  await registerUser(page);
  const set = await (await page.request.post("/api/sets", { data: { name: "Frame treatments", code: "FRM" } })).json();
  const saved = await page.request.post(`/api/sets/${set.id}/cards`, { data: {
    name: "Gleaming Relic", type_line: "Legendary Enchantment Artifact", mana_cost: "{W}", rules_text: "Flying",
  } });
  expect(saved.ok()).toBe(true);
  await page.goto(`/sets/${set.id}`);
  const preview = page.locator(".preview .mtg-card");
  await expect(preview).toHaveClass(/frame-w/);
  await expect(preview).toHaveClass(/treatment-legendary/);
  await expect(preview).toHaveClass(/treatment-enchantment/);
  await expect(preview).toHaveClass(/treatment-artifact/);
  expect(await preview.locator(".mtg-box").evaluate((box) => {
    const top = box.getBoundingClientRect().top + parseFloat(getComputedStyle(box).paddingTop);
    return Math.abs(box.querySelector(".mtg-rules")!.getBoundingClientRect().top - top);
  })).toBeLessThan(1);
  await page.fill('input[name="type_line"]', "Creature — Artifact");
  await expect(preview).not.toHaveClass(/treatment-(legendary|enchantment|artifact)/);
  await expect(preview).toHaveClass(/frame-w/);
});

test("wide templates use the rotated card aspect and keep their parts on the canvas", async ({ page }) => {
  await page.goto("/dev/frames?templates=1");
  await Promise.all(["landscape", "split", "fuse", "semi-split"].map(async (id) => {
    const sample = page.locator(`.frame-sample[data-template="${id}"] .mtg-card`);
    await expect(sample).toHaveAttribute("data-orientation", "landscape");
    const { width, height } = (await sample.boundingBox())!;
    expect(width / height).toBeCloseTo(88 / 63, 2);
    await expect(sample.locator(".mtg-title")).toHaveCount(2);
  }));
});
