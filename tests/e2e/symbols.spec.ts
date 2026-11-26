import { expect, test } from "./fixtures";
import { expectImage } from "./rendering";
import { registerUser } from "./session";

async function cardInEditor(page) {
  await registerUser(page);
  await page.fill('.new-set input[name="name"]', `Symbols ${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`);
  await page.fill('.new-set input[name="code"]', "SYM");
  await page.click(".new-set button");
  await expect(page.locator("h1")).toBeVisible();
  await page.click(".add-card");
  await expect(page.locator(".filmstrip .mtg-card")).toHaveCount(1);
}

test("every symbol we claim to support has a glyph in the font", async ({ page }) => {
  await page.goto("/");
  const missing = await page.evaluate(async () => {
    const { SYMBOL_KEYS } = await import("/public/frame.js");
    await document.fonts.load("16px Mana");
    return SYMBOL_KEYS.filter((key) => {
      const probe = document.createElement("i");
      probe.className = `ms ms-cost ms-${key}`;
      document.body.append(probe);
      const content = getComputedStyle(probe, "::before").content;
      probe.remove();
      return content === "none" || content === "normal" || content === '""';
    });
  });
  expect(missing, "SYMBOL_KEYS names a symbol the vendored font lacks").toEqual([]);
});

test("the Mana font loads, so symbols are never boxes", async ({ page }) => {
  await cardInEditor(page);
  await page.fill('input[name="mana_cost"]', "{2}{W}");
  await expect(page.locator(".preview .mtg-cost .mana")).toHaveCount(2);
  expect(await page.evaluate(async () => {
    await document.fonts.load("16px Mana");
    return document.fonts.check("16px Mana");
  })).toBe(true);
});

test("symbols are read the way they are written, in either order", async ({ page }) => {
  await cardInEditor(page);
  await page.fill('textarea[name="rules_text"]', "{T}: Add {W/U} or {U/W}.\n{G/U/P}, {u/g/p}, {2}, {X}, {q}");
  const rules = page.locator(".preview .mtg-rules");
  await expect(rules.locator(".ms-tap")).toHaveCount(1);
  await expect(rules.locator(".ms-wu")).toHaveCount(2);
  await expect(rules.locator(".ms-gup")).toHaveCount(2);
  await expect(rules.locator(".ms-2")).toHaveCount(1);
  await expect(rules.locator(".ms-x")).toHaveCount(1);
  await expect(rules.locator(".ms-untap")).toHaveCount(1);
  await expect(rules.locator(".mana-unknown")).toHaveCount(0);
});

test("a symbol the font does not have is shown as typed", async ({ page }) => {
  await cardInEditor(page);
  await page.fill('input[name="mana_cost"]', "{1}{Nope}");
  await expect(page.locator(".preview .mtg-cost .mana")).toHaveCount(1);
  await expect(page.locator(".preview .mtg-cost .mana-unknown")).toHaveText("{NOPE}");
});

test("the footer says whose symbols these are", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".site-footer")).toContainText("Wizards of the Coast");
  await expect(page.locator(".site-footer")).toContainText("not approved or endorsed");
});

test("a card with every kind of symbol keeps its look", async ({ page }) => {
  await cardInEditor(page);
  await page.fill('input[name="name"]', "Prism Adept");
  await page.fill('input[name="mana_cost"]', "{2}{W/U}{G/P}");
  await page.fill('input[name="type_line"]', "Creature — Human Wizard");
  await page.fill('textarea[name="rules_text"]', "{T}: Add {R}, {G}, or {B}.\n{X}{U}{U}, {T}: Draw a card.");
  await page.fill('input[name="power_toughness"]', "2/2");
  const card = page.locator(".preview .mtg-card");
  await expect(card.locator(".mana")).toHaveCount(3 + 4 + 4);
  await page.evaluate(() => document.fonts.ready);
  await expectImage(card, "symbols.png");
});

test("a symbol sits on the middle of its line of text, not below it", async ({ page }) => {
  await cardInEditor(page);
  await page.fill('textarea[name="rules_text"]', "{T}: Draw a card.\n{2}{U}, {T}: Scry 2.");
  await expect(page.locator(".preview .mtg-rules .mana")).toHaveCount(4);
  await page.evaluate(() => document.fonts.ready);
  // Where a reader's eye puts the middle of a line: half an x-height above the
  // baseline. The disc's centre should be there, to the pixel.
  const offsets = await page.evaluate(() =>
    [...document.querySelectorAll(".preview .mtg-rules p")].flatMap((paragraph) => {
      const style = getComputedStyle(paragraph);
      const canvas = document.createElement("canvas").getContext("2d")!;
      canvas.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      const xHeight = canvas.measureText("x").actualBoundingBoxAscent;
      // A zero-size inline box sits exactly on the baseline.
      const probe = document.createElement("span");
      probe.style.cssText = "display:inline-block;width:0;height:0";
      paragraph.append(probe);
      const baseline = probe.getBoundingClientRect().bottom;
      probe.remove();
      return [...paragraph.querySelectorAll(".mana")].map((symbol) => {
        const box = symbol.getBoundingClientRect();
        return (box.top + box.bottom) / 2 - (baseline - xHeight / 2);
      });
    }),
  );
  expect(offsets.length).toBe(4);
  for (const offset of offsets) expect(Math.abs(offset), `a symbol is ${offset.toFixed(2)}px off the middle of its line`).toBeLessThanOrEqual(1);
});
