//! Answering a game that stopped mid-action to ask something.

/// How to answer a parked game.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Answer {
    /// Option indices, for a select-cards question. Repeating one is refused:
    /// the same card twice is one selection to the game and two to the client.
    Cards(Vec<usize>),
    /// Decline a reward set. Refused if it is not skippable.
    Skip,
    /// Take rewards from a reward set.
    Take(Vec<TakeReward>),
    /// Uncover a cell, in the Crystal Sphere's minigame.
    Divine { x: u8, y: u8, tool: Tool },
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

/// One entry in an [`Answer::Take`].
///
/// Two indices live here and they address different things.
///
/// `index` is a position in the reward set the game offered. Those never
/// renumber as rewards are taken.
///
/// `card` is a position inside one card reward, in the pack of one to four cards
/// that reward is offering. It means nothing for any other kind.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TakeReward {
    /// Gold, a relic, a potion. The reward is whatever it is.
    NonCard { index: usize },
    /// A card reward, which is not taken until you name a card in its pack.
    Card { index: usize, card: usize },
}

impl TakeReward {
    /// Which reward in the offered set this takes.
    pub fn index(&self) -> usize {
        match self {
            Self::NonCard { index } | Self::Card { index, .. } => *index,
        }
    }
}
