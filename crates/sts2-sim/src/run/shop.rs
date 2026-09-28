//! What a merchant has on its shelves, and what the prices come to.
//!
//! `MerchantInventory.CreateForNormalMerchant`, in the order it fills them:
//! five cards from the character's pool by type, two from the colourless pool
//! by rarity, three relics, three potions, and the card removal.
//!
//! Two streams pay for it. Shops pays for every pick and every haggle, and
//! Rewards pays for the rarity rolls and for the upgrade roll a merchant card
//! never passes. The order below is the order they are drawn in.

use sts2_core::rng::{Bound, GameRng, Interval, Span};
use sts2_core::{
    CardIdent, CardKind, CardTable as _, Entry, PlayerStreamKinds, RelicTable as _, RewardRarity,
    Rules, ShopCard, Stock,
};

use super::Rarity;
use super::setup::RelicBag;
use crate::rng::MegaRng;
use crate::stream::Streams;

/// The merchant, and everything it reads to stock a shelf.
///
/// A borrow of the run's own pieces rather than of the run. Travelling into a
/// room works on copies and commits them at the end. So a shop that cannot be
/// stocked leaves the run where it stood.
pub(super) struct Merchant<'a, V: Rules> {
    pub(super) streams: &'a mut Streams<V>,
    pub(super) player_relics: &'a RelicBag<V>,
    pub(super) dealt_relics: &'a mut Vec<V::Relic>,
    pub(super) character: V::Character,
    /// The saved card rarity offset, which a shop reads and does not move.
    pub(super) offset: f32,
    /// `CardShopRemovalsUsed`, which this merchant's removal price opens at.
    pub(super) removals_used: usize,
}

/// `_coloredCardTypes`, which is what the five character slots sell.
const SHELVES: [CardKind; 5] = [
    CardKind::Attack,
    CardKind::Attack,
    CardKind::Skill,
    CardKind::Skill,
    CardKind::Power,
];

/// `_colorlessCardRarities`, which is what the two colourless slots sell.
const COLORLESS: [RewardRarity; 2] = [RewardRarity::Uncommon, RewardRarity::Rare];

/// `CardRarityOddsType.Shop`, whose rare share the saved offset moves.
const SHOP_RARE: f32 = 0.09;
const SHOP_UNCOMMON: f32 = 0.37;

/// `MerchantCardEntry.GetCost`, before the haggle.
const RARE_CARD: i32 = 150;
const UNCOMMON_CARD: i32 = 75;
const COMMON_CARD: i32 = 50;

/// What the colourless shelf adds to a card's price.
const COLORLESS_MARKUP: f32 = 1.15;

/// `MerchantCardRemovalEntry`'s opening price and what each removal adds.
const REMOVAL_BASE: i32 = 75;
const REMOVAL_STEP: i32 = 25;

/// `MerchantPotionEntry.GetCost`, which is flat.
const RARE_POTION: i32 = 100;
const UNCOMMON_POTION: i32 = 75;
const COMMON_POTION: i32 = 50;

/// How far a card's or a potion's price can wander.
const CARD_SPREAD: (f32, f32) = (0.95, 1.05);

/// The same for a relic, which wanders further.
const RELIC_SPREAD: (f32, f32) = (0.85, 1.15);

