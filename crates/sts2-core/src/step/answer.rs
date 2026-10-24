//! Answering a game that stopped mid-action to ask something.

/// How to answer a parked game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The options a select-cards question takes, in the order they were
    /// picked.
    ///
    /// Each one picked in turn and then [`Answer::Done`], after whatever the
    /// question already holds. A script's shorthand: the moves a run offers
    /// are the three below.
    Cards(Picks),
    /// One more card, after the ones a select-cards question already holds.
    ///
    /// The game's own screens take a selection a click at a time. The cards
    /// are handed back in the order of the clicks.
    Pick(u16),
    /// Take back the last card picked.
    ///
    /// The hand's list drops its last entry. A grid's set hands the freed
    /// slot to the next pick. Either screen ends where the pick began.
    Undo,
    /// Hand the cards picked so far to the game.
    Done,
    /// Decline a reward set. Refused if it is not skippable.
    Skip,
    /// Take one reward from a reward set.
    ///
    /// One, because the set is asked again with what is left until every
    /// reward is taken or the rest is skipped. Taking several is several
    /// answers.
    Take(TakeReward),
    /// Uncover a cell, in the Crystal Sphere's minigame.
    Divine { x: u8, y: u8, tool: Tool },
    /// Yes or no, to a popup that asks for confirmation.
    Confirm(bool),
    /// Backs out of a card selection that can be cancelled, as its screen's
    /// back button does. The game gets no cards, and whatever opened the
    /// selection is left undone. Smith, Cook, a shop's card removal and the
    /// card-removal reward open one.
    ///
    /// Last, because a digest hashes an answer by its place in this list.
    Cancel,
}

/// Which of a question's options a card selection takes, in the order it took
/// them.
///
/// Ordered, because the game's hand hands back its picks in click order, and a
/// card such as Purity exhausts them in that order.
///
/// Each option appears once. The same card twice is one selection to the game
/// and two to a client, so [`Picks::push`] panics on a repeat.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Picks {
    /// How many of `at` are picks. The rest stay zero, so the derived
    /// equality and hash see only the picks.
    len: u8,
    /// Each option's place among the question's. A deck Hoarder grows can
    /// hold more cards than a byte counts.
    at: [u16; PICKS],
}

/// How many cards one selection can take, held inline so that a
/// [`crate::Decision`] is `Copy`.
///
/// The hand holds ten. The widest selection outside it, Sea Glass's grid,
/// takes fifteen. [`Picks::push`] panics past this.
const PICKS: usize = 15;

impl Picks {
    /// A selection that takes nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// Take one more option, after the ones already taken.
    ///
    /// For a selection built from the question's own options, where one this
    /// cannot hold is a selection the simulator has not modelled.
    ///
    /// # Panics
    ///
    /// Where [`Picks::try_push`] refuses.
    pub fn push(&mut self, option: usize) {
        if let Err(why) = self.try_push(option) {
            unimplemented!("a selection that {why}");
        }
    }

    /// Take one more option, or say why it cannot be taken.
    ///
    /// For a selection read from outside: a recording, or what someone typed.
    /// A refusal leaves the selection as it was.
    pub fn try_push(&mut self, option: usize) -> Result<(), Unpickable> {
        if self.iter().any(|taken| taken == option) {
            return Err(Unpickable::Repeated(option));
        }
        let Ok(at) = u16::try_from(option) else {
            return Err(Unpickable::Distant(option));
        };
        let Some(slot) = self.at.get_mut(usize::from(self.len)) else {
            return Err(Unpickable::Full);
        };
        *slot = at;
        self.len += 1;
        Ok(())
    }

    /// The options taken, in the order they were taken.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.at[..usize::from(self.len)]
            .iter()
            .map(|&at| usize::from(at))
    }

    /// The option taken, for a selection that took exactly one.
    pub fn only(&self) -> Option<usize> {
        match self.len {
            1 => Some(usize::from(self.at[0])),
            _ => None,
        }
    }

    /// Gives back the option taken last.
    pub fn pop(&mut self) -> Option<usize> {
        let last = usize::from(self.len.checked_sub(1)?);
        let at = std::mem::take(&mut self.at[last]);
        self.len -= 1;
        Some(usize::from(at))
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl FromIterator<usize> for Picks {
    /// # Panics
    ///
    /// As [`Picks::push`] does.
    fn from_iter<I: IntoIterator<Item = usize>>(options: I) -> Self {
        let mut picks = Self::none();
        for option in options {
            picks.push(option);
        }
        picks
    }
}

impl TryFrom<&[usize]> for Picks {
    type Error = Unpickable;

    fn try_from(options: &[usize]) -> Result<Self, Unpickable> {
        let mut picks = Self::none();
        for &option in options {
            picks.try_push(option)?;
        }
        Ok(picks)
    }
}

impl<const N: usize> From<[usize; N]> for Picks {
    fn from(options: [usize; N]) -> Self {
        options.into_iter().collect()
    }
}

/// Why an option cannot join a [`Picks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unpickable {
    /// The selection already takes it.
    #[error("takes option {0} twice")]
    Repeated(usize),
    /// The selection already takes as many as one can.
    #[error("takes more options than a selection holds")]
    Full,
    /// No selection names an option that far along.
    #[error("names option {0}, past any a selection names")]
    Distant(usize),
}

