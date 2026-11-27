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
  await page.locator(".cardtypes .tint-option", { hasText: "Creature" }).click();
  await page.fill('input[name="power_toughness"]', "2/2");
  const card = page.locator(".preview .mtg-card");
  await expect(card.locator(".mana")).toHaveCount(3 + 4 + 4);
  await page.evaluate(() => document.fonts.ready);
  await expectImage(card, "symbols.png");
});

// What "centred" means here: the middle of the capitals, half way from the baseline
// up to the cap height (the ink of "H"). It is not the middle of the line box, which
// has room for descenders and so sits low, and it is not half an x-height, which is
// lower still. Measurements are in CSS pixels; a person sees about a pixel.
const RULES_TOLERANCE = 0.6;
// The symbol against the letters beside it: this is what the eye compares.
const SYMBOL_TOLERANCE = 0.8;
// The letters against their plate. A font's ascent is rounded to whole pixels and a
// plate sits at a fractional position, so this carries a little noise of its own.
const LETTERS_TOLERANCE = 1.0;

const SIZES = {
  // Rules text steps down in size with its length (see rulesSize in frame.js), so
  // one symbol is checked at each of the sizes a card can use.
  large: "{T}: Add one mana of any color.\n{2}{U}, {T}: Scry 2.",
  medium: `Whenever a creature enters the battlefield under your control, you may pay {1}{G}. If you do, draw a card and gain 2 life.\n{T}: Add one mana of any color. ${"Flying, vigilance, trample, and haste. ".repeat(1)}`,
  small: `${"Whenever another creature you control dies, you may pay {2}{B}. If you do, return it to the battlefield. ".repeat(3)}\n{T}: Add {G}. Draw a card, then {2}{U}{U}, {T}: Scry 3.`,
};

/** Where each symbol sits relative to the middle of the letters beside it, per symbol. */
async function symbolOffsets(page) {
  return page.evaluate(() => {
    const canvas = document.createElement("canvas").getContext("2d")!;
    const ascent = (element: Element, text: string) => {
      const style = getComputedStyle(element);
      canvas.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      return canvas.measureText(text).actualBoundingBoxAscent;
    };
    // A zero-size inline box right beside a node sits on that node's own baseline,
    // which matters in a paragraph that wraps. It is a <b>, never a <span>: a card
    // rule such as `.mtg-type span` would otherwise move the probe along with the text.
    // oxlint-disable-next-line unicorn/consistent-function-scoping -- runs in the browser; nothing outside it can be imported
    const baselineBeside = (node: Element) => {
      const probe = document.createElement("b");
      probe.style.cssText = "display:inline-block;width:0;height:0";
      node.after(probe);
      const y = probe.getBoundingClientRect().bottom;
      probe.remove();
      return y;
    };
    // oxlint-disable-next-line unicorn/consistent-function-scoping -- runs in the browser, like the rest of this callback
    const middle = (box: DOMRect) => (box.top + box.bottom) / 2;
    const rules = [...document.querySelectorAll(".preview .mtg-rules .mana")].map((symbol) => {
      const letters = ascent(symbol.closest("p")!, "H");
      return middle(symbol.getBoundingClientRect()) - (baselineBeside(symbol) - letters / 2);
    });
    // A plate: its text is centred on the plate by its letters, and the symbol
    // beside it (the cost, or the rarity pip) is centred on those same letters.
    const plate = (plateSelector: string, textSelector: string, symbolSelector: string) => {
      const text = document.querySelector(textSelector)!;
      const probe = document.createElement("b");
      probe.style.cssText = "display:inline-block;width:0;height:0";
      text.append(probe);
      const baseline = probe.getBoundingClientRect().bottom;
      probe.remove();
      const letters = baseline - ascent(text, "H") / 2;
      return {
        lettersOnPlate: letters - middle(document.querySelector(plateSelector)!.getBoundingClientRect()),
        symbolOnLetters: middle(document.querySelector(symbolSelector)!.getBoundingClientRect()) - letters,
      };
    };
    return {
      rules,
      name: plate(".preview .mtg-name", ".preview .mtg-title", ".preview .mtg-cost .mana"),
      type: plate(".preview .mtg-type", ".preview .mtg-type span", ".preview .mtg-pip"),
    };
  });
}

for (const [size, text] of Object.entries(SIZES)) {
  test(`symbols sit on the middle of the letters beside them (${size} rules text)`, async ({ page }) => {
    await cardInEditor(page);
    await page.fill('input[name="name"]', "Prism Adept");
    await page.fill('input[name="mana_cost"]', "{2}{W/U}{G}");
    await page.fill('input[name="type_line"]', "Creature — Elf Druid");
    await page.fill('textarea[name="rules_text"]', text);
    await expect(page.locator(".preview .mtg-rules .mana").first()).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    const found = await symbolOffsets(page);
    expect(found.rules.length, "the sample has symbols to measure").toBeGreaterThanOrEqual(3);
    for (const offset of found.rules) {
      expect(Math.abs(offset), `a rules-text symbol is ${offset.toFixed(2)}px off the middle of its letters`).toBeLessThanOrEqual(RULES_TOLERANCE);
    }
    for (const [name, plate] of Object.entries({ name: found.name, type: found.type })) {
      expect(Math.abs(plate.lettersOnPlate), `${name} letters are ${plate.lettersOnPlate.toFixed(2)}px off the middle of their plate`).toBeLessThanOrEqual(LETTERS_TOLERANCE);
      expect(Math.abs(plate.symbolOnLetters), `the ${name} plate's symbol is ${plate.symbolOnLetters.toFixed(2)}px off the middle of its letters`).toBeLessThanOrEqual(SYMBOL_TOLERANCE);
    }
  });
}

test("a symbol is the size of the text beside it, so small text does not get a big disc", async ({ page }) => {
  await cardInEditor(page);
  await page.fill('textarea[name="rules_text"]', SIZES.large);
  await expect(page.locator(".preview .mtg-rules .mana").first()).toBeVisible();
  const discAt = async () => page.locator(".preview .mtg-rules .ms-tap").first().evaluate((el) => el.getBoundingClientRect().height);
  const textAt = async () => page.locator(".preview .mtg-rules p").first().evaluate((el) => Number.parseFloat(getComputedStyle(el).fontSize));
  const [bigDisc, bigText] = [await discAt(), await textAt()];
  await page.fill('textarea[name="rules_text"]', SIZES.small);
  await expect.poll(textAt).toBeLessThan(bigText);
  const [smallDisc, smallText] = [await discAt(), await textAt()];
  expect(smallDisc).toBeLessThan(bigDisc);
  // The disc keeps its proportion to the text it stands in.
  expect(smallDisc / smallText).toBeCloseTo(bigDisc / bigText, 0);
});
