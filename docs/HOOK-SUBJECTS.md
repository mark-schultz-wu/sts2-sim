# Whom a hook is about

In co-op, every player's listeners hear every hook. A game override
compares the hook's subject with its own owner, as in `card.Owner !=
base.Owner` or `player != base.Owner`. It acts on `base.Owner`, never on
the player whose command raised the hook. The simulator was written for
one player, so its hooks often name no subject, and a listener's effect
lands on the acting creature. The co-op audit in `BLOCKERS.md` lists the
sites. This design closes classes A, B and C of that audit in four
parts. Each part lands on its own, in the order below.

## 1. A listener acts for its owner

A listener's effect is carried out with the board's actor set to the
listener's own creature, and with no `reaching`:

- a relic: its seat's player
- a power: the creature carrying it, as `base.Owner` is
- a card: its owner's player
- a monster: the monster
- a modifier: the actor the walk was raised with, as now.

`Board::as_listener` sets this up around what the listener asked, and
`Board::listener_asked` carries an effect out through it. The board's
walks go through them, and `hook::apply` reads `fight.actor()` as the
listener's owner. A walk that builds a board for each effect, as the
turn's phases do, builds it for the listener's owner's seat. A card
writes on itself in its owner's piles, wherever the walk runs.

In a fight of one player, every player-owned listener's owner is the
actor already. A monster's power gets its own monster in place of the
acting creature, which is a fix.

## 2. A hook names its subject

The game passes some hooks a player, a card or a creature, which an
override compares with its owner. There, the simulator's hook carries
that subject as a `CombatId`. A card's subject is `Card.Owner.Creature`, in
a field named `player`, as `CardPlayed` has now. The content compares it
with its own owner, in the same line the game does.

- Fight hooks that gain the field:
  - `CardDrawn` and `CardDrawnEarly`
  - `CardExhausted`, `CardDiscarded` and `CardPlayedLate`
  - `Shuffled`, `Flushed` and `FlushingLate`
  - `ShouldPlay`, `XValue`, `StarCost` and `EnergySpent`
  - `StarsGained`, `OrbChanneled`, `OrbPassiveTriggerCount` and
    `Forged`
  - `PotionUsing` and `PotionUsed`
  - `ShouldTakeExtraTurn` and `TakingExtraTurn`
  - `AfterAttack`, which carries the attacker's pet owner.
- `CardSource` gains its card's owner, for the damage and block hooks.
- Run hooks that gain the field:
  - `GoldGained`, as the event and as the modify hook
  - `AddedToDeck`, `RestSiteHealRewards` and `ShouldProcurePotion`
  - `ShouldDisableRemainingRestSiteOptions`
  - `ShouldForcePotionReward`
  - `CardRewardCreationOptions` and `CardRewardAlternatives`.

A run hook asks every player's listeners when the game's `Hook` method
iterates every listener, and carries the player. It asks one seat's
listeners through `notify_run_for` only when the game iterates that
player's models alone.

Every relic, power, card, potion and enchantment rule gets its owner
where it does not yet. Each comment that says "a fight here has one
player" goes, either with the check that replaces it or because the
code is right.

## 3. Facts about the listener's own player

Some hook fields describe the listener's own player and not the
hook's subject. Examples are block, energy, an empty hand, a non-empty
belt, the deck size and the player's health. The run's walk refreshes
these for each listener's owner, in `State::refreshed`. The fight's walk
gains the same step: `Live::refreshed(owner, hook)`, asked as the walk
hands a listener its hook. It covers:

- `BeforeSideTurnEndVeryEarly { block }`
- `BeforeSideTurnEnd { energy, hand }`
- `SideTurnEnd { block }`
- `RoomEntered { deck, player }`
- `CombatStarting { holding_potions }`
- `PotionUsed { holding_potions }` and
  `PotionDiscarded { holding_potions }`
- `BeforeSideTurnEndEarly { plays }`, which counts its owner's plays.

A listener no player owns reads the hook as it was raised.

## 4. History names its player

Each `HistoryEntry` names the player it is about, as the game's entries
carry `Actor` or `CardPlay.Player`. The views a rule reads,
`History::view(player)`, filter by that player. So "this turn" counts
only the reader's own draws, attacks, exhausts and so on, as the game's
`e.Actor == base.Owner` filters do.

## Cost

Part 1 swaps the actor once for each listener that asks for something,
not for each listener asked. Parts 2 and 4 add a `CombatId` to some
hook variants and history entries. Part 3 rebuilds a hook only for an
owned listener of a hook that carries owner facts. Each PR reports its
cost with `cargo xtask instructions`. Special cases for one player wait
for the perf pass.
