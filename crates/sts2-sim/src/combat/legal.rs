//! Which cards in hand can be played, and what each one does.
//!
//! `CardModel.CanPlay` starts at no reasons and adds one per rule it fails.
//! So this reports a set, rather than the first refusal.
//!
//! Two of the game's six rules are not decided here. `BlockedByHook` is what
//! every listener in the fight says. `BlockedByCardLogic` is the card's own
//! `IsPlayable` override. Nothing raises either one yet: this fight has no
//! powers, no relic that listens, and no card that overrides it.

use sts2_core::{
    CardIdent, CardTable, Combat, CombatId, Cost, Creature, EffectResolution, FlagSet, Held,
    Playable, PotionTable, PotionUsage, Preview, Rules, SideKinds, TargetKinds, Undrinkable,
    UnplayableReasonKinds,
};

use super::live::CardInstance;

/// Every card in hand, in hand order, with the reasons each cannot be played.
pub(super) fn hand<V: Rules>(
    hand: &[CardInstance<V>],
    creatures: &[Creature<V>],
    energy: V::Count,
    stars: V::Count,
) -> Vec<Playable<V>> {
    hand.iter()
        .map(|card| playable::<V>(card, creatures, energy, stars))
        .collect()
}

/// What a card names as its target.
pub(super) fn target_type<V: Rules>(
    card: CardIdent<V>,
    creatures: &[Creature<V>],
) -> V::TargetType {
    if card.affliction.is_some() || card.enchantment.is_some() {
        unimplemented!("what an afflicted or enchanted card prints");
    }
    // One of the four cards that work their target out from the fight, which
    // they do by reading the player's own powers.
    card.card
        .target()
        .unwrap_or_else(|| V::computed_card_target(card.card, super::board::powers::<V>(creatures)))
}

/// Why this card cannot be played, which is empty for one that can.
pub(super) fn reasons<V: Rules>(
    card: CardIdent<V>,
    creatures: &[Creature<V>],
    energy: V::Count,
    stars: V::Count,
) -> Vec<V::UnplayableReason> {
    let target = target_type::<V>(card, creatures);
    let mut reasons = Vec::new();
    if card.card.unplayable() {
        reasons.push(<V::UnplayableReason as UnplayableReasonKinds>::HAS_UNPLAYABLE_KEYWORD);
    }
    if spend(card.card.cost(), energy).is_none_or(|spent| spent > energy) {
        reasons.push(<V::UnplayableReason as UnplayableReasonKinds>::ENERGY_COST_TOO_HIGH);
    }
    if spend(card.card.star_cost(), stars).is_none_or(|spent| spent > stars) {
        reasons.push(<V::UnplayableReason as UnplayableReasonKinds>::STAR_COST_TOO_HIGH);
    }
    // One player creature is the player's own, so a card pointed at an ally
    // needs a second one alive.
    if target == <V::TargetType as TargetKinds>::ANY_ALLY
        && creatures
            .iter()
            .filter(|c| c.side == <V::CombatSide as SideKinds>::PLAYER && c.alive)
            .count()
            <= 1
    {
        reasons.push(<V::UnplayableReason as UnplayableReasonKinds>::NO_LIVING_ALLIES);
    }
    reasons
}

/// The creatures this card can be pointed at right now.
///
/// `CanPlayTargeting` is a valid target *and* a card that can be played. So a
/// card refused for any reason names nobody. It does not name who it was going
/// to hit.
pub(super) fn targets<V: Rules>(
    target: V::TargetType,
    creatures: &[Creature<V>],
    refused: bool,
) -> Vec<CombatId> {
    if refused {
        return Vec::new();
    }
    creatures
        .iter()
        .filter(|creature| valid_target::<V>(target, creature))
        .map(|creature| creature.id)
        .collect()
}

