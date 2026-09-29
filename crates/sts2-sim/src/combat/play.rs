//! Playing one card.
//!
//! A port of `PlayCardAction.ExecuteAction` and the wrapper it calls. The
//! order is the game's. The play is checked, the pools are spent, and the card
//! leaves the hand. Its own `OnPlay` runs. Only then does the card land in the
//! pile it ends in.
//!
//! What each card does is not here. That is per build and lives in
//! `sts2-content`, reached through [`Fight`]. This file is the fight that
//! trait acts on.

use crate::stream::LendsCombat;
use smallvec::SmallVec;
use sts2_core::{CardTable, CombatId, Cost, HandIndex, Reason, Rules};

use super::board::Board;
use super::live::{Asked, Asker, CardPlay, Live, Plays, Resuming, Skips, Then};
use super::powers::Powers as _;
use super::{Error, Fighting, hook, legal};

/// A play the fight allows, resolved but not yet made.
///
/// Split from making it the way travel is, so that a card the simulator
/// cannot resolve leaves the fight where it stood.
pub(crate) struct Play<V: Rules> {
    /// Where in the hand it is, which is the card's identity here. A hand
    /// holds two of one card, and only the index tells them apart.
    index: usize,
    /// The creature it was pointed at, or nothing for a card that names none.
    /// Held as the fight names one, because this is where it was found.
    target: Option<CombatId>,
    /// What each pool gives up for it, worked out where the play was approved
    /// rather than again where it is made.
    energy: V::Count,
    stars: V::Count,
}

/// Check one play against the fight, without changing anything.
///
/// The order the game refuses in, which is `Protocol.PlayCard`: the hand
/// index, then the creature id, then the card's own two rules.
pub(crate) fn resolve<V: Rules>(
    live: &Live<V>,
    relics: &crate::run::Belt<V>,
    index: HandIndex,
    target: Option<CombatId>,
) -> Result<Play<V>, Reason<V>> {
    let at = usize::from(index.get());
    let held = live
        .cards
        .get(sts2_core::Pile::Hand, at)
        .ok_or(Reason::OutOfRange {
            index: i64::from(index.get()),
            count: live.cards.len(sts2_core::Pile::Hand),
        })?;
    let card = held.face.ident;
    let (cost, star_cost) = legal::costs::<V>(&held, super::listen::Walk { live, relics });

    // The creature the play named has to still be in this fight. An id is
    // chosen against an observation, and a fight moves on from one.
    if let Some(id) = target
        && !live.creatures.iter().any(|creature| creature.id == id)
    {
        return Err(Reason::NoSuchCreature { id });
    }

    // The same rules the reported hand carries, asked of the one card being
    // played. Nothing here builds the rest of the hand, or its previews.
    let takes = legal::target_type::<V>(card, live);
    let fight = super::listen::Walk { live, relics };
    let (refused, preventer) = legal::reasons::<V>(
        card,
        fight,
        (cost, star_cost),
        live.energy,
        live.stars,
        legal::asks_should_play::<V>(fight),
    );
    if let Some(why) = refused {
        return Err(Reason::Unplayable {
            card,
            why,
            preventer,
        });
    }
    let targets = legal::targets::<V>(takes, &live.creatures, false);

    // `CanPlayTargeting`: a card that names a creature is pointed at one its
    // own target type accepts, and a card that names none at nothing.
    let aimed = match target {
        Some(id) => targets.contains(&id),
        None => !legal::names_creature::<V>(takes),
    };
    if !aimed {
        return Err(Reason::WrongTarget {
            card,
            target,
            takes,
        });
    }

    if star_cost == Cost::X {
        // `GetStarCostWithModifiers` is a number, not a pool to drain. No card
        // on this build prices its stars at X.
        unimplemented!("a card priced at X stars");
    }

    // `SpendResources`, worked out where the play was held to the pools rather
    // than again where it is made. `legal::playable` marks a card whose price
    // its pool cannot meet unplayable, and the reason check above has passed.
    const PAYS: &str = "a play the legality pass approved is one the pool pays";
    Ok(Play {
        index: at,
        target,
        energy: legal::spend(cost, live.energy).expect(PAYS),
        stars: legal::spend(star_cost, live.stars).expect(PAYS),
    })
}

/// Put a power on a creature, with no card or move behind it.
///
/// Scenario construction. The command a card applies a power with, reached
/// without a card, so the state it leaves is the state a card leaves.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn apply_power<V: Rules>(
    fighting: Fighting<'_, V>,
    at: CombatId,
    power: V::Power,
    amount: sts2_core::PowerAmount<V::Number>,
) -> Option<super::Ended> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    // Nothing here draws. A board is built with the set, and building one
    // leaves every counter where it was.
    let ascension = live.ascension;
    let mut board = Board::for_card(live, streams.combat(), relics, ascension, None, None);
    // `ApplyPowerConsoleCmd` hands `PowerCmd.Apply` no applier.
    board.land(at, power, amount, None);
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);

    merge::<V>(&mut live.skip_tick, skipped);
    if ended == Some(super::Ended::Won) {
        super::listen::combat_ended::<V>(live, relics);
    }
    ended
}

/// `Driver.GrantCard` in a fight: `CombatState.AddCard`, then
/// `CardPileCmd.Add` into hand, which raises `AfterCardEnteredCombat`. The
/// card is not generated, so nothing records it or hears it made.
///
/// Scenario construction, as [`apply_power`] is.
pub(crate) fn grant_card<V: Rules>(fighting: Fighting<'_, V>, card: V::Card) {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    let ascension = live.ascension;
    let mut board = Board::for_card(live, streams.combat(), relics, ascension, None, None);
    sts2_core::Fight::add_card(
        &mut board,
        sts2_core::CardIdent::new(card),
        sts2_core::Pile::Hand,
    );
    let skipped = std::mem::take(&mut board.skipped);
    merge::<V>(&mut live.skip_tick, skipped);
}

