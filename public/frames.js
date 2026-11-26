// Every frame the cards can wear, side by side. Reached at /frames; it is where
// a frame is designed and what its regression test looks at.
import { html, render } from "/public/vendor/htm-preact.js";
import { CardFace } from "/public/card-face.js";
import { frameOf } from "/public/frame.js";

/** A card from the API always has every field; the samples only write what they need. */
function withDefaults(card) {
  return { art: null, flavor_text: "", ...card };
}

const SAMPLES = [
  { name: "Dawnlit Paladin", mana_cost: "{1}{W}", type_line: "Creature — Human Knight", rarity: "common", rules_text: "Vigilance", power_toughness: "2/2", artist: "A. Painter" },
  { name: "Mist Warden", mana_cost: "{1}{U}", type_line: "Creature — Merfolk Wizard", rarity: "uncommon", rules_text: "Flying\n{T}: Draw a card.", flavor_text: "It sees the tide before the moon does.", power_toughness: "2/3", artist: "A. Painter" },
  { name: "Gravewake Fiend", mana_cost: "{2}{B}{B}", type_line: "Creature — Horror", rarity: "rare", rules_text: "Deathtouch\nWhen Gravewake Fiend dies, each opponent loses 2 life.", power_toughness: "3/3", artist: "A. Painter" },
  { name: "Ember Spark", mana_cost: "{R}", type_line: "Instant", rarity: "common", rules_text: "Ember Spark deals 2 damage to any target.", artist: "A. Painter" },
  { name: "Thornback Elk", mana_cost: "{3}{G}", type_line: "Creature — Elk Beast", rarity: "mythic", rules_text: "Reach\n{G}: Thornback Elk gets +1/+1 until end of turn.", power_toughness: "4/4", artist: "A. Painter" },
  { name: "Gilded Pact", mana_cost: "{W}{B}", type_line: "Enchantment", rarity: "rare", rules_text: "At the beginning of your upkeep, each opponent loses 1 life and you gain 1 life.", artist: "A. Painter" },
  { name: "Tideglass Compass", mana_cost: "{2}", type_line: "Artifact", rarity: "uncommon", rules_text: "{T}: Add one mana of any color.", artist: "A. Painter" },
  { name: "Verdant Crossing", mana_cost: "", type_line: "Land", rarity: "common", rules_text: "{T}: Add {G}.", artist: "A. Painter" },
  { name: "Void Wanderer", mana_cost: "{6}", type_line: "Creature — Eldrazi", rarity: "mythic", rules_text: "Trample", power_toughness: "6/6", artist: "A. Painter" },
].map(withDefaults);

function Frames() {
  return html`
    <h1>Frames</h1>
    <p class="muted">One card per frame the editor can pick, chosen by the mana cost and type line.</p>
    <div class="frames-grid">
      ${SAMPLES.map((card, index) => html`<figure class="frame-sample" data-frame=${frameOf(card)}>
        <${CardFace} card=${card} code="DSR" number=${index + 1} total=${SAMPLES.length} />
        <figcaption>${frameOf(card)} · ${card.rarity}</figcaption>
      </figure>`)}
    </div>
  `;
}

render(html`<${Frames} />`, document.getElementById("app"));