fn playable<V: Rules>(
    held: &CardInstance<V>,
    creatures: &[Creature<V>],
    energy: V::Count,
    stars: V::Count,
) -> Playable<V> {
    let card = &held.ident;
    let target = target_type::<V>(*card, creatures);
    let cost = card.card.cost();
    let star_cost = card.card.star_cost();
    let reasons = reasons::<V>(*card, creatures, energy, stars);
    let targets = targets::<V>(target, creatures, !reasons.is_empty());

    Playable {
        card: *card,
        cost,
        star_cost,
        target,
        reason: (!reasons.is_empty())
            .then(|| <V::UnplayableReasons as FlagSet>::from_members(&reasons)),
        // `BlockedByHook` is the only reason that names one, and nothing in a
        // fight raises it yet.
        preventer: None,
        effect: effect::<V>(held, creatures, &targets),
        targets,
    }
}

/// The belt, and why each potion in it cannot be drunk.
///
/// The order `NPotionPopup.RefreshButtons` asks in, minus the questions this
/// slice cannot raise. Nothing is queued, because a decision resolves whole.
/// Nothing is stuck, because that is a relic and none is ported. The player is
/// alive, because a dead one stops the run.
pub(crate) fn belt<V: Rules>(potions: &[V::Potion], combat: Option<&Combat<V>>) -> Vec<Held<V>> {
    potions
        .iter()
        .map(|&potion| {
            let reason = undrinkable::<V>(potion, combat);
            Held {
                potion,
                // The game reads both out of its localisation tables, which
                // the simulator does not have.
                title: None,
                text: None,
                targets: match (reason, combat) {
                    (None, Some(combat)) => aimable::<V>(potion, &combat.creatures),
                    // A potion nobody can drink names nobody, and one drunk
                    // outside a fight has no creature to name.
                    _ => Vec::new(),
                },
                reason,
            }
        })
        .collect()
}

/// Why one potion cannot be drunk, or nothing for one that can.
fn undrinkable<V: Rules>(potion: V::Potion, combat: Option<&Combat<V>>) -> Option<Undrinkable> {
    if potion.usage() == PotionUsage::Automatic {
        return Some(Undrinkable::Automatic);
    }
    match combat {
        None if potion.usage() == PotionUsage::CombatOnly => return Some(Undrinkable::OutOfCombat),
        None => {}
        Some(combat) if combat.phase != <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::PLAY => {
            return Some(Undrinkable::NotYourTurn);
        }
        Some(_) => {}
    }
    match potion.passes_own_check() {
        Some(true) => None,
        Some(false) => Some(Undrinkable::ItsOwnCheck),
        None => unimplemented!("a potion that decides for itself whether it can be drunk"),
    }
}

/// The creatures a potion can be pointed at.
fn aimable<V: Rules>(potion: V::Potion, creatures: &[Creature<V>]) -> Vec<CombatId> {
    let Some(target) = potion.target() else {
        // One of the potions that work their target out from the fight.
        unimplemented!("a potion that decides its own target");
    };
    creatures
        .iter()
        .filter(|creature| valid_target::<V>(target, creature))
        .map(|creature| creature.id)
        .collect()
}

/// What the pool it is priced in has to give up for it.
///
/// `GetAmountToSpend` clamps at zero, so a card not priced in a pool spends
/// nothing from it rather than the `-1` the game stores.
pub(super) fn spend<C: num_traits::PrimInt>(cost: Cost, pool: C) -> Option<C> {
    match cost {
        Cost::X => Some(pool),
        // Nothing at all for a price this build's count cannot hold, which
        // reads at the callsite as a cost nothing can pay.
        Cost::Fixed(n) => num_traits::cast(n),
        Cost::None => Some(C::zero()),
    }
}