/// Enchant the copy at `index` in hand, which is `EnchantConsoleCmd`.
///
/// Scenario construction. `CardCmd.Enchant` on the fight's copy, so the deck
/// keeps its card as it was.
///
/// # Panics
///
/// A card the enchantment does not take, which the game throws on.
pub(crate) fn enchant<V: Rules>(
    fighting: Fighting<'_, V>,
    enchantment: V::Enchantment,
    amount: u8,
    index: sts2_core::HandIndex,
) -> Result<(), sts2_core::Reason<V>> {
    let live = fighting.live;
    let count = live.cards.len(sts2_core::Pile::Hand);
    let at = usize::from(index.get());
    let Some(card) = live.cards.get(sts2_core::Pile::Hand, at) else {
        return Err(sts2_core::Reason::OutOfRange {
            index: i64::from(index.get()),
            count,
        });
    };
    let mut ident = card.face.ident;
    if !V::can_enchant(enchantment, &ident) {
        unimplemented!("{enchantment:?} put on {ident:?}, which it does not take");
    }
    V::enchant(enchantment, amount, &mut ident);
    live.cards.write(sts2_core::Pile::Hand, at, |face| {
        face.ident = ident;
        true
    });
    Ok(())
}

/// Add the skipped ticks an effect just made to the ones the fight holds.
pub(super) fn merge<V: Rules>(held: &mut Skips<V>, added: Vec<(CombatId, V::Power)>) {
    for one in added {
        if !held.contains(&one) {
            held.push(one);
        }
    }
}

/// Make a resolved play, on the fight where it stands.
///
/// Whatever the play ended, if it ended the fight.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn play<V: Rules>(
    fighting: Fighting<'_, V>,
    play: Play<V>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;

    // `SpendResources`, before the card leaves the hand.
    live.energy = live.energy - play.energy;
    let spent: V::Number = num_traits::cast(play.stars).expect("a star cost fits a game number");
    live.modify_stars(-spent);

    // `AddDuringManualCardPlay`: the card is in the play pile, and so out of
    // the hand, while its own effect runs.
    let card = live.cards.play(sts2_core::Pile::Hand, play.index);
    let ascension = live.ascension;
    let (pile, position) = result_pile::<V>(live, relics, &card, play.energy);
    let count = play_count::<V, _>(live, streams, relics, &card);
    before_played::<V, _>(live, streams, relics, &card, play.energy, play.target);
    live.history
        .start(card.face.ident.card, card.id, play.energy, true);
    let mut board = Board::for_card(
        live,
        streams.combat(),
        relics,
        ascension,
        play.target,
        Some(card.id),
    )
    .blocking();
    let played = V::on_play(card.face.ident.card, &mut board);
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);
    merge::<V>(&mut live.skip_tick, skipped);

    carry_on::<V, _>(
        Fighting {
            live,
            streams,
            relics,
        },
        ascension,
        Resuming {
            // `GeneratePlayCount`, worked out once and before the first play.
            plays: Plays {
                done: num_traits::zero(),
                count,
            },
            asker: Asker::Card(CardPlay {
                id: card.id,
                energy: play.energy,
                pile,
                position,
            }),
            target: play.target,
            ended,
        },
        played,
        true,
    )
}

/// `CardCmd.AutoPlay` of the copy `id`, which stands in hand, with no target
/// named. How the fight ended, if the play ended it.
///
/// The play is free, and it runs the loop a play made by hand runs. A card
/// that targets any enemy draws one off `CombatTargets`. It runs inside
/// whatever board drew the card, so it draws through that board's streams.
///
/// # Panics
///
/// A card that stops on a question, or that is unplayable or costs X. A card
/// that targets an ally, or has no enemy it can hit.
pub(super) fn auto_play<V: Rules>(
    live: &mut Live<V>,
    rngs: crate::stream::CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    id: sts2_core::CardId<V>,
    from: sts2_core::Pile,
    named: Option<CombatId>,
    pausable: bool,
) -> Option<super::Ended> {
    // A question an earlier auto-play asked stands the effect still, and
    // this one waits behind it where it lies.
    if live.asking.is_some() {
        if !pausable {
            unimplemented!("auto-playing {id:?} while a question waits outside an effect");
        }
        live.defer(super::live::Deferred::AutoPlay { id, named });
        return None;
    }
    let Some(at) = live.cards.find(from, id) else {
        unimplemented!("auto-playing {id:?}, which left the {from:?} pile");
    };
    // `CardPileCmd.Add` to the play pile, where it stands while it runs.
    let card = live.cards.play(from, at);
    auto_play_in_play::<V>(live, rngs, relics, card, named, pausable)
}

