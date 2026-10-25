//! Which cards in hand can be played, and what each one does.
//!
//! `CardModel.CanPlay` starts at no reasons and adds one per rule it fails.
//! So this reports a set, rather than the first refusal.

use crate::legal::{Legal, aimed, belt_index, hand_index};
use smallvec::{SmallVec, smallvec};
use sts2_core::{
    CardIdent, CardTable, CombatId, Cost, Decision, EffectResolution, FlagSet, Held, Pile,
    Playable, PotionTable, PotionUsage, Preview, Rules, SideKinds, TargetKinds, Targets,
    Undrinkable, UnplayableReasonKinds,
};

use super::listen::Walk;
use super::live::{CardInstance, Fighter, Live};

/// Every card in hand, in hand order, with the reasons each cannot be played.
pub(super) fn hand<V: Rules>(
    seat: sts2_core::SeatIndex,
    hand: impl ExactSizeIterator<Item = CardInstance<V>>,
    fight: Walk<'_, V>,
) -> Vec<Playable<V>> {
    let facts = HandFacts::of(seat, fight);
    hand.map(|card| InHand::new(seat, card, fight, &facts).playable())
        .collect()
}

/// What `CanPlay` reads off the fight rather than the card, which is the same
/// for every card in hand. Worked out once a hand.
pub(super) struct HandFacts<V: Rules> {
    energy: V::Count,
    stars: V::Count,
    /// Whether anything in the fight answers `ShouldPlay`.
    asks: bool,
}

impl<V: Rules> HandFacts<V> {
    #[inline(always)]
    pub(super) fn of(seat: sts2_core::SeatIndex, fight: Walk<'_, V>) -> Self {
        let live = fight.live;
        Self {
            energy: live.players[seat].energy,
            stars: live.players[seat].stars,
            asks: asks_should_play::<V>(fight),
        }
    }
}

/// The fight as a card's own `IsPlayable` reads it.
struct Logic<'a, V: Rules>(&'a Live<V>, sts2_core::SeatIndex);

impl<V: Rules> sts2_core::CardLogic for Logic<'_, V> {
    fn draw_pile(&self) -> usize {
        self.0.players[self.1].cards.len(Pile::Draw)
    }

    fn osty(&self) -> bool {
        super::board::osty::<V>(&self.0.creatures).is_some_and(|osty| osty.alive)
    }

    fn hand_all_attacks(&self) -> bool {
        (self.0.players[self.1].cards.iter(Pile::Hand))
            .all(|held| V::card_kind(held.face.ident.card) == Some(sts2_core::CardKind::Attack))
    }
}

/// A card in hand that `CanPlay`, as an auto-play that pays for it reads it.
pub(super) struct Payable<V: Rules> {
    pub(super) held: CardInstance<V>,
    /// What it costs now, in energy and in stars.
    pub(super) costs: (Cost, Cost),
    pub(super) target: V::TargetType,
}

/// The hand's first card that `CanPlay`, with no previews worked out.
pub(super) fn first_playable<V: Rules>(
    seat: sts2_core::SeatIndex,
    hand: impl Iterator<Item = CardInstance<V>>,
    fight: Walk<'_, V>,
) -> Option<Payable<V>> {
    let facts = HandFacts::of(seat, fight);
    hand.map(|held| InHand::new(seat, held, fight, &facts))
        .find(|card| card.reason.is_none())
        .map(|card| Payable {
            held: card.held,
            costs: (card.cost, card.star_cost),
            target: card.target,
        })
}

/// What a card names as its target.
#[inline(always)]
pub(super) fn target_type<V: Rules>(card: CardIdent<V>, live: &Live<V>) -> V::TargetType {
    // One of the four cards that work their target out from the fight, which
    // they do by reading the player's own powers.
    card.card
        .target()
        .unwrap_or_else(|| V::computed_card_target(card.card, &super::board::powers::<V>(live)))
}

/// Whether anything in the fight answers `ShouldPlay`, asked once for a hand
/// rather than once for each card in it.
fn asks_should_play<V: Rules>(fight: Walk<'_, V>) -> bool {
    super::listen::heard::<V>(fight.live, fight.belts, sts2_core::ModelHook::ShouldPlay)
}