/// `CardModel.IsValidTarget`, for a creature offered as this card's target.
fn valid_target<V: Rules>(target: V::TargetType, creature: &Creature<V>) -> bool {
    if !creature.alive {
        return false;
    }
    // The player is the only creature a card is played from, so "the other
    // side" and "this side" are the enemy's and the player's.
    match target {
        t if t == <V::TargetType as TargetKinds>::ANY_ENEMY => {
            creature.side == <V::CombatSide as SideKinds>::ENEMY
        }
        t if t == <V::TargetType as TargetKinds>::ANY_ALLY => {
            creature.side == <V::CombatSide as SideKinds>::PLAYER
        }
        _ => false,
    }
}

/// What the card does, once per outcome that differs.
///
/// A card that names a single creature is previewed against each legal
/// target. Identical previews collapse into one with no target, which is what
/// a card the fight moves no number of reports.
fn effect<V: Rules>(
    held: &CardInstance<V>,
    creatures: &[Creature<V>],
    targets: &[CombatId],
) -> Vec<Preview<V>> {
    let per_target = matches!(
        Some(target_type::<V>(held.ident, creatures)),
        Some(t) if t == <V::TargetType as TargetKinds>::ANY_ENEMY
            || t == <V::TargetType as TargetKinds>::ANY_ALLY
            || t == <V::TargetType as TargetKinds>::ANY_PLAYER
    );

    let numbers = numbers::<V>(held, creatures);
    let against: Vec<Option<CombatId>> = if per_target && !targets.is_empty() {
        targets.iter().copied().map(Some).collect()
    } else {
        vec![None]
    };

    let mut previews: Vec<Preview<V>> = against
        .into_iter()
        .map(|target| Preview {
            target,
            // The game formats this from its localisation tables, which the
            // simulator does not have. `Observation::rules_digest` leaves it
            // out for that reason, and a front end reads the numbers.
            text: None,
            vars: previewed::<V>(held, creatures, target),
            base: numbers.clone(),
        })
        .collect();

    if previews.len() > 1
        && previews[1..]
            .iter()
            .all(|preview| preview.vars == previews[0].vars)
    {
        previews.truncate(1);
        previews[0].target = None;
    }
    previews
}

/// The card's numbers after the fight has had its say, which is
/// `DynamicVar.UpdateCardPreview` for each of them.
///
/// Each number becomes the effect its own class names and is resolved, which
/// is the same call the card makes when it is played. Damage runs against the
/// creature the card is pointed at, and block for the player about to gain it.
///
/// # Panics
///
/// A number whose class the simulator has nowhere to run, which is the Osty's
/// damage. No character the simulator plays has one.
fn previewed<V: Rules>(
    held: &CardInstance<V>,
    creatures: &[Creature<V>],
    target: Option<CombatId>,
) -> std::collections::BTreeMap<V::CardVar, V::Number> {
    let dealer = super::board::player::<V>(creatures);
    held.vars
        .iter()
        .map(|var| {
            let resolved = var
                .modifier
                .and_then(|modifier| super::board::effect_of::<V>(modifier, var.id, dealer, target))
                .and_then(|effect| {
                    super::board::resolve::<V>(creatures, Some(held), effect).amount()
                });
            // A number no hook reaches, and one whose target has left the
            // fight, both print what the card prints.
            let value =
                resolved.unwrap_or_else(|| super::board::printed::<V>(held, var.id, creatures));
            (var.id, value)
        })
        .collect()
}