/// `CardCmd.AutoPlay` for a card already in the play pile. `named` is the
/// target the caller names, which an enemy-targeting card plays at in place of
/// one drawn off `CombatTargets`.
pub(super) fn auto_play_in_play<V: Rules>(
    live: &mut Live<V>,
    mut rngs: crate::stream::CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    card: super::live::CardInstance<V>,
    named: Option<CombatId>,
    pausable: bool,
) -> Option<super::Ended> {
    use sts2_core::{CardTable as _, TargetKinds as _};

    let ident = card.face.ident;
    if ident.card.unplayable() {
        return without_playing::<V>(live, rngs, relics, &card);
    }
    if ident.card.cost() == sts2_core::Cost::X {
        unimplemented!("auto-playing {ident:?}, which costs X");
    }
    // `Hook.ShouldPlay`, with the auto-play's type. A refused card goes to
    // its pile unplayed.
    if super::listen::first_deciding::<V>(
        super::listen::Walk { live, relics },
        sts2_core::DecisionHook::ShouldPlay {
            card: ident.card,
            affliction: ident.affliction,
            auto: true,
        },
        false,
    )
    .is_some()
    {
        return without_playing::<V>(live, rngs, relics, &card);
    }
    let ascension = live.ascension;
    let target = match super::legal::target_type::<V>(ident, live) {
        kind if kind == V::TargetType::ANY_ENEMY && named.is_some() => named,
        kind if kind == V::TargetType::ANY_ENEMY => {
            let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
            let Some(target) = sts2_core::Fight::random_opponent(&mut board) else {
                return without_playing::<V>(live, rngs, relics, &card);
            };
            Some(target)
        }
        kind if kind == V::TargetType::ANY_ALLY => {
            unimplemented!("auto-playing {ident:?}, which targets an ally")
        }
        _ => None,
    };
    super::turn::unheard::<V>(live, relics, sts2_core::ModelHook::BeforeCardAutoPlayed);

    let energy = num_traits::zero();
    let (pile, position) = result_pile::<V>(live, relics, &card, energy);
    let count = play_count::<V, _>(live, &mut rngs, relics, &card);
    before_played::<V, _>(live, &mut rngs, relics, &card, energy, target);
    live.history
        .start(card.face.ident.card, card.id, energy, true);
    let mut board = Board::for_card(
        live,
        rngs.reborrow(),
        relics,
        ascension,
        target,
        Some(card.id),
    );
    if pausable {
        board = board.blocking();
    }
    let played = V::on_play(card.face.ident.card, &mut board);
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);
    merge::<V>(&mut live.skip_tick, skipped);

    let ended = carry_on::<V, _>(
        Fighting {
            live: &mut *live,
            streams: &mut rngs,
            relics: &mut *relics,
        },
        ascension,
        Resuming {
            plays: Plays {
                done: num_traits::zero(),
                count,
            },
            asker: Asker::Card(CardPlay {
                id: card.id,
                energy,
                pile,
                position,
            }),
            target,
            ended,
        },
        played,
        false,
    )
    .unwrap_or_else(|error| unimplemented!("an auto-play that failed: {error:?}"));
    // A card that asks stands the effect around it still, which only a
    // card's or a potion's own effect can do.
    if live.asking.is_some() && !pausable {
        unimplemented!("an auto-played {ident:?} that stops on a question outside an effect");
    }
    ended
}

/// `CardModel.MoveToResultPileWithoutPlaying` for a card in the play pile:
/// exhausted once `ExhaustOnNextPlay` or its Exhaust says so, and discarded
/// otherwise.
fn without_playing<V: Rules>(
    live: &mut Live<V>,
    rngs: crate::stream::CombatRngs<'_>,
    relics: &mut crate::run::Belt<V>,
    card: &super::live::CardInstance<V>,
) -> Option<super::Ended> {
    let marked = live.exhaust_next.iter().position(|&id| id == card.id);
    if let Some(at) = marked {
        live.exhaust_next.remove(at);
    }
    match marked.is_some() || card.face.exhausts() {
        true => {
            live.cards.leave_play(
                card.id,
                Some(sts2_core::Pile::Exhaust),
                sts2_core::PilePosition::Bottom,
            );
            super::turn::exhausted::<V>(live, rngs, relics, card, false)
        }
        false => {
            live.cards.leave_play(
                card.id,
                Some(sts2_core::Pile::Discard),
                sts2_core::PilePosition::Bottom,
            );
            None
        }
    }
}

/// `Hook.BeforeCardPlayed`, before one play of `card`, which stands in the
/// play pile. A power that counts plays down decrements through the walk
/// itself.
///
/// A card that shifts its own cost writes it wherever it stands, and what
/// anything else asks runs through the board. `energy` is what the play
/// spent, which is the X an X-cost card captured.
fn before_played<V: Rules, S: LendsCombat>(
    live: &mut Live<V>,
    streams: &mut S,
    relics: &mut crate::run::Belt<V>,
    card: &super::live::CardInstance<V>,
    energy: V::Count,
    target: Option<sts2_core::CombatId>,
) {
    if !super::listen::heard::<V>(live, relics, sts2_core::ModelHook::BeforeCardPlayed) {
        return;
    }
    let resolved = match card.face.ident.card.cost() {
        sts2_core::Cost::X => num_traits::cast(energy).expect("an energy fits a game number"),
        _ => {
            let fight = super::listen::Walk { live, relics };
            match super::legal::costs::<V>(card, fight).0 {
                sts2_core::Cost::Fixed(cost) => {
                    num_traits::cast(cost.get()).expect("a cost fits a game number")
                }
                cost => unimplemented!("a card played at a cost of {cost:?}"),
            }
        }
    };
    let hook = sts2_core::EventHook::CardPlaying {
        card: card.face.ident.card,
        id: card.id,
        affliction: card.face.ident.affliction,
        target,
        ethereal: card.face.ethereal(),
        resolved,
        spent: num_traits::cast(energy).expect("an energy fits a game number"),
    };
    let mut rest: SmallVec<[sts2_core::HookEffect<V>; 4]> = SmallVec::new();
    for (listener, effect) in super::listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            (
                super::listen::Listener::Card { id },
                sts2_core::HookEffect::ModifySelf { change, lasting },
            ) => live.modify_self(id, change, lasting),
            (_, effect) => rest.push(effect),
        }
    }
    if !rest.is_empty() {
        let ascension = live.ascension;
        let mut board = Board::for_card(live, streams.combat(), relics, ascension, None, None);
        hook::apply::<V, _>(&mut board, rest);
        let skipped = std::mem::take(&mut board.skipped);
        merge::<V>(&mut live.skip_tick, skipped);
    }
}