/// `EnergyCost.GetWithModifiers` for a copy in hand or being played, which
/// runs `Hook.ModifyEnergyCostInCombat` over its own modifiers: every
/// listener's `TryModifyEnergyCostInCombat`, then every one's `Late`. `pile`
/// is where the copy stands, or nothing for the play pile.
#[inline(always)]
pub(super) fn costs<V: Rules>(
    held: &CardInstance<V>,
    pile: Option<Pile>,
    fight: Walk<'_, V>,
) -> (sts2_core::Cost, sts2_core::Cost) {
    let card = (held.id, held.face.ident.card);
    let ethereal = held.face.ethereal();
    let affliction = held.face.ident.affliction;
    held.face.costs(
        |cost| {
            super::board::modified_by::<V>(
                fight,
                cost,
                &[
                    sts2_core::ModifyHook::EnergyCost {
                        card,
                        ethereal,
                        affliction,
                        pile,
                    },
                    sts2_core::ModifyHook::EnergyCostLate {
                        card,
                        ethereal,
                        affliction,
                        pile,
                    },
                ],
            )
        },
        // `Hook.ModifyStarCost`, which few listen to and a Regent's hand asks
        // on every read.
        |cost| {
            let hook = sts2_core::ModelHook::TryModifyStarCost;
            match super::listen::heard::<V>(fight.live, fight.belts, hook) {
                true => super::board::modified_by::<V>(
                    fight,
                    cost,
                    &[sts2_core::ModifyHook::StarCost { card, pile }],
                ),
                false => cost,
            }
        },
    )
}

/// Why this card cannot be played, or nothing for one that can.
///
/// The set itself rather than a list to fold into one. Six reasons fit a byte
/// the build already generates, and a card in hand is asked this on every
/// report.
///
/// Beside it, the model that answered `ShouldPlay` with no, which is
/// `preventer`. `target` is the card's [`target_type`].
#[inline(always)]
pub(super) fn reasons<V: Rules>(
    seat: sts2_core::SeatIndex,
    card: CardIdent<V>,
    target: V::TargetType,
    fight: Walk<'_, V>,
    (cost, star_cost): (Cost, Cost),
    facts: &HandFacts<V>,
) -> (Option<V::UnplayableReasons>, Option<sts2_core::ModelId<V>>) {
    let HandFacts {
        energy,
        stars,
        asks,
    } = *facts;
    let creatures = &fight.live.creatures;
    let mut why = None;
    if card.card.unplayable() {
        why = refused::<V>(
            why,
            <V::UnplayableReason as UnplayableReasonKinds>::HAS_UNPLAYABLE_KEYWORD,
        );
    }
    if spend(cost, energy).is_none_or(|spent| spent > energy) {
        why = refused::<V>(
            why,
            <V::UnplayableReason as UnplayableReasonKinds>::ENERGY_COST_TOO_HIGH,
        );
    }
    if spend(star_cost, stars).is_none_or(|spent| spent > stars) {
        why = refused::<V>(
            why,
            <V::UnplayableReason as UnplayableReasonKinds>::STAR_COST_TOO_HIGH,
        );
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
        why = refused::<V>(
            why,
            <V::UnplayableReason as UnplayableReasonKinds>::NO_LIVING_ALLIES,
        );
    }
    // `Hook.ShouldPlay`, whose first no names the preventer.
    let preventer = asks
        .then(|| {
            super::listen::first_deciding::<V>(
                fight,
                sts2_core::DecisionHook::ShouldPlay {
                    card: card.card,
                    affliction: card.affliction,
                    auto: false,
                },
                false,
            )
        })
        .flatten();
    if preventer.is_some() {
        why = refused::<V>(
            why,
            <V::UnplayableReason as UnplayableReasonKinds>::BLOCKED_BY_HOOK,
        );
    }
    // `IsPlayable`, the card's own logic.
    if let sts2_core::HookResponse::Decision(false) =
        V::card_is_playable(card.card, &Logic(fight.live, seat))
    {
        why = refused::<V>(
            why,
            <V::UnplayableReason as UnplayableReasonKinds>::BLOCKED_BY_CARD_LOGIC,
        );
    }
    (why, preventer)
}

