//! `PotionModel.OnUse`, ported.
//!
//! One module per pool, filed the way `generated/potions/` is, so a potion's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first potion is ported.
//!
//! [`on_use`] panics for a potion nobody has ported, so this tree is also the
//! list of what a run can drink.

mod shared;

use sts2_core::{Fight, PotionTable};

use super::super::V0_107_1 as V;
use super::super::generated::potions::{Potion, PotionPool};

/// What a potion does when it is drunk.
///
/// # Panics
///
/// A potion from a pool nobody has ported, and a potion its own pool has not.
pub(crate) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) {
    match potion.pool() {
        PotionPool::Shared => shared::on_use(potion, fight),
        _ => unimplemented!("what {potion:?} does when it is drunk"),
    }
}