/// `GetResultPileTypeForCardPlay`, run through
/// `ModifyCardPlayResultPileTypeAndPosition`, then
/// `AfterModifyingCardPlayResultPileOrPosition` for each listener that changed
/// it.
///
/// A power card leaves the fight rather than going to a pile, and a card that
/// exhausts goes to the exhaust pile. Anything else is discarded. A card whose
/// class overrides the virtual then has its say.
///
/// # Panics
///
/// A card whose type the fight works out, or a listener other than a power
/// changing the pile.
fn result_pile<V: Rules>(
    live: &mut Live<V>,
    relics: &mut crate::run::Belt<V>,
    card: &super::live::CardInstance<V>,
    energy: V::Count,
) -> (Option<sts2_core::Pile>, sts2_core::PilePosition) {
    let played = card.face.ident.card;
    // `ExhaustOnNextPlay`, which the pile it picks clears.
    let marked = live.exhaust_next.iter().position(|&id| id == card.id);
    if let Some(at) = marked {
        live.exhaust_next.remove(at);
    }
    let pile = match played
        .power()
        .unwrap_or_else(|| unimplemented!("a card that works its own type out from the fight"))
    {
        true => None,
        false if marked.is_some() || card.face.exhausts() => Some(sts2_core::Pile::Exhaust),
        false => Some(sts2_core::Pile::Discard),
    };
    let pile = match V::card_result_pile(played, pile) {
        sts2_core::HookResponse::Decision(pile) => pile,
        sts2_core::HookResponse::DoesNotImplement => pile,
    };
    let player = live.player().id;
    let mut result = (pile, sts2_core::PilePosition::Bottom);
    let mut modifiers: SmallVec<[(CombatId, V::Power); 2]> = SmallVec::new();
    super::listen::each::<V>(
        super::listen::Walk { live, relics },
        sts2_core::Overrides::of(sts2_core::ModelHook::ModifyCardPlayResultPileTypeAndPosition),
        |listener, _| {
            let hook = sts2_core::ResultPile {
                player,
                card: played,
                energy,
                pile: result.0,
                position: result.1,
            };
            let sts2_core::HookResponse::Decision(next) = V::result_pile(listener, hook) else {
                return;
            };
            if next != result {
                match listener {
                    sts2_core::Listener::Power { holder, power, .. } => {
                        modifiers.push((holder, power))
                    }
                    _ => unimplemented!("a listener other than a power moving a played card"),
                }
            }
            result = next;
        },
    );
    // `Hook.AfterModifyingCardPlayResultPileOrPosition` over the modifiers,
    // which does nothing for one that does not override it.
    for (holder, power) in modifiers {
        if !sts2_core::PowerTable::overrides(power)
            .contains(sts2_core::ModelHook::AfterModifyingCardPlayResultPileOrPosition)
        {
            continue;
        }
        let Some(at) = live
            .powers
            .of(holder)
            .position(|held| held.power() == power)
        else {
            continue;
        };
        let mut asked = sts2_core::HookEffects::new();
        live.ask_power(
            Some(relics),
            holder,
            at,
            sts2_core::EventHook::ResultPileModified { card: played },
            &mut asked,
        );
        if let Some(effect) = asked.first() {
            unimplemented!("a power asking for {effect:?} after moving a played card")
        }
    }
    result
}

/// `GeneratePlayCount`: the copy's own plays, run through
/// `ModifyCardPlayCount`, then `AfterModifyingCardPlayCount` for each listener
/// that changed the count.
///
/// # Panics
///
/// A listener other than a power changing the count, or one that does
/// something other than add plays.
pub(super) fn play_count<V: Rules, S: LendsCombat>(
    live: &mut Live<V>,
    streams: &mut S,
    relics: &mut crate::run::Belt<V>,
    card: &super::live::CardInstance<V>,
) -> V::Number {
    let player = live.player().id;
    let hook = sts2_core::ModifyHook::CardPlayCount {
        player,
        card: card.face.ident.card,
    };
    let mut count = card.face.play_count();
    let mut modifiers: SmallVec<[(CombatId, V::Power); 2]> = SmallVec::new();
    // A relic's listener carries its state alone, so the walk keeps where
    // that state lives, and the belt names the slot after.
    let mut relic_modifiers: SmallVec<[*const V::RelicState; 1]> = SmallVec::new();
    super::listen::each::<V>(
        super::listen::Walk { live, relics },
        sts2_core::Overrides::of(sts2_core::ModelHook::ModifyCardPlayCount),
        |listener, _| {
            let before = count;
            match <V as sts2_core::Hooks>::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Added(more) => count = count + more,
                answer => unimplemented!("a play count answered with {answer:?}"),
            }
            if count != before {
                match listener {
                    sts2_core::Listener::Power { holder, power, .. } => {
                        modifiers.push((holder, power))
                    }
                    sts2_core::Listener::Relic(state) => relic_modifiers.push(state),
                    _ => unimplemented!(
                        "a listener other than a power or a relic changing a card's play count"
                    ),
                }
            }
        },
    );
    // `Hook.AfterModifyingCardPlayCount` over the modifiers, which does
    // nothing for one that does not override it.
    for (holder, power) in modifiers {
        if !sts2_core::PowerTable::overrides(power)
            .contains(sts2_core::ModelHook::AfterModifyingCardPlayCount)
        {
            continue;
        }
        let Some(at) = live
            .powers
            .of(holder)
            .position(|held| held.power() == power)
        else {
            continue;
        };
        let mut asked = sts2_core::HookEffects::new();
        live.ask_power(
            Some(relics),
            holder,
            at,
            sts2_core::EventHook::PlayCountModified {
                card: card.face.ident.card,
            },
            &mut asked,
        );
        let ascension = live.ascension;
        let mut board = Board::for_card(live, streams.combat(), relics, ascension, None, None);
        for effect in asked {
            match effect {
                sts2_core::HookEffect::Decrement => {
                    board.modify_amount(holder, power, -num_traits::one::<V::Number>())
                }
                effect => unimplemented!("a power asking for {effect:?} after a play count"),
            }
        }
    }
    for state in relic_modifiers {
        let slot = relics
            .iter()
            .position(|held| std::ptr::eq(&held.state, state))
            .expect("a relic that answered the walk is on the belt");
        let held = &mut relics[slot];
        if !sts2_core::RelicTable::overrides(held.relic)
            .contains(sts2_core::ModelHook::AfterModifyingCardPlayCount)
        {
            continue;
        }
        let mut asked = sts2_core::HookEffects::new();
        V::relic_event_hook(
            &mut held.state,
            sts2_core::EventHook::PlayCountModified {
                card: card.face.ident.card,
            },
            &mut asked,
        );
        if let Some(effect) = asked.first() {
            unimplemented!("a relic asking for {effect:?} after a play count");
        }
    }
    count
}