/// That set with one more reason in it, starting one where there is none.
fn refused<V: Rules>(
    why: Option<V::UnplayableReasons>,
    reason: V::UnplayableReason,
) -> Option<V::UnplayableReasons> {
    Some(match why {
        Some(held) => held.with(reason),
        None => <V::UnplayableReasons as FlagSet>::from_members(&[reason]),
    })
}

/// The creatures this card can legally be pointed at.
///
/// Empty for a card that cannot be played at all, because the game stops
/// asking once a rule has refused it.
#[inline(always)]
pub(super) fn targets<V: Rules>(
    target: V::TargetType,
    creatures: &[Fighter<V>],
    refused: bool,
) -> Targets {
    if refused {
        return Targets::new();
    }
    creatures
        .iter()
        .filter(|creature| valid_target::<V>(target, creature))
        .map(|creature| creature.id)
        .collect()
}

/// One card in hand, with the facts a report and a comparison both read off it.
///
/// Everything but the previews is worked out when this is made. The previews
/// are the dear part, and are worked out when asked for.
pub(crate) struct InHand<'a, V: Rules> {
    /// The seat whose hand it is in.
    seat: sts2_core::SeatIndex,
    held: CardInstance<V>,
    fight: Walk<'a, V>,
    cost: Cost,
    star_cost: Cost,
    target: V::TargetType,
    reason: Option<V::UnplayableReasons>,
    preventer: Option<sts2_core::ModelId<V>>,
    targets: Targets,
}

impl<'a, V: Rules> InHand<'a, V> {
    pub(super) fn new(
        seat: sts2_core::SeatIndex,
        held: CardInstance<V>,
        fight: Walk<'a, V>,
        facts: &HandFacts<V>,
    ) -> Self {
        let creatures = &fight.live.creatures;
        let target = target_type::<V>(held.face.ident, fight.live);
        let (cost, star_cost) = costs::<V>(&held, Some(Pile::Hand), fight);
        let (reason, preventer) = reasons::<V>(
            seat,
            held.face.ident,
            target,
            fight,
            (cost, star_cost),
            facts,
        );
        Self {
            seat,
            held,
            fight,
            cost,
            star_cost,
            target,
            reason,
            preventer,
            targets: targets::<V>(target, creatures, reason.is_some()),
        }
    }

    /// What playing it does, per target it varies by.
    pub(super) fn previews(&self) -> Previews<V> {
        effect::<V>(self.seat, &self.held, self.fight, &self.targets)
    }

    /// The card as a report lists it.
    fn playable(&self) -> Playable<V> {
        // Sized to the previews. `into_vec` keeps the inline capacity, and a
        // `collect` that cannot trust the length starts at four, so either
        // allocates four previews for the one a card usually has.
        let previews = self.previews();
        let mut effect = Vec::with_capacity(previews.len());
        effect.extend(previews);
        Playable {
            card: self.held.face.ident,
            cost: self.cost,
            star_cost: self.star_cost,
            target: self.target,
            reason: self.reason,
            preventer: self.preventer,
            effect,
            targets: self.targets.clone(),
        }
    }
}

impl<V: Rules> sts2_core::ObservedPlayable<V> for InHand<'_, V> {
    fn card(&self) -> CardIdent<V> {
        self.held.face.ident
    }

    fn cost(&self) -> Cost {
        self.cost
    }

    fn star_cost(&self) -> Cost {
        self.star_cost
    }

    fn target(&self) -> V::TargetType {
        self.target
    }

    fn reason(&self) -> Option<V::UnplayableReasons> {
        self.reason
    }

    fn preventer(&self) -> Option<sts2_core::ModelId<V>> {
        self.preventer
    }

    fn targets(&self) -> &[CombatId] {
        &self.targets
    }

    fn effect<R>(&self, read: impl FnOnce(&[Preview<V>]) -> R) -> R {
        read(&self.previews())
    }
}

