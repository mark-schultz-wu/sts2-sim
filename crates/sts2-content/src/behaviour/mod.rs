//! What content does, written once for every build.
//!
//! Generic over the build, and filed the way a build's own behaviour is. A
//! rule moves here once it names content through the build. A rule a later
//! build changed stays in each build that has it.

pub(crate) mod acts;
pub(crate) mod afflictions;
pub(crate) mod cards;
pub(crate) mod enchantments;
pub(crate) mod encounters;
pub mod events;
pub(crate) mod modifiers;
pub(crate) mod monsters;
pub(crate) mod orbs;
pub(crate) mod paused;
pub(crate) mod potions;
pub(crate) mod powers;
pub(crate) mod relics;
pub(crate) mod restsite;
pub(crate) mod rules;

use sts2_core::{
    CardAt, CardKind, Change, CostPool, Fight, FightKind, Lasting, Pile, RunStreamKinds,
};

/// How far a play got in a fight of kind `K`: where it stopped, if it did.
pub type Played<V, K> = sts2_core::Played<V, Resume<V, K>>;

/// Where a rule that waits on a question resumes, under the module whose rule
/// it is, in a fight of kind `K`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resume<V: crate::Build, K: FightKind<V>> {
    Card(crate::behaviour::cards::Resume<V, K>),
    Potion(crate::behaviour::potions::Resume<V, K>),
    Monster(crate::behaviour::monsters::Resume<V, K>),
    /// A play or a move of the build's [`crate::Build::Paused`], standing
    /// still where it asked.
    Build(V::Paused<K>),
}

impl<V: crate::Build, K: FightKind<V>> From<crate::behaviour::cards::Resume<V, K>>
    for Resume<V, K>
{
    fn from(at: crate::behaviour::cards::Resume<V, K>) -> Self {
        Resume::Card(at)
    }
}

impl<V: crate::Build, K: FightKind<V>> From<crate::behaviour::monsters::Resume<V, K>>
    for Resume<V, K>
{
    fn from(at: crate::behaviour::monsters::Resume<V, K>) -> Self {
        Resume::Monster(at)
    }
}

impl<V: crate::Build, K: FightKind<V>> From<crate::behaviour::potions::Resume<V, K>>
    for Resume<V, K>
{
    fn from(at: crate::behaviour::potions::Resume<V, K>) -> Self {
        Resume::Potion(at)
    }
}

/// An option an event offers.
///
/// An ancient offers relics, and its keys name the relic. The Architect's
/// lines and a bare `PROCEED` name no page. Every other option is one its
/// page names, which the localisation table lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventOptionId<V: sts2_core::ContentVersion> {
    Relic(V::Relic),
    Said(V::EventOption),
    /// A line of The Architect's dialogue, `<event>.dialogue.<line>`, which
    /// moves it on to the next.
    Dialogue(u8),
    /// A bare `PROCEED`, which no page names.
    Proceed,
    /// The option a run modifier gives Neow, whose key is the modifier's id.
    Modifier(V::Modifier),
}

impl<V: sts2_core::ContentVersion> sts2_core::EventOptionKinds<V> for EventOptionId<V> {
    fn from_relic(relic: V::Relic) -> Self {
        Self::Relic(relic)
    }

    fn relic(self) -> Option<V::Relic> {
        match self {
            Self::Relic(relic) => Some(relic),
            Self::Said(_) | Self::Modifier(_) | Self::Dialogue(_) | Self::Proceed => None,
        }
    }

    fn from_modifier(modifier: V::Modifier) -> Self {
        Self::Modifier(modifier)
    }
}

/// What a fight's card writes on its deck version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckWrite {
    /// `BuffFromPlay(n)` of `GeneticAlgorithm` or `TheScythe`.
    BuffFromPlay(i32),
    /// `DeckVersion.Enchantment.Amount += n`, Goopy's.
    RaiseEnchantment(u8),
}

/// `CardModel.SetToFreeThisTurn`: energy and stars at nothing this turn or
/// until played.
pub(crate) fn free_this_turn<V: crate::Build, F: Fight<V>>(fight: &mut F, at: CardAt<V>) {
    for pool in [CostPool::Energy, CostPool::Star] {
        fight.modify_card(
            at,
            Change::SetCost(pool, num_traits::zero()),
            Lasting::ThisTurnOrUntilPlayed,
        );
    }
}

/// `GetDistinctForCombat` of one card of the character's pool that `keeps`
/// holds, off `CombatCardGeneration`, then `SetToFreeThisTurn` and into the
/// hand. Nothing when the pool holds none.
///
/// `generated` is `CardPileCmd.AddGeneratedCardToCombat`, which the fight
/// records and raises `AfterCardGeneratedForCombat` for. Otherwise it is a
/// plain `CardPileCmd.Add`.
pub(crate) fn free_card_into_hand<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    keeps: impl Fn(V::Card) -> bool,
    generated: bool,
) {
    let mut pool: Vec<V::Card> = V::combat_cards(fight.character(), fight.multiplayer())
        .iter()
        .copied()
        .filter(|&card| keeps(card))
        .collect();
    if pool.is_empty() {
        return;
    }
    let stream = <V::RunRngType as RunStreamKinds>::COMBAT_CARD_GENERATION;
    let [made] = fight
        .generate_cards::<1>(&mut pool, stream)
        .unwrap_or_else(|_| unreachable!("a pool with a card in it makes one"));
    let at = match generated {
        true => fight
            .add_generated_cards(made, Pile::Hand, 1)
            .first()
            .copied(),
        false => fight.add_card(made, Pile::Hand),
    };
    if let Some(at) = at {
        free_this_turn(fight, at);
    }
}

/// The character's combat pool, narrowed to the cards of `kind`. A card
/// whose type the fight works out is in none.
pub(crate) fn character_cards<V: crate::Build, F: Fight<V>>(
    fight: &F,
    kind: CardKind,
) -> Vec<V::Card> {
    V::combat_cards(fight.character(), fight.multiplayer())
        .iter()
        .copied()
        .filter(|&card| V::card_kind(card) == Some(kind))
        .collect()
}

/// `ALL_CHARACTERS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// The characters `ModelDb.AllCharacters` lists, in its order, which
        /// is also `UnlockState.CharacterCardPools`' with every character
        /// unlocked.
        ///
        /// Not the table's, which is alphabetical. A draw indexes into this
        /// one.
        const ALL_CHARACTERS: &'static [<$build as sts2_core::ContentVersion>::Character] = &[
            <$build as sts2_core::ContentVersion>::Character::IRONCLAD,
            <$build as sts2_core::ContentVersion>::Character::SILENT,
            <$build as sts2_core::ContentVersion>::Character::REGENT,
            <$build as sts2_core::ContentVersion>::Character::NECROBINDER,
            <$build as sts2_core::ContentVersion>::Character::DEFECT,
        ];
    };
}

pub(crate) use lists;