/// Resume the card play the fight parked on a selection.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn answer<V: Rules>(
    fighting: Fighting<'_, V>,
    chosen: sts2_core::Picks,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    let asking = live
        .asking
        .take()
        .unwrap_or_else(|| unimplemented!("an answer to a fight that asked nothing"));
    // What this answer's play defers goes above what already waits.
    live.deferring_at = None;
    let taken =
        match &asking.question {
            Asked::Pile {
                pile: from,
                selection,
            } => {
                // An answer names a card by where it was offered, which is
                // not always where it lies.
                let offered = live.offered(*from);
                let cards: Option<sts2_core::Picked<V>> = chosen
                    .iter()
                    .map(|index| {
                        let at = *offered.get(index)?;
                        live.cards.get(*from, at).map(|card| {
                            let at = num_traits::NumCast::from(at).unwrap_or_else(|| {
                                unimplemented!("a pile deeper than a fight can number")
                            });
                            sts2_core::CardAt::new(card.id, *from, at)
                        })
                    })
                    .collect();
                let cards = cards
                    .unwrap_or_else(|| unimplemented!("a selection naming a card not offered"));
                sts2_core::Taken::Selected(selection.validate_answer(cards).unwrap_or_else(|| {
                    unimplemented!("a selection with the wrong number of cards")
                }))
            }
            // Each card named where it stood as the selection asked, which
            // nothing moves while it waits.
            Asked::Among { cards, selection } => {
                let picked: Option<sts2_core::Picked<V>> = chosen
                    .iter()
                    .map(|index| cards.get(index).copied())
                    .collect();
                let picked = picked
                    .unwrap_or_else(|| unimplemented!("a selection naming a card not offered"));
                sts2_core::Taken::Selected(selection.validate_answer(picked).unwrap_or_else(|| {
                    unimplemented!("a selection with the wrong number of cards")
                }))
            }
            Asked::Offered { offered, skippable } => {
                let chosen = if chosen.is_empty() {
                    if !*skippable {
                        unimplemented!("skipping a choice that cannot be skipped");
                    }
                    None
                } else {
                    let Some(only) = chosen.only() else {
                        unimplemented!("a choice taking more than one card");
                    };
                    Some(*offered.get(only).unwrap_or_else(|| {
                        unimplemented!("a choice naming a card that was not offered")
                    }))
                };
                sts2_core::Taken::Chosen(chosen)
            }
        };

    let (mut effect, at, outermost) = match asking.then {
        Then::Play {
            effect,
            at,
            outermost,
        } => (effect, at, outermost),
        Then::Setup { act } => {
            let sts2_core::Taken::Selected(picked) = taken else {
                unreachable!("a turn's setup asks for cards of a pile")
            };
            let ascension = live.ascension;
            let ended = super::turn::answer_setup::<V>(live, streams.combat(), relics, act, picked);
            return run_deferred(
                Fighting {
                    live,
                    streams,
                    relics,
                },
                ascension,
                ended,
            );
        }
        // A shuffle's pick inside an effect's draw: the cards picked, then
        // the rest of the draw and the effect behind it.
        Then::Pick { act } => {
            let sts2_core::Taken::Selected(picked) = taken else {
                unreachable!("a shuffle asks for cards of a pile")
            };
            let ascension = live.ascension;
            let (sly, ended) =
                super::turn::act_on::<V>(live, streams.combat(), relics, act, picked.as_slice());
            let ended = ended
                .or_else(|| super::turn::play_sly::<V>(live, streams.combat(), relics, sly, true));
            return run_deferred(
                Fighting {
                    live,
                    streams,
                    relics,
                },
                ascension,
                ended,
            );
        }
        // The choice a monster's move offered, then the rest of the turn.
        Then::Chosen { at } => {
            let ascension = live.ascension;
            let ended = super::turn::resume_chosen::<V>(
                Fighting {
                    live: &mut *live,
                    streams: &mut *streams,
                    relics: &mut *relics,
                },
                at,
                taken,
            );
            return run_deferred(
                Fighting {
                    live,
                    streams,
                    relics,
                },
                ascension,
                ended,
            );
        }
    };
    let playing = match &effect.asker {
        Asker::Card(card) => Some(card.id),
        Asker::Potion(_) => None,
    };
    let ascension = live.ascension;
    let mut board = Board::for_card(
        live,
        streams.combat(),
        relics,
        ascension,
        effect.target,
        playing,
    )
    .blocking();
    let played = V::resume_play(&mut board, at, taken);
    effect.ended = effect.ended.or(board.ended());
    let skipped = std::mem::take(&mut board.skipped);
    merge::<V>(&mut live.skip_tick, skipped);
    let ended = carry_on(
        Fighting {
            live: &mut *live,
            streams: &mut *streams,
            relics: &mut *relics,
        },
        ascension,
        effect,
        played,
        outermost,
    )?;
    run_deferred(
        Fighting {
            live,
            streams,
            relics,
        },
        ascension,
        ended,
    )
}