/// A card's previews, held inline because a comparison asks for them one card
/// at a time and keeps none.
///
/// Four, which holds a card aimed at every enemy the recordings field. A
/// report copies them into its own list.
pub(super) type Previews<V> = SmallVec<[Preview<V>; 4]>;

/// Every decision a live fight accepts, in the order its report implies them.
///
/// What [`sts2_core::legal::implied`] reads off the report, read from the
/// fight instead. The rules are the ones the report runs: the belt's reasons,
/// and each card's reasons and targets. Nothing computes a preview, and
/// nothing is collected but the moves, which go on the end of `moves`.
pub(crate) fn moves<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &Live<V>,
    belts: &crate::run::Belts<V>,
    potions: &[V::Potion],
    alive: bool,
    moves: &mut Legal<V>,
) {
    drinks::<V>(seat, potions, Some(live), false, alive, false, moves);

    let facts = HandFacts::of(seat, Walk { live, belts });
    for (at, card) in live.players[seat].cards.iter(Pile::Hand).enumerate() {
        let costs = costs::<V>(&card, Some(Pile::Hand), Walk { live, belts });
        let target = target_type::<V>(card.face.ident, live);
        let (refused, _) = reasons::<V>(
            seat,
            card.face.ident,
            target,
            Walk { live, belts },
            costs,
            &facts,
        );
        if refused.is_none() {
            let targets = targets::<V>(target, &live.creatures, false);
            // A card that names a creature has nothing to be played at while
            // none is alive to name.
            if targets.is_empty() && names_creature::<V>(target) {
                continue;
            }
            aimed(hand_index(at), &targets, moves, |index, target| {
                Decision::PlayCard { index, target }
            });
        }
    }
    moves.push(Decision::EndTurn);
}

/// Every drink the belt allows, aimed at each creature it can be.
///
/// What [`belt`] reports, as moves: a potion with no reason against it, aimed
/// only inside a live fight, where there are creatures to name.
pub(crate) fn drinks<V: Rules>(
    seat: sts2_core::SeatIndex,
    potions: &[V::Potion],
    fight: Option<&Live<V>>,
    merchant: bool,
    alive: bool,
    stuck: bool,
    moves: &mut Legal<V>,
) {
    for (at, &potion) in potions.iter().enumerate() {
        if undrinkable::<V>(seat, potion, fight, merchant, alive, stuck).is_some() {
            continue;
        }
        let targets = match fight {
            Some(fight) => aimable::<V>(potion, &fight.creatures),
            None => Targets::new(),
        };
        // `IsValidTarget` refuses no target for a potion thrown at one other
        // creature, so with none to throw it at there is nothing to drink.
        if targets.is_empty() && thrown::<V>(potion) {
            continue;
        }
        aimed(belt_index(at), &targets, moves, |index, target| {
            Decision::UsePotion { index, target }
        });
    }
}

/// The belt, and why each potion in it cannot be drunk.
///
/// The order `NPotionPopup.RefreshButtons` asks in, minus the questions this
/// slice cannot raise. Nothing is queued, because a decision resolves whole.
/// `stuck` is `CanRemovePotions` off, which an event sets while it runs.
/// `merchant` is whether the room has a merchant a Foul Potion can be
/// thrown at, which [`Rules::computed_potion_check`] reads.
pub(crate) fn belt<'a, V: Rules>(
    seat: sts2_core::SeatIndex,
    potions: &'a [V::Potion],
    fight: Option<&'a Live<V>>,
    merchant: bool,
    alive: bool,
    stuck: bool,
) -> impl ExactSizeIterator<Item = Held<V>> + 'a {
    potions.iter().map(move |&potion| {
        let reason = undrinkable::<V>(seat, potion, fight, merchant, alive, stuck);
        Held {
            potion,
            // The game reads both out of its localisation tables, which
            // the simulator does not have.
            title: None,
            text: None,
            // The creatures it could be pointed at whether or not it can be
            // drunk now, as a potion waiting out the enemy's turn still
            // names them. Outside a fight, and once the player has died and
            // the fight is no longer in progress, there are none.
            targets: match fight {
                Some(fight) if alive => aimable::<V>(potion, &fight.creatures),
                _ => Targets::new(),
            },
            reason,
        }
    })
}