/// The numbers this copy of the card prints, as they resolve right now.
///
/// This copy's own, not the table's. A fight writes on a card, and what it
/// wrote is what the card prints afterwards. Nothing in the fight has listened
/// yet: [`previewed`] is these after it has.
fn numbers<V: Rules>(
    held: &CardInstance<V>,
    creatures: &[Creature<V>],
) -> std::collections::BTreeMap<V::CardVar, V::Number> {
    held.vars
        .iter()
        .map(|var| (var.id, super::board::printed::<V>(held, var.id, creatures)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::{
        V0_107_1,
        cards::{Cards, ColorlessCard, RegentCard, StatusCard},
        characters::Character,
        monsters::Monster,
        runstate::{CombatSide, UnplayableReason, UnplayableReasons},
    };
    use sts2_core::ModelId;
    use sts2_core::fixture::at;

    type V = V0_107_1;

    fn card(card: Cards) -> CardInstance<V> {
        CardInstance::new(CardIdent::new(card))
    }

    /// A player and one living enemy, which is the smallest fight.
    fn fight() -> Vec<Creature<V>> {
        vec![
            creature(
                at(0),
                ModelId::Character(Character::Regent),
                CombatSide::Player,
            ),
            creature(
                at(1),
                ModelId::Monster(Monster::TwigSlimeS),
                CombatSide::Enemy,
            ),
        ]
    }

    fn creature(id: CombatId, model: ModelId<V>, side: CombatSide) -> Creature<V> {
        Creature {
            id,
            model: Some(model),
            side,
            hp: 10,
            max_hp: 10,
            block: 0,
            alive: true,
            powers: Vec::new(),
            next_move: None,
        }
    }

    fn only(
        hand: &[CardInstance<V>],
        energy: <V as sts2_core::ContentVersion>::Count,
        stars: <V as sts2_core::ContentVersion>::Count,
    ) -> Playable<V> {
        let mut plays = super::hand::<V>(hand, &fight(), energy, stars);
        assert_eq!(plays.len(), 1);
        plays.remove(0)
    }

    #[test]
    fn each_empty_pool_names_the_pool_it_is_empty_from() {
        let strike = card(Cards::Regent(RegentCard::Strike));
        let falling_star = card(Cards::Regent(RegentCard::FallingStar));

        assert_eq!(only(std::slice::from_ref(&strike), 1, 0).reason, None);
        assert_eq!(
            only(std::slice::from_ref(&strike), 0, 0).reason,
            Some(UnplayableReasons::new(&[
                UnplayableReason::EnergyCostTooHigh
            ]))
        );
        // Falling Star costs no energy and two stars, so an empty energy pool
        // is not what stops it.
        assert_eq!(only(std::slice::from_ref(&falling_star), 0, 2).reason, None);
        assert_eq!(
            only(std::slice::from_ref(&falling_star), 0, 1).reason,
            Some(UnplayableReasons::new(&[UnplayableReason::StarCostTooHigh]))
        );
    }

    /// The reasons are a set, and the game adds one bit per rule failed.
    #[test]
    fn a_card_that_fails_two_rules_carries_both() {
        let coordinate = card(Cards::Colorless(ColorlessCard::Coordinate));

        let play = only(&[coordinate], 0, 0);

        assert_eq!(
            play.reason,
            Some(UnplayableReasons::new(&[
                UnplayableReason::EnergyCostTooHigh,
                UnplayableReason::NoLivingAllies,
            ]))
        );
        // An ally card in a fight with no ally names nobody it can be played
        // at, and still says what it does.
        assert!(play.targets.is_empty());
        assert_eq!(play.effect.len(), 1);
    }

    /// A status card is unplayable however much energy is left.
    #[test]
    fn the_unplayable_keyword_does_not_depend_on_a_pool() {
        let burn = card(Cards::Status(StatusCard::Burn));

        assert_eq!(
            only(&[burn], 99, 99).reason,
            Some(UnplayableReasons::new(&[
                UnplayableReason::HasUnplayableKeyword
            ]))
        );
    }

    /// A dead creature is not a target, and the game checks that first.
    #[test]
    fn a_dead_enemy_is_not_a_target() {
        let strike = card(Cards::Regent(RegentCard::Strike));
        let mut creatures = fight();
        creatures[1].alive = false;

        let plays = super::hand::<V>(&[strike], &creatures, 3, 0);

        assert!(plays[0].targets.is_empty());
        // Nothing to hit is not a reason the card cannot be played. The game
        // refuses the play when it is pointed at nothing.
        assert_eq!(plays[0].reason, None);
    }
}