/// What waited behind an answered question, run until nothing does or
/// another question is asked. An auto-play waiting on a fight already over
/// never runs, and the effects it stood inside still end.
fn run_deferred<V: Rules>(
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    mut ended: Option<super::Ended>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    while live.asking.is_none() {
        let Some(deferred) = live.deferred.pop() else {
            break;
        };
        live.deferring_at = None;
        let fighting = Fighting {
            live: &mut *live,
            streams: &mut *streams,
            relics: &mut *relics,
        };
        ended = match deferred {
            super::live::Deferred::AutoPlay { id, named } => {
                let from = [
                    sts2_core::Pile::Discard,
                    sts2_core::Pile::Draw,
                    sts2_core::Pile::Hand,
                    sts2_core::Pile::Exhaust,
                ]
                .into_iter()
                .find(|&pile| fighting.live.cards.find(pile, id).is_some());
                match (ended, from) {
                    (None, Some(from)) => auto_play::<V>(
                        fighting.live,
                        fighting.streams.combat(),
                        fighting.relics,
                        id,
                        from,
                        named,
                        true,
                    ),
                    (ended, _) => ended,
                }
            }
            super::live::Deferred::Resume {
                mut effect,
                played,
                outermost,
            } => {
                effect.ended = effect.ended.or(ended);
                carry_on(fighting, ascension, effect, played, outermost)?
            }
            super::live::Deferred::Finish {
                mut effect,
                outermost,
            } => {
                effect.ended = effect.ended.or(ended);
                finish_effect(fighting, ascension, effect, outermost)?
            }
            parked @ super::live::Deferred::Move { .. } => {
                let resumed = match ended {
                    None => super::turn::resume_move::<V>(
                        Fighting {
                            live: &mut *fighting.live,
                            streams: &mut *fighting.streams,
                            relics: &mut *fighting.relics,
                        },
                        &parked,
                    ),
                    ended => Some(ended),
                };
                match resumed {
                    // It stood still again, and waits once more.
                    None => ended,
                    Some(ended) => {
                        let super::live::Deferred::Move {
                            actor,
                            move_id,
                            acting,
                            max_energy,
                            ..
                        } = parked
                        else {
                            unreachable!("a move was parked")
                        };
                        super::turn::after_move::<V>(
                            fighting, actor, move_id, &acting, max_energy, ended,
                        )?
                    }
                }
            }
            super::live::Deferred::Setup(stage) => super::turn::finish_setup::<V>(
                fighting.live,
                fighting.streams.combat(),
                fighting.relics,
                ascension,
                stage,
                ended,
            ),
            held @ super::live::Deferred::Hits { .. } => match ended {
                None => super::board::land_held::<V>(
                    fighting.live,
                    fighting.streams.combat(),
                    fighting.relics,
                    ascension,
                    held,
                ),
                ended => ended,
            },
            super::live::Deferred::Draw {
                count,
                hand_draw,
                resumed,
            } => match ended {
                None => super::board::draw_in_effect::<V>(
                    fighting.live,
                    fighting.streams.combat(),
                    fighting.relics,
                    ascension,
                    count,
                    hand_draw,
                    resumed,
                ),
                ended => ended,
            },
        };
    }
    Ok(ended)
}

/// Carry a play on from wherever it got to.
///
/// `OnPlay` is a coroutine in the game. A card that has to ask says so and
/// comes back here with the answer. The first call and each resumption use
/// this loop.
///
/// `outermost` is a play no other is running inside, which finishes a fight
/// it won. A play nested in another leaves that to what it is nested in.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn carry_on<V: Rules, S: LendsCombat>(
    fighting: Fighting<'_, V, S>,
    ascension: V::AscensionLevel,
    mut effect: Resuming<V>,
    mut played: sts2_core::Played<V>,
    outermost: bool,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live: next,
        streams,
        relics,
    } = fighting;
    loop {
        // A card this effect auto-played asked a question, and the effect
        // stands where its own code left it until that card's play is over.
        if next.asking.is_some() {
            let ended = effect.ended;
            next.defer(super::live::Deferred::Resume {
                effect,
                played,
                outermost,
            });
            return Ok(ended);
        }
        // The effect waited on a card it auto-played, whose play is over.
        if let sts2_core::Played::Waiting { at } = played {
            let playing = match &effect.asker {
                Asker::Card(card) => Some(card.id),
                Asker::Potion(_) => None,
            };
            let mut board = Board::for_card(
                next,
                streams.combat(),
                relics,
                ascension,
                effect.target,
                playing,
            )
            .blocking();
            played = V::resume_play(&mut board, at, sts2_core::Taken::Waited);
            effect.ended = effect.ended.or(board.ended());
            let skipped = std::mem::take(&mut board.skipped);
            merge::<V>(&mut next.skip_tick, skipped);
            continue;
        }
        let (at, question, answer) = match played {
            // One turn of `GeneratePlayCount`'s loop is over. A card carrying
            // replays runs its own effect again, from the top.
            sts2_core::Played::Done => {
                effect.plays.done = effect.plays.done + num_traits::one();
                let Asker::Card(play) = effect.asker else {
                    break;
                };
                let card = play.id;
                let face = next
                    .cards
                    .named(card)
                    .unwrap_or_else(|| {
                        unimplemented!("a card that left play before its play ended")
                    })
                    .face;
                let played_card = face.ident.card;
                let ethereal = face.ethereal();
                // `EnchantmentModel.OnPlay`, after the card's own and before
                // the play is finished.
                if let Some(enchanted) = face.ident.enchantment {
                    let mut status = face.enchantment_status();
                    let mut board = Board::for_card(
                        next,
                        streams.combat(),
                        relics,
                        ascension,
                        effect.target,
                        Some(card),
                    )
                    .blocking();
                    V::enchantment_on_play(enchanted, &mut status, &mut board);
                    effect.ended = effect.ended.or(board.ended());
                    let skipped = std::mem::take(&mut board.skipped);
                    merge::<V>(&mut next.skip_tick, skipped);
                    next.cards
                        .write_in_play(card, |face| face.set_enchantment_status(status));
                }
                let ended = finished::<V, _>(
                    Fighting {
                        live: &mut *next,
                        streams: &mut *streams,
                        relics: &mut *relics,
                    },
                    ascension,
                    played_card,
                    card,
                    ethereal,
                );
                effect.ended = effect.ended.or(ended);
                if effect.plays.done >= effect.plays.count {
                    break;
                }
                if next.asking.is_some() {
                    unimplemented!("a replay waiting behind a question its last play's hook asked");
                }
                let replaying = next
                    .cards
                    .named(card)
                    .expect("a card replaying stands in the play pile");
                before_played::<V, _>(
                    next,
                    streams,
                    relics,
                    &replaying,
                    play.energy,
                    effect.target,
                );
                next.history.start(played_card, card, play.energy, false);
                let mut board = Board::for_card(
                    next,
                    streams.combat(),
                    relics,
                    ascension,
                    effect.target,
                    Some(card),
                )
                .blocking();
                played = V::on_play(played_card, &mut board);
                effect.ended = effect.ended.or(board.ended());
                let skipped = std::mem::take(&mut board.skipped);
                merge::<V>(&mut next.skip_tick, skipped);
                continue;
            }
            sts2_core::Played::Asking {
                pile,
                selection,
                at,
            } => (
                at,
                Asked::Pile { pile, selection },
                taken::<V>(next, pile, selection).map(sts2_core::Taken::Selected),
            ),
            // `FromHand` over what its filter lets through, which takes them
            // all unasked the way an unfiltered selection does.
            sts2_core::Played::AskingAmong {
                cards,
                selection,
                at,
            } => {
                let taken = selection
                    .validate_automatic(cards.iter().copied().collect())
                    .map(sts2_core::Taken::Selected);
                (at, Asked::Among { cards, selection }, taken)
            }
            sts2_core::Played::Choosing {
                offered,
                skippable,
                at,
            } => (at, Asked::Offered { offered, skippable }, None),
            sts2_core::Played::Waiting { .. } => unreachable!("a waiting effect resumed above"),
        };
        let Some(answer) = answer else {
            let ended = effect.ended;
            next.asking = Some(super::live::Asking {
                then: Then::Play {
                    effect,
                    at,
                    outermost,
                },
                question,
            });
            return Ok(ended);
        };

        let playing = match &effect.asker {
            Asker::Card(card) => Some(card.id),
            Asker::Potion(_) => None,
        };
        let mut board = Board::for_card(
            next,
            streams.combat(),
            relics,
            ascension,
            effect.target,
            playing,
        )
        .blocking();
        played = V::resume_play(&mut board, at, answer);
        effect.ended = effect.ended.or(board.ended());
        let skipped = std::mem::take(&mut board.skipped);
        merge::<V>(&mut next.skip_tick, skipped);
    }

    // A question a hook of the last play asked stands the end still too.
    if next.asking.is_some() {
        let ended = effect.ended;
        next.defer(super::live::Deferred::Finish { effect, outermost });
        return Ok(ended);
    }
    finish_effect(
        Fighting {
            live: next,
            streams,
            relics,
        },
        ascension,
        effect,
        outermost,
    )
}