/// Why one potion cannot be drunk, or nothing for one that can.
///
/// The order is the game's, so the reason reported is the first that holds.
/// `NPotionPopup.RefreshButtons` asks whether the player is dead before it
/// asks anything else. A dead run answers for every potion in the belt,
/// whatever else is true of one.
fn undrinkable<V: Rules>(
    seat: sts2_core::SeatIndex,
    potion: V::Potion,
    fight: Option<&Live<V>>,
    merchant: bool,
    alive: bool,
    stuck: bool,
) -> Option<Undrinkable> {
    if !alive {
        return Some(Undrinkable::Dead);
    }
    if stuck {
        return Some(Undrinkable::Stuck);
    }
    if potion.usage() == PotionUsage::Automatic {
        return Some(Undrinkable::Automatic);
    }
    match fight {
        None if potion.usage() == PotionUsage::CombatOnly => return Some(Undrinkable::OutOfCombat),
        None => {}
        // `CurrentSide` not the player's, or `PlayerActionsDisabled`, which
        // ending the turn sets and the next player turn's start clears. So a
        // question asked as the turn starts leaves potions drinkable. Only a
        // `CombatOnly` potion asks, and one usable any time does not.
        Some(fight)
            if potion.usage() == PotionUsage::CombatOnly && !drinking_turn::<V>(seat, fight) =>
        {
            return Some(Undrinkable::NotYourTurn);
        }
        Some(_) => {}
    }
    let passes = potion
        .passes_own_check()
        .unwrap_or_else(|| V::computed_potion_check(potion, fight.is_some(), merchant));
    (!passes).then_some(Undrinkable::ItsOwnCheck)
}

/// Whether the player's side holds the turn, with actions not disabled:
/// from the turn's start until the player ends it. Where a build's selection
/// takes back an ended turn, a question asked as the turn ends lifts it too.
fn drinking_turn<V: Rules>(seat: sts2_core::SeatIndex, fight: &Live<V>) -> bool {
    use sts2_core::TurnPhaseKinds as _;
    type Phase<V> = <V as sts2_core::ContentVersion>::PlayerTurnPhase;
    fight.side == <V::CombatSide as SideKinds>::PLAYER
        && ([
            Phase::<V>::START,
            Phase::<V>::AUTO_PRE_PLAY,
            Phase::<V>::PLAY,
        ]
        .contains(&fight.players[seat].phase)
            || V::selection_undoes_ended_turn() && fight.asking.is_some())
}

/// The creatures a potion can be pointed at.
/// Whether `potion` is thrown at one creature other than its drinker, which
/// `AnyEnemy` and `AnyAlly` are.
pub(crate) fn thrown<V: Rules>(potion: V::Potion) -> bool {
    let target = potion
        .target()
        .unwrap_or_else(|| V::computed_potion_target(potion, true));
    target == <V::TargetType as TargetKinds>::ANY_ENEMY
        || target == <V::TargetType as TargetKinds>::ANY_ALLY
}

fn aimable<V: Rules>(potion: V::Potion, creatures: &[Fighter<V>]) -> Targets {
    let target = potion
        .target()
        .unwrap_or_else(|| V::computed_potion_target(potion, true));
    creatures
        .iter()
        .filter(|creature| {
            creature.is_alive()
                && match target {
                    t if t == <V::TargetType as TargetKinds>::ANY_ENEMY => {
                        creature.side == <V::CombatSide as SideKinds>::ENEMY
                    }
                    t if t == <V::TargetType as TargetKinds>::ANY_ALLY => {
                        creature.side == <V::CombatSide as SideKinds>::PLAYER
                    }
                    _ => false,
                }
        })
        .map(|creature| creature.id)
        .collect()
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
        Cost::Fixed(n) => num_traits::cast(n.get()),
        Cost::None => Some(C::zero()),
    }
}

/// `CardModel.IsValidTarget`, for a creature offered as this card's target.
/// Whether a card of this target type is played at a creature, which is
/// `IsValidTarget` refusing it none.
pub(super) fn names_creature<V: Rules>(target: V::TargetType) -> bool {
    target == <V::TargetType as TargetKinds>::ANY_ENEMY
        || target == <V::TargetType as TargetKinds>::ANY_ALLY
}

