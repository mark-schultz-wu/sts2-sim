//! `SeaGlass`: fifteen cards of another character's pool, any of which join
//! the deck as it is picked up.

use crate::ids::CharacterIds as _;
use sts2_core::{EventHook, HookEffect};

/// `CardsVar(15)`, a third of it at each rarity.
const CARDS: usize = 15;

/// `CharacterId`, which it saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State<V: crate::Build> {
    character: Option<V::Character>,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        Self {
            character: Default::default(),
        }
    }
}

/// Orobas names the character before the relic is obtained. `AfterObtained`
/// falls back on the Ironclad for one obtained unnamed, as the game logs and
/// does. It makes five cards of each rarity from that character's pool, and a
/// grid takes any of them into the deck.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CharacterNamed(character) => state.character = Some(character),
        EventHook::Obtained { .. } => {
            let pool = *state.character.get_or_insert(V::Character::IRONCLAD);
            asked.push(HookEffect::ChooseCardsOfPool {
                pool,
                per_rarity: CARDS / 3,
                min: 0,
                max: CARDS,
            });
        }
        hook => unimplemented!("what SeaGlass does for {hook:?}"),
    }
}