/// The end of an effect whose last play is over: `Hook.AfterPotionUsed` for
/// a potion, and for a card the pile its play sends it to.
fn finish_effect<V: Rules, S: LendsCombat>(
    fighting: Fighting<'_, V, S>,
    ascension: V::AscensionLevel,
    effect: Resuming<V>,
    outermost: bool,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live: next,
        streams,
        relics,
    } = fighting;
    let mut ended = effect.ended;
    let Asker::Card(play) = effect.asker else {
        // `Hook.AfterPotionUsed`, once the potion is done, for a fight still
        // in progress.
        if ended.is_none() {
            let asked = super::listen::notify::<V>(
                next,
                relics,
                sts2_core::EventHook::PotionUsed { in_fight: true },
            );
            if !asked.is_empty() {
                let mut board =
                    Board::for_card(next, streams.combat(), relics, ascension, None, None);
                hook::apply::<V, _>(&mut board, asked.into_iter().map(|(_, effect)| effect));
                ended = board.ended();
            }
        }
        if outermost && ended == Some(super::Ended::Won) {
            super::listen::combat_ended::<V>(next, relics);
        }
        return Ok(ended);
    };

    // `AfterCardPlayedCleanup`, wherever the card went.
    next.cards
        .write_named(play.id, super::live::Face::was_played);

    // `Pile.Type == PileType.Play`: a card that put itself elsewhere while it
    // ran stays there.
    if !next.cards.in_play(play.id) {
        if outermost && ended == Some(super::Ended::Won) {
            super::listen::combat_ended::<V>(next, relics);
        }
        return Ok(ended);
    }

    // Where the result pile said, worked out before the first play. A full
    // hand sends the card to the discard pile instead.
    let to = match play.pile {
        Some(sts2_core::Pile::Hand)
            if next.cards.len(sts2_core::Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND =>
        {
            Some(sts2_core::Pile::Discard)
        }
        pile => pile,
    };
    let exhausting = match to {
        Some(sts2_core::Pile::Exhaust) => next.cards.named(play.id),
        _ => None,
    };
    next.cards.leave_play(play.id, to, play.position);
    // `CardCmd.Exhaust` for a card that exhausts as it is played, whose hook
    // follows the card into the pile. A fight already over exhausts nothing.
    let ended = match (ended, exhausting) {
        (None, Some(card)) => {
            super::turn::exhausted::<V>(next, streams.combat(), relics, &card, false)
        }
        (ended, _) => ended,
    };

    if outermost && ended == Some(super::Ended::Won) {
        super::listen::combat_ended::<V>(next, relics);
    }
    Ok(ended)
}