impl<V: Rules> Merchant<'_, V> {
    /// Everything one merchant has for sale, in the order it stocks them.
    ///
    /// The indices are the order `AllEntries` walks: the cards, then the
    /// relics, then the potions, then the removal. A decision names a slot by
    /// that index.
    pub(super) fn stock(&mut self) -> Vec<Entry<V>> {
        let mut cards = self.card_shelves();
        cards.extend(self.colorless_shelves());
        let relics = self.relic_shelves();
        let potions = self.potion_shelves();

        let mut entries = Vec::new();
        for (card, cost) in cards {
            entries.push(self.entry(
                Stock::Card(Some(ShopCard {
                    card,
                    // A merchant's card is never upgraded, so what it costs to
                    // play is what its row prints.
                    cost: card.card.cost(),
                    star_cost: card.card.star_cost(),
                })),
                cost,
                entries.len(),
            ));
        }
        for (relic, cost) in relics {
            entries.push(self.entry(Stock::Relic(Some(relic)), cost, entries.len()));
        }
        for (potion, cost) in potions {
            entries.push(self.entry(Stock::Potion(Some(potion)), cost, entries.len()));
        }
        let removal = removal_cost(self.removals_used);
        entries.push(self.entry(Stock::CardRemoval, removal, entries.len()));
        entries
    }

    /// `RestockAfterPurchase`: what shelf `index` holds once a purchase
    /// refilled it, and its price. `on_shelves` is every card the merchant
    /// shows, the one just bought included.
    ///
    /// A card shelf is `Populate` again, off sale, leaving those out. A relic
    /// shelf rolls a rarity and pulls from the back of the bag. A potion
    /// shelf draws one potion, with the blacklist it builds never passed.
    pub(super) fn restock(
        &mut self,
        index: usize,
        stock: Stock<V>,
        on_shelves: &[V::Card],
    ) -> (Stock<V>, i32) {
        match stock {
            Stock::Card(_) => {
                let (card, cost) = match SHELVES.get(index) {
                    Some(&kind) => {
                        let rarity = self.rolled_rarity();
                        let rarity = self.allowed_rarity(kind, rarity, on_shelves);
                        let card =
                            self.pick(V::merchant_cards(self.character, kind, rarity), on_shelves);
                        (card, self.haggled(card_cost(rarity, false), CARD_SPREAD))
                    }
                    None => {
                        let rarity = COLORLESS[index - SHELVES.len()];
                        let card = self.pick(V::merchant_colorless_cards(rarity), on_shelves);
                        (card, self.haggled(card_cost(rarity, true), CARD_SPREAD))
                    }
                };
                let card: CardIdent<V> = CardIdent::new(card);
                let shelf = ShopCard {
                    card,
                    cost: card.card.cost(),
                    star_cost: card.card.star_cost(),
                };
                (Stock::Card(Some(shelf)), cost)
            }
            Stock::Relic(_) => {
                let rarity = self.rolled_relic_rarity();
                let relic =
                    self.player_relics
                        .back(rarity, self.dealt_relics, V::Relic::allowed_in_shops);
                self.dealt_relics.push(relic);
                let base = relic
                    .merchant_price()
                    .unwrap_or_else(|| unimplemented!("a merchant stocking {relic:?}"));
                (
                    Stock::Relic(Some(relic)),
                    self.haggled(i32::from(base), RELIC_SPREAD),
                )
            }
            Stock::Potion(_) => {
                let character = self.character;
                let [potion] = crate::reward::potion::distinct::<V, MegaRng>(
                    self.shops(),
                    character,
                    1,
                    false,
                )[..] else {
                    unreachable!("one draw returns one potion")
                };
                (
                    Stock::Potion(Some(potion)),
                    potion_cost::<V>(character, potion),
                )
            }
            Stock::CardRemoval => unreachable!("the card removal never restocks"),
        }
    }

    /// One shelf, priced and numbered. What the run can afford is derived.
    fn entry(&self, stock: Stock<V>, cost: i32, index: usize) -> Entry<V> {
        Entry {
            index: u8::try_from(index).expect("a merchant has fewer shelves than a byte counts"),
            stock,
            cost: num_traits::cast(cost).expect("a merchant's price fits the build's number"),
            title: None,
            text: None,
            stocked: true,
            affordable: false,
            blocked: None,
        }
    }

    /// The five slots the character's own pool fills, one of them on sale.
    fn card_shelves(&mut self) -> Vec<(CardIdent<V>, i32)> {
        let bound = Bound::try_new(
            num_traits::cast(SHELVES.len()).expect("a shelf count fits the RNG integer"),
        )
        .expect("a merchant has shelves");
        let on_sale: usize = num_traits::cast(self.shops().next_int(bound))
            .expect("a bounded draw is a valid shelf index");

        let mut stocked = Vec::new();
        let mut picked: Vec<V::Card> = Vec::new();
        for (at, kind) in SHELVES.into_iter().enumerate() {
            let rarity = self.rolled_rarity();
            let rarity = self.allowed_rarity(kind, rarity, &picked);
            let card = self.pick(V::merchant_cards(self.character, kind, rarity), &picked);
            picked.push(card);
            let base = card_cost(rarity, false);
            let mut cost = self.haggled(base, CARD_SPREAD);
            if at == on_sale {
                // `SetOnSale` calls `CalcCost` again rather than halve what it
                // had, so the slot on sale costs one draw more than its
                // neighbours.
                cost = self.haggled(base, CARD_SPREAD) / 2;
            }
            stocked.push((CardIdent::new(card), cost));
        }
        stocked
    }

    /// The two slots the colourless pool fills, whose rarity is not rolled.
    ///
    /// The picked list starts empty here, although `Populate` reads every card
    /// entry. The character shelves hold no colourless card, so this pool holds
    /// nothing that entry excludes.
    fn colorless_shelves(&mut self) -> Vec<(CardIdent<V>, i32)> {
        let mut stocked = Vec::new();
        let mut picked: Vec<V::Card> = Vec::new();
        for rarity in COLORLESS {
            let card = self.pick(V::merchant_colorless_cards(rarity), &picked);
            picked.push(card);
            let cost = self.haggled(card_cost(rarity, true), CARD_SPREAD);
            stocked.push((CardIdent::new(card), cost));
        }
        stocked
    }

    /// One card off a shelf, and the upgrade roll that follows it.
    ///
    /// `CreateForMerchant` rolls for an upgrade against a chance of minus a
    /// billion, so the draw happens and never lands.
    fn pick(&mut self, options: Vec<V::Card>, picked: &[V::Card]) -> V::Card {
        let options: Vec<V::Card> = options
            .into_iter()
            .filter(|card| !picked.contains(card))
            .collect();
        let count = num_traits::cast(options.len()).expect("a shelf fits the RNG integer");
        let span = Span::new(num_traits::zero(), count).expect("a shelf has something on it");
        let at: usize = num_traits::cast(self.shops().next_int_range(span))
            .expect("a bounded draw is a valid card index");
        let card = options[at];
        self.rewards().next_double();
        card
    }

    /// `RollWithoutChangingFutureOdds(Shop)`, which reads the saved offset and
    /// leaves it where it was.
    fn rolled_rarity(&mut self) -> RewardRarity {
        let rare = SHOP_RARE + self.offset;
        let roll = self.rewards().next_double() as f32;
        if roll < rare {
            RewardRarity::Rare
        } else if roll < SHOP_UNCOMMON + rare {
            RewardRarity::Uncommon
        } else {
            RewardRarity::Common
        }
    }

    /// `GetNextAllowedRarity`: walk on while the shelf has nothing at this one.
    ///
    /// The walk wraps through the three drafted rarities and stops when it
    /// comes back where it started.
    fn allowed_rarity(
        &self,
        kind: CardKind,
        rarity: RewardRarity,
        picked: &[V::Card],
    ) -> RewardRarity {
        let mut wanted = rarity;
        for _ in 0..3 {
            let held = V::merchant_cards(self.character, kind, wanted)
                .into_iter()
                .any(|card| !picked.contains(&card));
            if held {
                return wanted;
            }
            wanted = match wanted {
                RewardRarity::Common => RewardRarity::Uncommon,
                RewardRarity::Uncommon => RewardRarity::Rare,
                _ => RewardRarity::Common,
            };
        }
        unimplemented!("a merchant shelf with nothing at any rarity")
    }

    /// The three relic slots: two at a rolled rarity, then the shop's own.
    fn relic_shelves(&mut self) -> Vec<(V::Relic, i32)> {
        // The three rarities are worked out before any of them is filled,
        // because the game builds the array first and walks it afterwards.
        let rarities = [
            self.rolled_relic_rarity(),
            self.rolled_relic_rarity(),
            <Rarity<V> as sts2_core::RarityKinds>::SHOP,
        ];

        let mut stocked = Vec::new();
        for rarity in rarities {
            // `PullNextRelicFromBack` reads the player's own bag and then takes
            // what it drew out of the shared one, so a relic sold here is gone
            // from both. The dealt list is what both bags are read against.
            let relic =
                self.player_relics
                    .back(rarity, self.dealt_relics, V::Relic::allowed_in_shops);
            self.dealt_relics.push(relic);
            let base = relic
                .merchant_price()
                .unwrap_or_else(|| unimplemented!("a merchant stocking {relic:?}"));
            let cost = self.haggled(i32::from(base), RELIC_SPREAD);
            stocked.push((relic, cost));
        }
        stocked
    }

    /// The three potion slots, which are three distinct potions.
    ///
    /// `CalcCost` haggles a potion's price only with `TestMode.IsOff`, and the
    /// oracle runs headless with it on. So a potion's price is its rarity's
    /// number and no draw pays for it here.
    fn potion_shelves(&mut self) -> Vec<(V::Potion, i32)> {
        let character = self.character;
        crate::reward::potion::distinct::<V, MegaRng>(self.shops(), character, 3, false)
            .into_iter()
            .map(|potion| (potion, potion_cost::<V>(character, potion)))
            .collect()
    }

    /// `RelicFactory.RollRarity`, which reads the Rewards stream.
    fn rolled_relic_rarity(&mut self) -> Rarity<V> {
        crate::reward::relic_rarity::<V, _>(self.rewards())
    }

    /// `CalcCost`'s haggle: the base times a draw inside the spread, rounded.
    fn haggled(&mut self, base: i32, spread: (f32, f32)) -> i32 {
        let interval = Interval::new(spread.0, spread.1);
        let scale = self.shops().next_f32_range(interval);
        rounded(base as f32 * scale)
    }

    fn shops(&mut self) -> &mut MegaRng {
        self.streams
            .player(<V::PlayerRngType as PlayerStreamKinds>::SHOPS)
    }

    fn rewards(&mut self) -> &mut MegaRng {
        self.streams
            .player(<V::PlayerRngType as PlayerStreamKinds>::REWARDS)
    }
}

