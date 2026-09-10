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
    CardIdent, CardTable, CardVars, CombatId, Cost, Creature, FlagSet, Playable, Preview, Rules,
    SideKinds, TargetKinds, UnplayableReasonKinds,
};

use super::Error;

/// Every card in hand, in hand order, with the reasons each cannot be played.
pub(crate) fn hand<V: Rules>(
    hand: &[CardIdent<V>],
    creatures: &[Creature<V>],
    energy: V::Count,
    stars: V::Count,
) -> Result<Vec<Playable<V>>, Error> {
    hand.iter()
        .map(|card| playable::<V>(card, creatures, energy, stars))
        .collect()
}

fn playable<V: Rules>(
    card: &CardIdent<V>,
    creatures: &[Creature<V>],
    energy: V::Count,
    stars: V::Count,
) -> Result<Playable<V>, Error> {
    if card.upgrade != 0 {
        // An upgraded card prints different numbers, and the tables carry the
        // unupgraded ones. See `OnUpgrade` on the card.
        unimplemented!("what an upgraded card prints");
    }
    if card.affliction.is_some() || card.enchantment.is_some() {
        unimplemented!("what an afflicted or enchanted card prints");
    }

    // One of the four cards that work their target out from the fight.
    let target = card
        .card
        .target()
        .unwrap_or_else(|| unimplemented!("a card that decides its own target"));
    let cost = card.card.cost();
    let star_cost = card.card.star_cost();

    let mut reasons = Vec::new();
    if card.card.unplayable() {
        reasons.push(<V::UnplayableReason as UnplayableReasonKinds>::HAS_UNPLAYABLE_KEYWORD);
    }
    if spend(cost, energy).is_none_or(|spent| spent > energy) {
        reasons.push(<V::UnplayableReason as UnplayableReasonKinds>::ENERGY_COST_TOO_HIGH);
    }
    if spend(star_cost, stars).is_none_or(|spent| spent > stars) {
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

    // `CanPlayTargeting` is a valid target *and* a card that can be played, so
    // a card refused for any reason names nobody, rather than naming who it
    // was going to hit.
    let targets: Vec<CombatId> = if reasons.is_empty() {
        creatures
            .iter()
            .filter(|creature| valid_target::<V>(target, creature))
            .map(|creature| creature.id)
            .collect()
    } else {
        Vec::new()
    };

    Ok(Playable {
        card: *card,
        cost,
        star_cost,
        target,
        reason: (!reasons.is_empty())
            .then(|| <V::UnplayableReasons as FlagSet>::from_members(&reasons)),
        // `BlockedByHook` is the only reason that names one, and nothing in a
        // fight raises it yet.
        preventer: None,
        effect: effect::<V>(card, &targets)?,
        targets,
    })
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
/// target. Identical previews collapse into one with no target.
///
/// Nothing modifies a card's numbers yet, so the collapse always fires. It is
/// written as the collapse anyway. The first hook to move a number then
/// reports per target, and this does not change.
fn effect<V: Rules>(card: &CardIdent<V>, targets: &[CombatId]) -> Result<Vec<Preview<V>>, Error> {
    let per_target = matches!(
        card.card.target(),
        Some(t) if t == <V::TargetType as TargetKinds>::ANY_ENEMY
            || t == <V::TargetType as TargetKinds>::ANY_ALLY
            || t == <V::TargetType as TargetKinds>::ANY_PLAYER
    );

    let numbers = numbers::<V>(card)?;
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
            vars: numbers.clone(),
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
    Ok(previews)
}

/// The numbers the card prints, as they resolve right now.
fn numbers<V: Rules>(
    card: &CardIdent<V>,
) -> Result<std::collections::BTreeMap<V::CardVar, V::Number>, Error> {
    card.card
        .vars()
        .iter()
        .map(|var| {
            if var.calculated {
                // A `CalculatedVar` counts something in the live fight.
                unimplemented!("the {} this card prints", var.id.name());
            }
            // A build can store exact fractions where this slice only
            // evaluates whole-number effects.
            let base = num_traits::NumCast::from(var.base).unwrap_or_else(|| {
                unimplemented!("the fractional {} this card prints", var.id.name())
            });
            Ok((var.id, base))
        })
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

    type V = V0_107_1;

    fn card(card: Cards) -> CardIdent<V> {
        CardIdent::new(card)
    }

    /// A player and one living enemy, which is the smallest fight.
    fn fight() -> Vec<Creature<V>> {
        vec![
            creature(
                CombatId::new(0).expect("a creature id"),
                ModelId::Character(Character::Regent),
                CombatSide::Player,
            ),
            creature(
                CombatId::new(1).expect("a creature id"),
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
        hand: &[CardIdent<V>],
        energy: <V as sts2_core::ContentVersion>::Count,
        stars: <V as sts2_core::ContentVersion>::Count,
    ) -> Playable<V> {
        let mut plays = super::hand::<V>(hand, &fight(), energy, stars).expect("the hand is known");
        assert_eq!(plays.len(), 1);
        plays.remove(0)
    }

    #[test]
    fn each_empty_pool_names_the_pool_it_is_empty_from() {
        let strike = card(Cards::Regent(RegentCard::Strike));
        let falling_star = card(Cards::Regent(RegentCard::FallingStar));

        assert_eq!(only(&[strike], 1, 0).reason, None);
        assert_eq!(
            only(&[strike], 0, 0).reason,
            Some(UnplayableReasons::new(&[
                UnplayableReason::EnergyCostTooHigh
            ]))
        );
        // Falling Star costs no energy and two stars, so an empty energy pool
        // is not what stops it.
        assert_eq!(only(&[falling_star], 0, 2).reason, None);
        assert_eq!(
            only(&[falling_star], 0, 1).reason,
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

        let plays = super::hand::<V>(&[strike], &creatures, 3, 0).expect("the hand is known");

        assert!(plays[0].targets.is_empty());
        // Nothing to hit is not a reason the card cannot be played. The game
        // refuses the play when it is pointed at nothing.
        assert_eq!(plays[0].reason, None);
    }
}
