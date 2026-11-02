//! The filters a `CardSelectCmd` lists a pile through.
//!
//! The command lists the pile once its choice has signalled, so a choice that
//! waits behind another player's lists their pile as it stands then.

use sts2_core::{CardAt, CardKind, Fight, Pile};

use crate::generated::cards::CardKeyword;

/// The filter a `CardSelectCmd` lists one pile's cards through.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CardFilter<V: crate::Build> {
    /// `TouchOfInsanity`: a card that costs energy or stars, with its own
    /// modifiers or with every one.
    CostsEnergyOrStars,
    /// `DualWield`: an Attack or a Power.
    AttackOrPower,
    /// `HandTrick`: a Skill that is not Sly this turn.
    SkillNotSlyThisTurn,
    /// `SecretWeapon` and `SecretTechnique`: a card of one type.
    Kind(CardKind),
    /// `SculptingStrike` and `Snap`: a card without the keyword, from its
    /// own sources alone where `local`.
    WithoutKeyword { keyword: CardKeyword, local: bool },
    /// `FromHandForUpgrade`: a card with an upgrade left.
    Upgradable,
    /// `DecisionsDecisions`: a Skill that is not Unplayable.
    PlayableSkill,
    /// `HeirloomHammer`: a card that shows as colorless.
    Colorless,
    /// `SeekerStrike`: one of the cards it drew its options from.
    OneOf(smallvec::SmallVec<[sts2_core::CardId<V>; sts2_core::MAX_CARDS_IN_HAND]>),
}

/// The cards of `pile` that `filter` lets through, in the pile's order.
pub(crate) fn among<V: crate::Build, F: Fight<V>>(
    fight: &F,
    pile: Pile,
    filter: &CardFilter<V>,
) -> smallvec::SmallVec<[CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]> {
    fight
        .cards_in(pile)
        .into_iter()
        .filter(|&card| lets_through::<V, F>(fight, filter, card))
        .collect()
}

/// Whether `filter` lets `card` through.
fn lets_through<V: crate::Build, F: Fight<V>>(
    fight: &F,
    filter: &CardFilter<V>,
    card: CardAt<V>,
) -> bool {
    use sts2_core::CardTable as _;

    let kind = || V::card_kind(fight.card_ident(card).card);
    match filter {
        CardFilter::CostsEnergyOrStars => {
            let costs = |cost| matches!(cost, sts2_core::Cost::Fixed(cost) if cost.get() > 0);
            let (energy, stars) = fight.card_costs(card, false);
            let (all_energy, all_stars) = fight.card_costs(card, true);
            costs(energy) || costs(stars) || costs(all_energy) || costs(all_stars)
        }
        CardFilter::AttackOrPower => matches!(kind(), Some(CardKind::Attack | CardKind::Power)),
        CardFilter::SkillNotSlyThisTurn => {
            kind() == Some(CardKind::Skill) && !fight.card_keyword_this_turn(card, CardKeyword::Sly)
        }
        CardFilter::Kind(wanted) => kind() == Some(*wanted),
        CardFilter::WithoutKeyword { keyword, local } => !match local {
            true => fight.card_local_keyword(card, *keyword),
            false => fight.card_keyword(card, *keyword),
        },
        CardFilter::Upgradable => {
            let ident = fight.card_ident(card);
            ident.upgrade < ident.card.max_upgrade()
        }
        CardFilter::PlayableSkill => {
            let kind = kind().unwrap_or_else(|| {
                unimplemented!("Decisions, Decisions reading a card whose type is computed")
            });
            kind == CardKind::Skill && !fight.card_keyword(card, CardKeyword::Unplayable)
        }
        CardFilter::Colorless => fight.card_ident(card).card.visually_colorless(),
        CardFilter::OneOf(cards) => cards.contains(&card.id()),
    }
}
