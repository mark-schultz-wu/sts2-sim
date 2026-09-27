//! The Defect's cards the generator leaves to a hand port.

use sts2_core::{Fight, Played};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::DefectCard;
use super::super::super::generated::orbs::Orb;
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: DefectCard, fight: &mut F) -> Played<V> {
    match card {
        DefectCard::Dualcast => dualcast(fight),
        DefectCard::Tempest => tempest(fight),
        card => card_plays::defect::on_play(card, fight),
    }
}

/// `Tempest`: a Lightning channelled for each energy of its X, and one more
/// once upgraded.
fn tempest<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let orbs = fight.x_value() + super::Number::from(fight.upgraded());
    for _ in 0..orbs {
        fight.channel(Orb::LightningOrb);
    }
    Played::Done
}

/// `Dualcast`. Evoke the first orb twice: once where it stands, then once
/// as it leaves. Nothing with no orb.
fn dualcast<F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.orbs_held() > 0 {
        fight.evoke_next(false);
        fight.evoke_next(true);
    }
    Played::Done
}