/// `MerchantCardEntry.GetCost`, which the colourless shelf marks up.
fn card_cost(rarity: RewardRarity, colorless: bool) -> i32 {
    let base = match rarity {
        RewardRarity::Rare => RARE_CARD,
        RewardRarity::Uncommon => UNCOMMON_CARD,
        _ => COMMON_CARD,
    };
    match colorless {
        true => rounded(base as f32 * COLORLESS_MARKUP),
        false => base,
    }
}

/// `MerchantPotionEntry.GetCost`, which reads the potion's rarity alone.
fn potion_cost<V: Rules>(character: V::Character, potion: V::Potion) -> i32 {
    match crate::reward::potion::rarity_of::<V>(character, potion) {
        RewardRarity::Rare => RARE_POTION,
        RewardRarity::Uncommon => UNCOMMON_POTION,
        _ => COMMON_POTION,
    }
}

/// `MerchantCardRemovalEntry.GetCost`, which climbs with every removal bought.
pub(super) fn removal_cost(used: usize) -> i32 {
    REMOVAL_BASE + REMOVAL_STEP * i32::try_from(used).expect("a run buys fewer removals than that")
}

/// `Mathf.RoundToInt`, which is `Math.Round` and so rounds a half to even.
///
/// The markup on a rare colourless card is exactly a half. Rounding it the
/// other way puts every price on that shelf one gold out.
fn rounded(value: f32) -> i32 {
    value.round_ties_even() as i32
}
