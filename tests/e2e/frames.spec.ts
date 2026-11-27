import { expect, test } from "./fixtures";
import { expectLayout } from "./layout";
import { expectImage } from "./rendering";

const FRAMES = ["w", "u", "b", "r", "g", "gold", "artifact", "land", "colorless"];

/** WCAG relative luminance of an `rgb(r, g, b)` / `rgba(...)` string. */
function luminance(css: string): number {
  // Translucent plates compute to `color(srgb 0.98 0.97 0.92 / 0.88)`, on a 0-1 scale.
  const scale = css.startsWith("color(") ? 1 : 255;
  const [r, g, b] = css
    .replace(/\/.*$/, "")
    .match(/[\d.]+/g)!
    .slice(0, 3)
    .map((value) => {
      const c = Number(value) / scale;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].toSorted((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

const TEMPLATES = ["modern", "classic", "borderless", "full-art"];

for (const template of TEMPLATES) {
test.describe(template, () => {
test.beforeEach(async ({ page }) => {
  await page.goto(`/dev/frames?template=${template}`);
  await expect(page.locator(".frame-sample .mtg-card")).toHaveCount(FRAMES.length);
  await page.evaluate(() => document.fonts.ready);
});

test("every frame has a sample, and each is chosen by the card, not hard-coded", async ({ page }) => {
  await Promise.all(
    FRAMES.map((frame) => expect(page.locator(`.frame-sample[data-frame="${frame}"] .mtg-card.frame-${frame}`)).toHaveCount(1)),
  );
});

test("text stays legible on every plate and text box", async ({ page }) => {
  // The card's ink against the solid tint each piece of text sits on. The
  // plates are gradients over that tint, so the tint is the floor.
  const pairs = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>(".frame-sample")].flatMap((sample) =>
      [
        ["name", ".mtg-title", ".mtg-name"],
        ["type", ".mtg-type span", ".mtg-type"],
        ["rules", ".mtg-rules p", ".mtg-box"],
      ].flatMap(([part, text, ground]) => {
        const ink = sample.querySelector(text);
        const plate = sample.querySelector(ground);
        if (!ink || !plate) return [];
        return [{ label: `${part} on the ${sample.dataset.frame} frame`, ink: getComputedStyle(ink).color, ground: getComputedStyle(plate).backgroundColor }];
      }),
    ),
  );
  expect(pairs.length).toBeGreaterThanOrEqual(FRAMES.length * 3 - 1);
  for (const { label, ink, ground } of pairs) {
    expect(contrast(ink, ground), label).toBeGreaterThanOrEqual(7);
  }
});

test("the collector line is clear of the P/T box, and of the frame where the border is", async ({ page }) => {
  const edgeless = template === "borderless" || template === "full-art";
  const overlaps = await page.evaluate((edgelessCard) =>
    [...document.querySelectorAll(".frame-sample")].flatMap((sample) => {
      const frame = sample.querySelector(".mtg-frame")!.getBoundingClientRect();
      const foot = sample.querySelector(".mtg-foot")!.getBoundingClientRect();
      const pt = sample.querySelector(".mtg-pt")?.getBoundingClientRect();
      const problems: string[] = [];
      const card = sample.querySelector(".mtg-card")!.getBoundingClientRect();
      if (edgelessCard ? foot.bottom > card.bottom + 0.5 : foot.top < frame.bottom - 0.5) problems.push(`${(sample as HTMLElement).dataset.frame}: footer inside the frame`);
      if (pt && foot.right > pt.left && foot.bottom > pt.top) problems.push(`${(sample as HTMLElement).dataset.frame}: footer under the P/T box`);
      return problems;
    }),
    edgeless,
  );
  expect(overlaps).toEqual([]);
});

test("the frames keep their look", async ({ page }) => {
  await expectLayout(page, `frames-${template}`, [
    ".frame-sample:first-child .mtg-card",
    ".frame-sample:first-child .mtg-frame",
    ".frame-sample:first-child .mtg-name",
    ".frame-sample:first-child .mtg-art",
    ".frame-sample:first-child .mtg-type",
    ".frame-sample:first-child .mtg-box",
    ".frame-sample:first-child .mtg-pt",
    ".frame-sample:first-child .mtg-foot",
  ]);
  await expectImage(page.locator(".frames-grid"), `frames-${template}.png`);
});

});
}