impl std::fmt::Debug for Picks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// What a [`Answer::Divine`] uncovers with.
///
/// The Crystal Sphere hides items under an 11x11 grid and gives three or six
/// divinations. Each costs the same whichever tool it uses, so `Big` is not
/// free. It uncovers more per divination, and one thing it uncovers is a curse.
///
/// The game's enum has a third member, `None`, which is left out. `CellClicked`
/// tests only for `Big`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// The cell named, and nothing else.
    Small,
    /// The cell named and its neighbours, in all eight directions.
    Big,
}

/// The reward an [`Answer::Take`] takes.
///
/// Two indices live here and they address different things.
///
/// `index` is a position in the reward set the game offered. Those never
/// renumber as rewards are taken.
///
/// `card` is a position inside one card reward, in the pack of one to four cards
/// that reward is offering. It means nothing for any other kind.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TakeReward {
    /// Gold, a relic, a potion. The reward is whatever it is.
    NonCard { index: u8 },
    /// A card reward, which is not taken until you name a card in its pack.
    Card { index: u8, card: u8 },
    /// One of a card reward's `CardRewardAlternative`s in place of a card.
    Alternative {
        index: u8,
        alternative: RewardAlternative,
    },
}

/// A `CardRewardAlternative` other than `Skip`, by its `OptionId`. Skipping a
/// card reward is leaving it untaken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RewardAlternative {
    /// `REROLL`: the reward draws a fresh pack, and stays on offer.
    Reroll,
    /// `SACRIFICE`: the reward is taken with no card.
    Sacrifice,
}

impl RewardAlternative {
    /// Its `OptionId`, which is how the game names it.
    pub fn option_id(self) -> &'static str {
        match self {
            Self::Reroll => "REROLL",
            Self::Sacrifice => "SACRIFICE",
        }
    }

    /// The alternative an `OptionId` names, if this crate knows it.
    pub fn from_option_id(id: &str) -> Option<Self> {
        match id {
            "REROLL" => Some(Self::Reroll),
            "SACRIFICE" => Some(Self::Sacrifice),
            _ => None,
        }
    }
}

impl TakeReward {
    /// Gold, a relic or a potion, by its position in the offered set.
    ///
    /// # Panics
    ///
    /// A position past what a reward set can hold.
    pub fn non_card(index: usize) -> Self {
        Self::NonCard {
            index: position(index),
        }
    }

    /// One card of a card reward, by the reward's position in the set and the
    /// card's in its pack.
    ///
    /// # Panics
    ///
    /// A position past what a reward set or a pack can hold.
    pub fn card(index: usize, card: usize) -> Self {
        Self::Card {
            index: position(index),
            card: position(card),
        }
    }

    /// A card reward's alternative, by the reward's position in the set.
    ///
    /// # Panics
    ///
    /// A position past what a reward set can hold.
    pub fn alternative(index: usize, alternative: RewardAlternative) -> Self {
        Self::Alternative {
            index: position(index),
            alternative,
        }
    }

    /// Which reward in the offered set this takes.
    pub fn index(&self) -> usize {
        match self {
            Self::NonCard { index }
            | Self::Card { index, .. }
            | Self::Alternative { index, .. } => usize::from(*index),
        }
    }
}

/// A position in a reward set or a pack, as a [`TakeReward`] holds one.
fn position(at: usize) -> u8 {
    u8::try_from(at).unwrap_or_else(|_| unimplemented!("a reward at position {at}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_keep_the_order_they_were_taken_in() {
        let picks = Picks::from([3, 0, 2]);
        assert_eq!(picks.iter().collect::<Vec<_>>(), [3, 0, 2]);
        assert_ne!(picks, Picks::from([0, 2, 3]), "order is part of the answer");
    }

    #[test]
    fn picks_built_apart_are_equal_when_they_take_the_same() {
        let mut built = Picks::none();
        built.push(4);
        built.push(1);
        assert_eq!(built, Picks::from([4, 1]));
        assert_eq!(built.only(), None);
        assert_eq!(Picks::from([7]).only(), Some(7));
        assert!(Picks::none().is_empty());
    }

    /// What cannot be read comes back as a reason, and the selection is left
    /// as it was.
    #[test]
    fn a_selection_read_from_outside_says_why_it_cannot_be_held() {
        let mut picks = Picks::from([1]);
        assert_eq!(picks.try_push(1), Err(Unpickable::Repeated(1)));
        assert_eq!(
            picks.try_push(usize::MAX),
            Err(Unpickable::Distant(usize::MAX))
        );
        assert_eq!(picks, Picks::from([1]), "a refusal changes nothing");

        let full: Vec<usize> = (0..=PICKS).collect();
        assert_eq!(Picks::try_from(full.as_slice()), Err(Unpickable::Full));
    }

    #[test]
    #[should_panic(expected = "twice")]
    fn a_repeated_pick_panics() {
        let _ = Picks::from([1, 1]);
    }

    #[test]
    #[should_panic(expected = "more options than a selection holds")]
    fn a_selection_past_what_one_holds_panics() {
        let _: Picks = (0..=PICKS).collect();
    }

    #[test]
    #[should_panic(expected = "past any a selection names")]
    fn an_option_past_what_one_holds_panics() {
        let _ = Picks::from([usize::from(u16::MAX) + 1]);
    }
}
