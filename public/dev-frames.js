// Every frame the cards can wear, side by side. Reached at /dev/frames (a development build only); it is where
// a frame is designed and what its regression test looks at.
import { html, render } from "/public/vendor/htm-preact.js";
import { CardFace } from "/public/card-face.js";
import { frameOf } from "/public/frame.js";

/** A card from the API always has every field; the samples only write what they need. */
function withDefaults(card) {
  return { art: null, flavor_text: "", ...card };
}

const BY_COLOUR = [
  { features: ["cost", "power_toughness"], name: "Dawnlit Paladin", mana_cost: "{1}{W}", type_line: "Creature — Human Knight", rarity: "common", rules_text: "Vigilance", power_toughness: "2/2", artist: "A. Painter" },
  { features: ["cost", "power_toughness"], name: "Mist Warden", mana_cost: "{1}{U}", type_line: "Creature — Merfolk Wizard", rarity: "uncommon", rules_text: "Flying\n{T}: Draw a card.", flavor_text: "It sees the tide before the moon does.", power_toughness: "2/3", artist: "A. Painter" },
  { features: ["cost", "power_toughness"], name: "Gravewake Fiend", mana_cost: "{2}{B}{B}", type_line: "Creature — Horror", rarity: "rare", rules_text: "Deathtouch\nWhen Gravewake Fiend dies, each opponent loses 2 life.", power_toughness: "3/3", artist: "A. Painter" },
  { features: ["cost"], name: "Ember Spark", mana_cost: "{R}", type_line: "Instant", rarity: "common", rules_text: "Ember Spark deals 2 damage to any target.", artist: "A. Painter" },
  { features: ["cost", "power_toughness"], name: "Thornback Elk", mana_cost: "{3}{G}", type_line: "Creature — Elk Beast", rarity: "mythic", rules_text: "Reach\n{G}: Thornback Elk gets +1/+1 until end of turn.", power_toughness: "4/4", artist: "A. Painter" },
  { features: ["cost"], name: "Gilded Pact", mana_cost: "{W}{B}", type_line: "Enchantment", rarity: "rare", rules_text: "At the beginning of your upkeep, each opponent loses 1 life and you gain 1 life.", artist: "A. Painter" },
  { features: ["cost"], name: "Tideglass Compass", mana_cost: "{2}", type_line: "Artifact", rarity: "uncommon", rules_text: "{T}: Add one mana of any color.", artist: "A. Painter" },
  { features: ["cost"], name: "Verdant Crossing", mana_cost: "", type_line: "Land", rarity: "common", rules_text: "{T}: Add {G}.", artist: "A. Painter" },
  { features: ["cost", "power_toughness"], name: "Void Wanderer", mana_cost: "{6}", type_line: "Creature — Eldrazi", rarity: "mythic", rules_text: "Trample", power_toughness: "6/6", artist: "A. Painter" },
].map(withDefaults);

// `?layouts=1` shows one card of each layout instead of one per frame colour.
const LAYOUTS = [
  { name: "Munchkin, Soft-Feathered Owl", mana_cost: "{2}{W}{W}{W}", type_line: "Legendary Planeswalker — Bird", features: ["cost", "loyalty"], loyalty: "4", rarity: "mythic", rules_text: "+1: Add {B}{B}\n0: Draw a card\n\u22127: Create five 2/2 white Owl Soldier tokens with haste, draw a card, add {W}{W}", artist: "A. Painter" },
  { name: "Mermiac, Ridge-Watcher", mana_cost: "{W}{U}{B}{R}{G}", type_line: "Legendary Planeswalker — Mermiac", features: ["cost", "loyalty"], loyalty: "5", rarity: "mythic", rules_text: "+2: Draw a card, then scry 1\n0: Create two 0/1 Myr artifact creature tokens\n-6: Create a copy of each Myr you control.\n: Mermiac can be your commander.", artist: "A. Painter" },
  { name: "Jorgan, Sire Of Truth", mana_cost: "{4}{R}{G}", type_line: "Enchantment — Saga", features: ["cost"], template: "saga", rarity: "uncommon", rules_text: ": (As this saga enters and after your draw step, add a lore counter. Sacrifice after V.)\nI: Create an emblem with no maximum hand size.\nII: Creatures you control have double strike until end of turn.\nIII, IV: Draw 3 cards, discard 1 card.\nV: Draw cards equal to the creatures you control.", artist: "A. Painter" },
  { name: "Dog", mana_cost: "", type_line: "Token Creature — Dog", features: ["power_toughness"], template: "token", rarity: "common", rules_text: "Haste", power_toughness: "1/1", artist: "A. Painter" },
  { name: "Elemental Of Cuteness", mana_cost: "", type_line: "Creature token — Elemental", features: ["power_toughness"], template: "token", rarity: "mythic", rules_text: "Trample, Haste\n{G}: Elemental Of Cuteness gains +1/+0 until end of turn.", power_toughness: "3/2", artist: "A. Painter" },
].map(withDefaults);

const query = new URLSearchParams(location.search);
const SAMPLES = query.get("layouts") ? LAYOUTS : BY_COLOUR;

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