fn valid_target<V: Rules>(target: V::TargetType, creature: &Fighter<V>) -> bool {
    if !creature.is_alive() {
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
/// target. Identical previews collapse into one with no target, which is what
/// a card the fight moves no number of reports.
fn effect<V: Rules>(
    seat: sts2_core::SeatIndex,
    held: &CardInstance<V>,
    fight: Walk<'_, V>,
    targets: &[CombatId],
) -> Previews<V> {
    let per_target = matches!(
        Some(target_type::<V>(held.face.ident, fight.live)),
        Some(t) if t == <V::TargetType as TargetKinds>::ANY_ENEMY
            || t == <V::TargetType as TargetKinds>::ANY_ALLY
            || t == <V::TargetType as TargetKinds>::ANY_PLAYER
    );

    let numbers = numbers::<V>(held);
    // Inline, because the common list is the one entry a card that names no
    // target previews against, and this is built for every card in hand.
    let against: SmallVec<[Option<CombatId>; 4]> = if per_target && !targets.is_empty() {
        targets.iter().copied().map(Some).collect()
    } else {
        smallvec![None]
    };

    // Sized up front. A `SmallVec` does not promise its length the way a
    // `Vec` does, and a collect that cannot read one grows by doubling.
    let mut previews: Previews<V> = Previews::with_capacity(against.len());
    previews.extend(against.into_iter().map(|target| Preview {
        target,
        // The game formats this from its localisation tables, which the
        // simulator does not have. `Observation::rules_digest` leaves it
        // out for that reason, and a front end reads the numbers.
        text: None,
        vars: previewed::<V>(seat, held, fight, target),
        base: numbers.clone(),
    }));

    if previews.len() > 1
        && previews[1..]
            .iter()
            .all(|preview| preview.vars == previews[0].vars)
    {
        previews.truncate(1);
        previews[0].target = None;
    }
    previews
}

/// The card's numbers after the fight has had its say, which is
/// `DynamicVar.UpdateCardPreview` for each of them.
///
/// Each number becomes the effect its own class names and is resolved, which
/// is the same call the card makes when it is played. Damage runs against the
/// creature the card is pointed at, and block for the player about to gain it.
///
/// # Panics
///
/// A number whose class the simulator has nowhere to run, which is the Osty's
/// damage. No character the simulator plays has one.
fn previewed<V: Rules>(
    seat: sts2_core::SeatIndex,
    held: &CardInstance<V>,
    fight: Walk<'_, V>,
    target: Option<CombatId>,
) -> sts2_core::Keyed<V::CardVar, V::Number> {
    let creatures = &fight.live.creatures;
    held.face
        .numbers()
        .map(|var| {
            let printed = || super::board::var::<V>(seat, held, var.id, fight, target);
            let resolved = match (var.modifier, target) {
                // `UpdateCardPreview` hands the null target to
                // `Hook.ModifyDamage` rather than skipping it, so a hit
                // previewed against nobody still runs the dealer's own powers
                // and only the ones reading the target sit it out.
                (
                    Some(
                        modifier @ (sts2_core::CardNumberModifier::Damage
                        | sts2_core::CardNumberModifier::OstyDamage
                        | sts2_core::CardNumberModifier::UnpoweredDamage),
                    ),
                    None,
                ) => {
                    let dealer = super::board::dealer_of::<V>(modifier, creatures);
                    // A card's `DamageVar` is a powered attack's unless its
                    // props say otherwise.
                    let powered = modifier != sts2_core::CardNumberModifier::UnpoweredDamage;
                    Some(super::board::card_hit::<V>(
                        seat, fight, dealer, None, held, var.id, powered,
                    ))
                }
                // `PowerVar<T>.UpdateCardPreview`: `Hook.ModifyPowerAmountGiven`
                // on `T`, from the card's owner.
                (Some(sts2_core::CardNumberModifier::PowerGiven), _) => {
                    let power = sts2_core::CardVars::power(var.id)
                        .unwrap_or_else(|| unreachable!("{:?} is a PowerVar of no power", var.id));
                    let giver = super::board::player::<V>(creatures);
                    let (given, _) = super::board::given_by::<V>(
                        seat,
                        fight,
                        giver,
                        target,
                        power,
                        printed(),
                        Some(held.id),
                    );
                    Some(given)
                }
                // `ExtraDamageVar.UpdateCardPreview`: the enchantment's
                // `EnchantDamageMultiplicative` over the number, for a
                // powered hit, and nothing else.
                (Some(sts2_core::CardNumberModifier::ExtraDamage), _) => {
                    super::board::enchant_scaled::<V>(held, printed())
                }
                (Some(modifier), _) => match super::board::dealer_of::<V>(modifier, creatures) {
                    Some(dealer) => super::board::effect_of::<V>(modifier, var.id, dealer, target)
                        .map(|effect| {
                            super::board::resolve::<V>(seat, fight, Some(held), effect).amount()
                        }),
                    // `OstyDamageVar` with `Owner.Osty` null: `Hook.ModifyDamage`
                    // with no dealer.
                    None => Some(super::board::card_hit::<V>(
                        seat, fight, None, target, held, var.id, true,
                    )),
                },
                (None, _) => None,
            };
            // A number no hook reaches reads what the card reads.
            (var.id, resolved.unwrap_or_else(printed))
        })
        .collect()
}

/// The numbers this copy of the card prints, which is each one's `BaseValue`.
///
/// This copy's own, not the table's. A fight writes on a card, and what it
/// wrote is what the card prints afterwards. Nothing in the fight has listened
/// yet: [`previewed`] is these after it has. A calculated number's is its
/// `CalculationBase`, and never what it works out to.
fn numbers<V: Rules>(held: &CardInstance<V>) -> sts2_core::Keyed<V::CardVar, V::Number> {
    held.face.numbers().map(|var| (var.id, var.base)).collect()
}

#[cfg(test)]
mod tests {
    /// The one seat of the solo fights these play.
    const SOLO: sts2_core::SeatIndex = sts2_core::SeatIndex(0);

    use super::super::powers::Powers as _;
    use super::*;
    use sts2_content::v0_107_1::{
        V0_107_1,
        cards::{Cards, ColorlessCard, RegentCard, StatusCard},
        characters::Character,
        monsters::Monster,
        runstate::{CombatSide, UnplayableReason, UnplayableReasons},
    };
    use sts2_core::ModelId;
    use sts2_core::fixture::at;

    type V = V0_107_1;

    fn card(card: Cards) -> CardInstance<V> {
        // One card per fixture fight, so one id is enough to tell it apart
        // from the nothing beside it.
        CardInstance::new(CardIdent::new(card), sts2_core::CardId::new(0))
    }

    fn live(creatures: Vec<Fighter<V>>) -> Live<V> {
        Live::of(creatures)
    }

    /// `fight` with `belt`, which a test leaves empty.
    fn walk<'a>(fight: &'a Live<V>, belt: &'a crate::run::Belts<V>) -> Walk<'a, V> {
        Walk {
            live: fight,
            belts: belt,
        }
    }

    /// A player and one living enemy, which is the smallest fight.
    fn fight() -> Vec<Fighter<V>> {
        vec![
            creature(
                at(0),
                ModelId::Character(Character::Regent),
                CombatSide::Player,
            ),
            creature(
                at(1),
                ModelId::Monster(Monster::TwigSlimeS),
                CombatSide::Enemy,
            ),
        ]
    }

    fn creature(id: CombatId, model: ModelId<V>, side: CombatSide) -> Fighter<V> {
        Fighter {
            id,
            model: Some(model),
            side,
            hp: 10,
            max_hp: 10,
            block: 0,
            alive: true,
            next_move: None,
            owner: None,
        }
    }

    fn only(
        hand: &[CardInstance<V>],
        energy: <V as sts2_core::ContentVersion>::Count,
        stars: <V as sts2_core::ContentVersion>::Count,
    ) -> Playable<V> {
        let mut fight = live(fight());
        fight.players[SOLO].energy = energy;
        fight.players[SOLO].stars = stars;
        let mut plays = super::hand::<V>(
            SOLO,
            hand.iter().copied(),
            walk(
                &fight,
                &crate::run::Belts::new(crate::run::Belt::new(), Vec::new()),
            ),
        );
        assert_eq!(plays.len(), 1);
        plays.remove(0)
    }

    /// A fight where `carrier` carries one power, at one amount.
    fn fight_with(
        carrier: CombatId,
        power: sts2_content::v0_107_1::powers::Power,
        amount: i32,
    ) -> Live<V> {
        let mut fight = live(fight());
        fight.powers.push(
            carrier,
            super::super::live::HeldPower::new(
                power,
                crate::combat::live::Stacked::counted(amount),
                None,
            ),
        );
        fight
    }

    /// `UpdateCardPreview` hands a null target to `Hook.ModifyDamage` rather
    /// than skipping the hook, so the dealer's own powers still move a hit
    /// nobody is pointed at.
    #[test]
    fn a_hit_previewed_against_nobody_still_reads_the_dealer_s_powers() {
        use sts2_content::v0_107_1::{cards::CardVar, powers::Power};

        let strike = card(Cards::Regent(RegentCard::Strike));
        let fight = fight_with(at(0), Power::StrengthPower, -2);
        let damage = |targets: &[CombatId]| {
            effect::<V>(
                SOLO,
                &strike,
                walk(
                    &fight,
                    &crate::run::Belts::new(crate::run::Belt::new(), Vec::new()),
                ),
                targets,
            )[0]
            .vars[&CardVar::Damage]
        };

        // The same card, previewed at the one enemy and at nobody. Strength
        // belongs to the player, so it lands on both.
        let printed = super::numbers::<V>(&strike)[&CardVar::Damage];
        assert_eq!(damage(&[at(1)]), printed - 2);
        assert_eq!(damage(&[]), printed - 2);
    }

    /// The other side of it. A power the target carries is the one that sits
    /// out a preview against nobody, because there is nobody to read it off.
    #[test]
    fn a_hit_previewed_against_nobody_skips_the_target_s_own_powers() {
        use sts2_content::v0_107_1::{cards::CardVar, powers::Power};

        let strike = card(Cards::Regent(RegentCard::Strike));
        let fight = fight_with(at(1), Power::VulnerablePower, 1);
        let damage = |targets: &[CombatId]| {
            effect::<V>(
                SOLO,
                &strike,
                walk(
                    &fight,
                    &crate::run::Belts::new(crate::run::Belt::new(), Vec::new()),
                ),
                targets,
            )[0]
            .vars[&CardVar::Damage]
        };

        let printed = super::numbers::<V>(&strike)[&CardVar::Damage];
        assert!(damage(&[at(1)]) > printed);
        assert_eq!(damage(&[]), printed);
    }

    #[test]
    fn each_empty_pool_names_the_pool_it_is_empty_from() {
        let strike = card(Cards::Regent(RegentCard::Strike));
        let falling_star = card(Cards::Regent(RegentCard::FallingStar));

        assert_eq!(only(std::slice::from_ref(&strike), 1, 0).reason, None);
        assert_eq!(
            only(std::slice::from_ref(&strike), 0, 0).reason,
            Some(UnplayableReasons::new(&[
                UnplayableReason::EnergyCostTooHigh
            ]))
        );
        // Falling Star costs no energy and two stars, so an empty energy pool
        // is not what stops it.
        assert_eq!(only(std::slice::from_ref(&falling_star), 0, 2).reason, None);
        assert_eq!(
            only(std::slice::from_ref(&falling_star), 0, 1).reason,
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

        let mut fight = live(creatures);
        fight.players[SOLO].energy = 3;
        fight.players[SOLO].stars = 0;
        let plays = super::hand::<V>(
            SOLO,
            [strike].into_iter(),
            walk(
                &fight,
                &crate::run::Belts::new(crate::run::Belt::new(), Vec::new()),
            ),
        );

        assert!(plays[0].targets.is_empty());
        // Nothing to hit is not a reason the card cannot be played. The game
        // refuses the play when it is pointed at nothing.
        assert_eq!(plays[0].reason, None);
    }
}
