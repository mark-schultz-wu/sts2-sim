//! An enemy, and the move it has declared.

use alloc::collections::BTreeMap;
use alloc::string::String;

use serde::{Deserialize, Serialize};

/// The game's `MonsterMoves.Intents.IntentType`.
///
/// `Hidden` is the game concealing a move; `Unknown` is a move it could not
/// classify. Both are stated intents, distinct from having none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    Attack,
    Buff,
    Debuff,
    DebuffStrong,
    Defend,
    Escape,
    Heal,
    Hidden,
    Summon,
    Sleep,
    Stun,
    StatusCard,
    CardDebuff,
    DeathBlow,
    Unknown,
}

/// What an enemy has declared it will do next.
///
/// One move, not a list: STS1's `move_id` and STS2's `MonsterModel.NextMove`
/// are both singular, and a multi-hit attack is one intent with
/// [`repeats`](Intent::repeats) above 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub kind: IntentType,
    /// Hit count for a multi-hit attack. The game's `AttackIntent.Repeats`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeats: Option<u8>,
    /// Damage per hit. Absent means the backend could not say.
    ///
    /// Comes from the move rather than the intent — the game's
    /// `AbstractIntent` carries no damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Monster {
    pub id: String,
    pub hp: i32,
    pub max_hp: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub block: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<Intent>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub powers: BTreeMap<String, i32>,
}

fn is_zero(n: &i32) -> bool {
    *n == 0
}