/// The end of one play of `card`: `History.CardPlayFinished`, then
/// `AfterCardPlayed`, with the card still in the play pile.
///
/// Once for each play, so a card with replays finishes more than once. A
/// relic that answers acts through the board a card acts through, and draws
/// from the same stream.
fn finished<V: Rules, S: LendsCombat>(
    fighting: Fighting<'_, V, S>,
    ascension: V::AscensionLevel,
    card: V::Card,
    id: sts2_core::CardId<V>,
    ethereal: bool,
) -> Option<super::Ended> {
    let Fighting {
        live: next,
        streams,
        relics,
    } = fighting;
    next.history.finish(card, id, ethereal);
    let mut ended = None;
    // The copy stays in the play pile until its play is over.
    let affliction = next
        .cards
        .named(id)
        .and_then(|held| held.face.ident.affliction);
    // What the play's started entry spent, which both `AfterCardPlayed` and
    // its late pass read.
    let spent = next
        .history()
        .last_started(id)
        .expect("a play that finished started this turn")
        .energy;
    let asked = super::listen::notify::<V>(
        next,
        relics,
        sts2_core::EventHook::CardPlayed {
            card,
            id,
            affliction,
            ethereal,
            spent,
        },
    );
    if !asked.is_empty() {
        let mut rest: SmallVec<[sts2_core::HookEffect<V>; 4]> = SmallVec::new();
        for (listener, effect) in asked {
            match (listener, effect) {
                // Written on the card that asked, wherever it stands.
                (
                    super::listen::Listener::Card { id },
                    sts2_core::HookEffect::ModifySelf { change, lasting },
                ) => next.modify_self(id, change, lasting),
                // The card is still in the play pile until its play is over.
                (
                    super::listen::Listener::Card { id },
                    sts2_core::HookEffect::DisableOwnEnchantment,
                ) => next.cards.write_in_play(id, |face| {
                    face.set_enchantment_status(sts2_core::EnchantmentStatus::Disabled)
                }),
                // `CreatureCmd.GainBlock`, unpowered, on the power's owner,
                // then `PowerCmd.Remove(this)`.
                (
                    super::listen::Listener::Power { owner, power, .. },
                    sts2_core::HookEffect::GainBlockAndRemove(amount),
                ) => {
                    next.gain_unpowered_block(owner, amount);
                    next.remove_power(owner, power);
                }
                // `CreatureCmd.Damage` on the power's owner, unpowered.
                (
                    super::listen::Listener::Power { owner, .. },
                    sts2_core::HookEffect::DamageOwnerInFight {
                        amount,
                        unblockable,
                        self_dealt,
                    },
                ) => {
                    let mut board =
                        Board::for_card(next, streams.combat(), relics, ascension, None, None);
                    board.hit_unpowered(self_dealt.then_some(owner), owner, amount, unblockable);
                    ended = ended.or(board.ended());
                }
                // Made by the power's owner, which for a monster's power is
                // nobody.
                (
                    super::listen::Listener::Power { owner, .. },
                    sts2_core::HookEffect::GenerateInHand { card, count },
                ) => {
                    let player = next.player().id;
                    let count = num_traits::cast(count)
                        .unwrap_or_else(|| unimplemented!("{count:?} cards generated"));
                    let mut board =
                        Board::for_card(next, streams.combat(), relics, ascension, None, None);
                    board.generate(
                        sts2_core::CardIdent::new(card),
                        sts2_core::Pile::Hand,
                        count,
                        (owner == player).then_some(player),
                    );
                    ended = ended.or(board.ended());
                }
                // `PowerCmd.Apply` on the power's owner, from its owner, which
                // for a monster's power is not the player playing the card.
                (
                    super::listen::Listener::Power { owner, .. },
                    sts2_core::HookEffect::ApplyToOwner(power, amount),
                ) => {
                    let mut board =
                        Board::for_card(next, streams.combat(), relics, ascension, None, None);
                    board.land(owner, power, amount, Some(owner));
                    ended = ended.or(board.ended());
                }
                // Clones of the card just played, which is still in the
                // play pile, into hand.
                (
                    super::listen::Listener::Power { .. },
                    sts2_core::HookEffect::CloneIntoHand { card, count },
                ) => {
                    let count: usize = num_traits::cast(count)
                        .unwrap_or_else(|| unimplemented!("{count:?} clones"));
                    let mut board =
                        Board::for_card(next, streams.combat(), relics, ascension, None, None);
                    for _ in 0..count {
                        board.clone_named(card, sts2_core::Pile::Hand);
                    }
                    ended = ended.or(board.ended());
                }
                // `PowerCmd.Apply` from nobody.
                (
                    super::listen::Listener::Power { .. },
                    sts2_core::HookEffect::ApplyPower {
                        target,
                        power,
                        amount,
                    },
                ) => {
                    let mut board =
                        Board::for_card(next, streams.combat(), relics, ascension, None, None);
                    board.land(target, power, amount, None);
                    ended = ended.or(board.ended());
                }
                (_, effect) => rest.push(effect),
            }
        }
        let mut board = Board::for_card(next, streams.combat(), relics, ascension, None, None);
        hook::apply::<V, _>(&mut board, rest);
        ended = ended.or(board.ended());
        let skipped = std::mem::take(&mut board.skipped);
        merge::<V>(&mut next.skip_tick, skipped);
    }
    // `AfterCardPlayedLate`, with the energy the play's started entry spent.
    if ended.is_none() {
        let late = sts2_core::EventHook::CardPlayedLate {
            card,
            id,
            energy: spent,
        };
        for (listener, effect) in super::listen::notify::<V>(next, relics, late) {
            match (listener, effect) {
                // `CardPileCmd.Add(this, PileType.Hand)`, which a full hand
                // turns away.
                (
                    super::listen::Listener::Card { id },
                    sts2_core::HookEffect::AddSelfToHand { only_from },
                ) => {
                    let from = next.cards.pile_of(id).filter(|&pile| {
                        pile != sts2_core::Pile::Hand && only_from.is_none_or(|only| only == pile)
                    });
                    if let Some(from) = from
                        && next.cards.len(sts2_core::Pile::Hand) < super::pile::MAX_CARDS_IN_HAND
                        && let Some(at) = next.cards.find(from, id)
                    {
                        next.cards.move_to(from, at, sts2_core::Pile::Hand);
                    }
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} late after a play")
                }
            }
        }
    }
    ended
}

/// What a selection takes without asking, or nothing for one that is a
/// question.
///
/// `FromHand`: an empty pile selects nothing, and a pile no larger than an
/// exact selection is taken whole. Anything else the player answers.
pub(super) fn taken<V: Rules>(
    live: &Live<V>,
    from: sts2_core::Pile,
    selection: sts2_core::Selection,
) -> Option<sts2_core::SelectedCards<V>> {
    let cards = live
        .cards
        .iter(from)
        .enumerate()
        .map(|(index, card)| {
            let index = num_traits::NumCast::from(index)
                .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
            sts2_core::CardAt::new(card.id, from, index)
        })
        .collect();
    selection.validate_automatic(cards)
}
