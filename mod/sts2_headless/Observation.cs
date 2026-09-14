using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Potions;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Localization;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.MonsterMoves.Intents;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>
/// Everything the simulator is compared against after a decision (H4).
/// </summary>
/// <remarks>
/// Deliberately a superset of the game's own <c>NetFullCombatState</c>, which is
/// a divergence *fingerprint* rather than a state description: it carries piles
/// only while combat is in progress, drops the Rewards and Shops RNG streams
/// from what it compares, and omits monster intent entirely because two peers
/// can each re-derive it. None of those omissions are safe for us.
///
/// Nothing here may be defaulted when unavailable. A field the oracle cannot
/// fill is reported absent, so a simulator disagreement surfaces rather than
/// being papered over by a shared zero (H10).
/// </remarks>
public static class Observation
{
    private static readonly JsonSerializerOptions Json = new JsonSerializerOptions { WriteIndented = false };

    /// <summary>
    /// A digest of the full observation, for step-by-step comparison (H5).
    /// </summary>
    /// <remarks>
    /// The full observation runs ~1.6KB per decision — 27% creatures, 27% legal
    /// moves, 19% RNG counters, 11% deck, most of it repeated key names for
    /// values that rarely change. At 250 decisions/sec that is 400KB/s, which is
    /// wasteful when almost every step agrees.
    ///
    /// So the steady state compares digests, and the full observation is only
    /// needed for the step that disagrees. **This is affordable precisely
    /// because D1 holds**: a diverging run can be replayed with
    /// <c>STS2_HEADLESS_OBS=full</c> and will reproduce exactly, so nothing is lost
    /// by not having shipped the bytes the first time.
    ///
    /// Compacting the JSON was the other option and is the worse one: it would
    /// save perhaps 40% while making the failure path harder to read, and the
    /// failure path is the one that matters.
    /// </remarks>
    public static string Digest(RunState state) => DigestOf(Capture(state));

    /// <summary>
    /// The observation and its digest, built from a single capture.
    /// </summary>
    /// <remarks>
    /// Callers that want both must use this rather than calling
    /// <see cref="Capture"/> and <see cref="Digest"/> in turn: the latter
    /// captures again internally, so a full observation cost two serialisations
    /// of the same state and measurably halved protocol throughput.
    /// </remarks>
    public static (string Json, string Digest) Snapshot(RunState state)
    {
        string json = Capture(state);
        return (json, DigestOf(json));
    }

    private static string DigestOf(string capture)
    {
        byte[] hash = SHA256.HashData(Encoding.UTF8.GetBytes(capture));
        return Convert.ToHexString(hash, 0, 8);
    }

    public static string Capture(RunState state)
    {
        Player me = state.Players[0];
        CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
        PlayerCombatState? pcs = me.PlayerCombatState;

        var obs = new Dictionary<string, object?>
        {
            ["act"] = state.CurrentActIndex,
            ["floor"] = state.TotalFloor,
            ["hp"] = me.Creature.CurrentHp,
            ["maxHp"] = me.Creature.MaxHp,

            ["alive"] = me.Creature.IsAlive,

            // A win kills every player too — RunManager.WinRun calls
            // GuaranteeKillAllPlayers — so "the run is over" and "you lost" are
            // not the same question. IsVictoryRoom is what separates them, and
            // it is only meaningful once the run has ended.
            ["runOver"] = state.IsGameOver,
            ["victory"] = state.IsGameOver ? state.CurrentRoom?.IsVictoryRoom ?? false : null,
            ["gold"] = me.Gold,
            ["relics"] = me.Relics.Select(Carried).ToList(),
            ["potions"] = me.Potions.Select(p => Held(p, me, state)).ToList(),

            // The belt's size, not just what is in it. A full belt is what makes
            // a potion refuse to be bought or taken, and `potions` alone cannot
            // say whether three is full — relics change the count.
            ["potionSlots"] = me.PotionSlots.Count,
            ["deck"] = me.Deck.Cards.Select(Ident).ToList(),
            ["inCombat"] = CombatManager.Instance.IsInProgress,
            ["map"] = MapPosition(state),
            ["rng"] = Counters(state, me),
        };

        Dictionary<string, object?>? room = Room(state, me);
        if (room != null)
        {
            obs["room"] = room;
        }

        // Absent rather than empty: "not in combat" and "in combat with nothing
        // in hand" must not serialise the same way.
        if (combat != null && pcs != null)
        {
            obs["combat"] = new Dictionary<string, object?>
            {
                ["turn"] = pcs.TurnNumber,
                ["phase"] = pcs.Phase.ToString(),
                ["energy"] = pcs.Energy,
                ["stars"] = pcs.Stars,
                ["round"] = combat.RoundNumber,

                // Which fight this is, which nothing else in the observation
                // says: the roster is in `creatures`, but two encounters can
                // field the same monsters at different counts and one monster
                // appears in eleven of them. Absent for a combat with no
                // encounter behind it -- `CombatState` allows one, and a null
                // here is the difference between "some other fight" and "not a
                // listed fight at all".
                ["encounter"] = combat.Encounter?.Id.Entry,
                ["hand"] = pcs.Hand.Cards.Select(Ident).ToList(),
                ["draw"] = pcs.DrawPile.Cards.Select(Ident).ToList(),
                ["discard"] = pcs.DiscardPile.Cards.Select(Ident).ToList(),
                ["exhaust"] = pcs.ExhaustPile.Cards.Select(Ident).ToList(),
                ["creatures"] = combat.Creatures.Select(Describe).ToList(),
                ["legal"] = Legal(combat, pcs).ToList(),

                // Ordered, because an orb queue is a queue: the same orbs in a
                // different order evoke differently. Capacity is separate from
                // count, since empty slots are part of the state. Only Defect
                // uses these, which is why they went unwritten until `start`
                // could ask for a character.
                //
                // Absent when the subsystem is inert -- no capacity and nothing
                // channelled -- rather than reported as an empty queue. Exactly
                // one character overrides BaseOrbSlotCount, so for four runs in
                // five this field carries no information, and "absent" is the
                // observation's existing way of saying so. Keyed on capacity
                // rather than on the character, because a relic can grant slots
                // to someone who started with none.
                ["orbs"] = (pcs.OrbQueue.Capacity == 0 && pcs.OrbQueue.Orbs.Count == 0)
                    ? null
                    : new Dictionary<string, object?>
                    {
                        ["capacity"] = pcs.OrbQueue.Capacity,
                        ["queue"] = pcs.OrbQueue.Orbs.Select(o => new Dictionary<string, object?>
                        {
                            ["id"] = o.Id.Entry,
                            ["passive"] = o.PassiveVal,
                            ["evoke"] = o.EvokeVal,
                        }).ToList(),
                    },
            };
        }

        return JsonSerializer.Serialize(obs, Json);
    }

    /// <summary>Card identity: the id alone is not enough to compare two decks.</summary>
    /// <remarks>
    /// Everything a card carries that survives being put in a pile, because a
    /// deck holding an upgraded Strike is not a deck holding a plain one and the
    /// id is the same string either way. <c>CurrentUpgradeLevel</c>,
    /// <c>Affliction</c> and <c>Enchantment</c> are the three, and they are the
    /// same three <c>CombatState.IterateHookListeners</c> walks off a card.
    ///
    /// The separators are safe because an entry name cannot contain one:
    /// <c>StringHelper.Slugify</c> ends in a <c>[^A-Z0-9_]</c> strip, so an id
    /// is exactly those characters and nothing else.
    ///
    /// Packed into one string rather than an object per card, the way
    /// <see cref="Id(ModelId)"/> packs a model id. A deck is forty of these and
    /// four combat piles are more, so the key names would outweigh the values.
    /// </remarks>
    /// <summary>
    /// What a card costs in energy, in the three states the game has.
    /// </summary>
    /// <remarks>
    /// A number, the string <c>"x"</c>, or null. The game spells "not priced in
    /// this pool" as <c>-1</c>, and <c>GetWithModifiers</c> hands that straight
    /// back for a curse or a status. It also zeroes the canonical cost of an
    /// X-cost card, so on the wire an X card and a free card were the same
    /// integer. Both are resolved here, once, rather than by every reader.
    /// </remarks>
    public static object? EnergyCost(CardModel card)
    {
        if (card.EnergyCost.CostsX)
        {
            return "x";
        }

        int cost = card.EnergyCost.GetWithModifiers(CostModifiers.All);
        return cost < 0 ? null : cost;
    }

    /// <summary>
    /// What a card costs in stars, the same three ways.
    /// </summary>
    /// <remarks>
    /// <c>GetStarCostWithModifiers</c> rather than <c>CurrentStarCost</c>,
    /// because it is the star-side twin of the energy call above: it runs the
    /// combat hooks that change a card's cost where it is sitting. Outside a
    /// combat pile it falls through to <c>CurrentStarCost</c>, so a reward
    /// screen reads the same either way.
    ///
    /// Every card outside the Regent's has <c>CanonicalStarCost</c> of -1, so
    /// null here is the common case rather than the exception.
    /// </remarks>
    public static object? StarCost(CardModel card)
    {
        if (card.HasStarCostX)
        {
            return "x";
        }

        int cost = card.GetStarCostWithModifiers();
        return cost < 0 ? null : cost;
    }

    public static string Ident(CardModel card)
    {
        string s = card.Id.Entry;
        if (card.CurrentUpgradeLevel > 0)
        {
            s += "+" + card.CurrentUpgradeLevel;
        }
        if (card.Affliction != null)
        {
            s += "/" + card.Affliction.Id.Entry;
        }
        if (card.Enchantment != null)
        {
            s += "*" + card.Enchantment.Id.Entry;
        }
        return s;
    }

    private static string Ident(RelicModel relic) => relic.Id.Entry;

    /// <summary>The words a <c>LocString</c> stands for, or nothing.</summary>
    /// <remarks>
    /// <c>GetFormattedText</c> throws a <c>LocException</c> for a key the table
    /// does not have, and a missing key is ordinary: Neow's event has no page
    /// description, and an option built with a title and no description gets a
    /// <c>LocString</c> pointing at a key nobody wrote. <c>Exists</c> is the
    /// game's own way of asking, so this reports absence as absence rather than
    /// losing the whole observation to a key that was never going to be there.
    ///
    /// Not a try/catch: an exception thrown and swallowed on every observation
    /// would hide a table that genuinely failed to load behind one that simply
    /// has nothing to say here.
    /// </remarks>
    private static string? Words(LocString? loc) =>
        loc != null && loc.Exists() ? loc.GetFormattedText() : null;

    /// <summary>An event's prose, prepared the way the event room prepares it.</summary>
    /// <remarks>
    /// An event's text is not a fixed string. <em>Gain <c>{MaxHp}</c> Max HP</em>
    /// is formatted against the event's <c>DynamicVars</c>, exactly as a card's
    /// text is formatted against its own, and formatting it without them throws a
    /// <c>FormattingException</c> rather than returning the raw string — so this
    /// is not a nicety, it is what makes the field readable at all.
    ///
    /// The preparation mirrors <c>NEventRoom.SetDescription</c> and
    /// <c>NEventOptionButton._Ready</c>, which are the two places the game itself
    /// does it: the character's details, whether the run is multiplayer, and the
    /// event's vars. Mirrored rather than reduced to "add the vars", because a
    /// description referring to the character by name is written the same way and
    /// would fail the same way.
    ///
    /// The event's <em>title</em> is deliberately not prepared: <c>SetTitle</c>
    /// formats it bare, so a title that needed a variable would be a bug in the
    /// game rather than here.
    ///
    /// This mutates the <c>LocString</c> — <c>Add</c> writes to a dictionary on
    /// it — which is what the game's own screens do to the same objects. Nothing
    /// the digest covers is touched.
    /// </remarks>
    private static string? Words(EventModel ev, LocString? loc)
    {
        if (loc == null || !loc.Exists())
        {
            return null;
        }

        ev.Owner?.Character.AddDetailsTo(loc);
        if (ev.Owner != null)
        {
            loc.Add("IsMultiplayer", ev.Owner.RunState.Players.Count > 1);
        }
        ev.DynamicVars.AddTo(loc);
        return loc.GetFormattedText();
    }

    /// <summary>One relic, and the words the game shows for it.</summary>
    /// <remarks>
    /// <c>DynamicDescription</c> rather than the raw <c>Description</c>: a relic
    /// that counts prints its count, and formatting the string without the
    /// variables throws rather than returning the unformatted text. It is what
    /// <c>RelicModel.HoverTip</c> uses, so this is the same sentence a player
    /// reads by hovering the relic.
    ///
    /// The two <c>HoverTip</c> wrappers are deliberately not mirrored. A melted
    /// or used-up relic reads <em>"&lt;description&gt; (Melted)"</em> on screen,
    /// which is a fact the prose does not need glued to it twice.
    ///
    /// <c>DisplayAmount</c> is the number the relic itself counts: Bone Tea's
    /// combats left, Pocketwatch's cards played this turn. It is a rule and
    /// not a decoration, so it is reported whether or not <c>ShowCounter</c>
    /// puts it on screen. A relic that counts nothing reports zero.
    /// </remarks>
    private static Dictionary<string, object?> Carried(RelicModel relic) =>
        new()
        {
            ["id"] = relic.Id.Entry,
            ["amount"] = relic.DisplayAmount,
            ["title"] = Words(relic.Title),
            ["text"] = Words(relic.DynamicDescription),
        };

    /// <summary>One potion in the belt, and whether it can be drunk.</summary>
    /// <remarks>
    /// Deliberately the same shape as one card in <c>legal</c>: a reason that is
    /// absent exactly when it can be drunk, and the creatures it may be pointed
    /// at. A client applies no rule of its own — deciding this is the
    /// implementation's job, and two implementations that decide differently is
    /// a divergence rather than a screen bug.
    /// </remarks>
    private static Dictionary<string, object?> Held(PotionModel potion, Player me, RunState state)
    {
        string? reason = Undrinkable(potion, me);
        return new Dictionary<string, object?>
        {
            ["id"] = potion.Id.Entry,
            ["title"] = Words(potion.Title),
            ["text"] = Words(potion.DynamicDescription),
            ["usable"] = reason == null,
            ["reason"] = reason,
            ["targets"] = Targets(potion, state),
        };
    }

    /// <summary>Why a potion cannot be drunk right now, or null when it can.</summary>
    /// <remarks>
    /// The game has no enum for this: <c>NPotionPopup</c> decides by enabling
    /// and disabling a button, in <c>_Ready</c>'s switch over
    /// <c>PotionUsage</c> and then in <c>RefreshButtons</c>. This is that
    /// decision written down. The order is the order the game applies it, so
    /// the reason reported is the first one that holds.
    ///
    /// <c>internal</c> because <c>Protocol.UsePotion</c> asks the same question
    /// before it enqueues: a refusal that can be decided without awaiting is
    /// decided in front of the action, not inside it.
    /// </remarks>
    internal static string? Undrinkable(PotionModel potion, Player me)
    {
        if (me.Creature.IsDead)
        {
            return "Dead";
        }

        if (potion.IsQueued)
        {
            return "Queued";
        }

        if (!GameApi.CanUseOrRemovePotions(me))
        {
            return "Stuck";
        }

        switch (potion.Usage)
        {
            case PotionUsage.Automatic:
                return "Automatic";

            case PotionUsage.AnyTime:
                break;

            case PotionUsage.CombatOnly:
                if (!CombatManager.Instance.IsInProgress)
                {
                    return "OutOfCombat";
                }

                // The rest of `RefreshButtons`. `InACardSelectScreen` is a
                // screen the headless game never puts up, so the two that can
                // hold are whose turn it is and whether the game has actions
                // disabled while something resolves.
                if (me.Creature.CombatState?.CurrentSide != me.Creature.Side
                    || !me.Creature.IsAlive
                    || CombatManager.Instance.PlayerActionsDisabled)
                {
                    return "NotYourTurn";
                }

                break;

            // `PotionUsage.None` throws in the game's own switch, so it is not
            // a state any potion is in. Reported rather than thrown, because a
            // refusal is an answer and a crashed oracle is not.
            default:
                return "Automatic";
        }

        return potion.PassesCustomUsabilityCheck ? null : "ItsOwnCheck";
    }

    /// <summary>Every creature a potion must be chosen between.</summary>
    /// <remarks>
    /// <c>PotionModel.IsValidTarget</c>, asked of each creature in the fight —
    /// but only for the target types where a creature is genuinely chosen.
    ///
    /// That restriction is what makes this field mean the same thing it means
    /// beside a card, and it is the game's own distinction rather than ours.
    /// <c>CardModel.IsValidTarget</c> returns false for <em>any</em> non-null
    /// target unless the type is <c>AnyEnemy</c> or <c>AnyAlly</c>, so a Self
    /// card already reports no targets and is played with none.
    /// <c>PotionModel.IsValidTarget</c> is written the other way and answers
    /// true for <c>Self</c> and <c>AnyPlayer</c> as well — yet the game never
    /// asks: <c>NPotionPopup</c> labels its button <em>throw</em> only for
    /// <c>AnyEnemy</c>, <c>AllEnemies</c>, <c>TargetedNoCreature</c> and
    /// <c>CanThrowAtAlly</c>, and <em>drink</em> otherwise.
    ///
    /// So a self-targeted potion reports nothing here and is drunk with no
    /// target, which is exactly what the game's own screen sends:
    /// <c>EnqueueManualUse</c> fills the drinker in. Swift Potion is the case
    /// that shows why <c>CanThrowAtAlly</c> is asked rather than its target
    /// type read: it is <c>AnyPlayer</c>, and in a single-player run there is
    /// no other player to throw it at, so it is drunk.
    ///
    /// Empty outside combat too, where there are no combat ids to name.
    /// </remarks>
    private static List<uint> Targets(PotionModel potion, RunState state)
    {
        CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
        if (combat == null || !CombatManager.Instance.IsInProgress)
        {
            return new List<uint>();
        }

        // `CanThrowAtAlly` rather than `AnyPlayer`, because it is the game's own
        // test and it carries the part an enum cannot: an `AnyPlayer` potion is
        // a choice only when there is another player to throw it at. In single
        // player Swift Potion is drunk, and the button says so.
        if (potion.TargetType is not (TargetType.AnyEnemy or TargetType.AnyAlly)
            && !potion.CanThrowAtAlly())
        {
            return new List<uint>();
        }

        return combat.Creatures
            .Where(c => c.CombatId.HasValue && potion.IsValidTarget(c))
            .Select(c => c.CombatId!.Value)
            .ToList();
    }

    /// <summary>What a card says on its face, for a card nobody is holding.</summary>
    /// <remarks>
    /// The rules text with this card's own numbers in it — upgrades and
    /// enchantments included, since both are on the model — but with no combat
    /// hooks applied, because there is no combat to apply them from.
    /// <see cref="Effects"/> is the in-hand version and does run them.
    ///
    /// For a reward or a selection prompt that is the right answer anyway: the
    /// question is which card to add to a deck, and the numbers that matter are
    /// the ones it will have in every fight rather than the ones it would have
    /// in this one.
    /// </remarks>
    public static string Text(CardModel card) => card.GetDescriptionForPile(PileType.None);

    /// <summary>A model id whose category is not fixed by the field it sits in.</summary>
    /// <remarks>
    /// Both halves, because <c>Entry</c> alone is ambiguous: three entries exist
    /// in two categories at once — <c>LOST_WISP</c> (EVENT and RELIC),
    /// <c>BYRDPIP</c> and <c>PAELS_LEGION</c> (MONSTER and RELIC) — and the last
    /// two are reachable here, since monsters and relics are both combat hook
    /// listeners and either can be a card's preventer.
    ///
    /// Written as <c>CATEGORY/ENTRY</c> rather than a nested object: one string
    /// and one field, the way <see cref="Ident(CardModel)"/> already packs a
    /// card's identity, and cheap on a field that appears once per creature and
    /// once per playable card in every observation.
    ///
    /// Where a field's category *is* fixed — a power, a relic, an orb — the bare
    /// entry stays, because there is nothing to disambiguate.
    /// </remarks>
    public static string Id(ModelId id) => id.Category + "/" + id.Entry;

    private static Dictionary<string, object?> Describe(Creature c)
    {
        return new Dictionary<string, object?>
        {
            ["id"] = c.CombatId,
            // `Creature.ModelId` is the game's own rule for which of the two this
            // is, and it keys on `IsPlayer` rather than on `Side` — a pet fights
            // on the player's side while still being a monster, so the sibling
            // `side` field below does not determine the category. Guarded because
            // that property dereferences whichever of the two it picks.
            ["model"] = (c.Monster != null || c.Player != null) ? Id(c.ModelId) : null,
            ["side"] = c.Side.ToString(),
            ["hp"] = c.CurrentHp,
            ["maxHp"] = c.MaxHp,
            ["block"] = c.Block,
            ["alive"] = c.IsAlive,
            ["powers"] = c.Powers.Select(p => new Dictionary<string, object?>
            {
                ["id"] = p.Id.Entry,
                ["amount"] = p.Amount,
            }).ToList(),

            // The game never ships intent — NetFullCombatState omits it because
            // peers re-derive it, and MonsterModel.NextMove is serialised
            // nowhere. A simulator that picks the wrong monster move is exactly
            // the bug this catches, so it is reported here.
            ["intent"] = c.Monster?.NextMove.Id,
            ["intents"] = Intents(c),
        };
    }

    /// <summary>
    /// What the move is actually going to do, the way the game displays it.
    /// </summary>
    /// <remarks>
    /// A move carries a <em>list</em> of intents — <c>MoveState.Intents</c> —
    /// so an attack that also blocks reports two, in the order the game draws
    /// them. The move id alone says which move was chosen; this says what it
    /// amounts to, which is the part a simulator gets wrong quietly.
    ///
    /// Only attacks carry numbers, and that is the game's own shape rather than
    /// a gap here: <c>DefendIntent</c> has no block field at all, and neither do
    /// the other twelve <c>IntentType</c> values. The base game draws a shield
    /// with no number for the same reason.
    /// </remarks>
    private static List<Dictionary<string, object?>>? Intents(Creature c)
    {
        if (c.Monster == null)
        {
            return null;
        }

        var described = new List<Dictionary<string, object?>>();
        foreach (AbstractIntent intent in c.Monster.NextMove.Intents)
        {
            var d = new Dictionary<string, object?>
            {
                ["type"] = intent.IntentType.ToString(),
            };

            if (intent is AttackIntent attack)
            {
                // Targets are the monster's targets, which Creature.HoverTips
                // resolves the same way: CombatState.Allies, owner being the
                // monster itself.
                //
                // GetSingleDamage runs Hook.ModifyDamage, so this is the number
                // that will actually land rather than the move's base value.
                // That path goes through LocalContext.GetMe, which returns null
                // when NetId is unset — and a null there would silently skip the
                // hook and report damage ignoring Strength and Vulnerable.
                // Checked: NetId *is* set in a headless serve run, so the hook
                // does apply. If that ever changes this reports plausible wrong
                // numbers rather than failing, which is why it is written down.
                // A monster taking a turn always has a CombatState, so this is
                // guarded rather than assumed: an NRE here would lose the whole
                // observation, and the absence of `damage` on an Attack is a
                // louder signal than a crash mid-serialisation would be.
                if (c.CombatState != null)
                {
                    d["damage"] = attack.GetSingleDamage(c.CombatState.Allies, c);
                    d["repeats"] = attack.Repeats;
                }
            }

            described.Add(d);
        }

        return described;
    }

    /// <summary>
    /// The legal command set, with the reason each illegal card is illegal.
    /// </summary>
    /// <remarks>
    /// Comparing reasons rather than booleans localises a simulator bug to the
    /// rule it got wrong. <c>UnplayableReason</c> is a [Flags] enum covering the
    /// unplayable keyword, energy and star cost, no-living-allies, hook blocks
    /// and card logic.
    /// </remarks>
    private static IEnumerable<Dictionary<string, object?>> Legal(CombatState combat, PlayerCombatState pcs)
    {
        List<Creature> alive = combat.Creatures.Where(c => c.IsAlive).ToList();

        foreach (CardModel card in pcs.Hand.Cards)
        {
            bool canPlay = card.CanPlay(out UnplayableReason reason, out AbstractModel? preventer);
            yield return new Dictionary<string, object?>
            {
                ["card"] = Ident(card),
                ["cost"] = EnergyCost(card),
                ["starCost"] = StarCost(card),
                ["target"] = card.TargetType.ToString(),
                ["playable"] = canPlay,
                ["reason"] = reason == UnplayableReason.None ? null : reason.ToString(),
                // Any combat hook listener can be the one that said no: a power,
                // the monster itself, a relic, a potion, an orb, a card, an
                // affliction, an enchantment, a run modifier, a badge. So the
                // category travels with it.
                ["preventer"] = preventer is null ? null : Id(preventer.Id),
                ["targets"] = alive.Where(card.CanPlayTargeting).Select(c => c.CombatId).ToList(),
                ["effect"] = Effects(card, alive),
            };
        }
    }

    /// <summary>What the card will actually do, with everything applied.</summary>
    /// <remarks>
    /// A card's numbers are not static data. Strike's 6 becomes 9 with Strength,
    /// 13 against a Vulnerable target, less again under Shrinker — and none of
    /// that is anywhere in the generated tables, because none of it is a
    /// property of the card. It is the answer <c>Hook.ModifyDamage</c> and
    /// <c>Hook.ModifyBlock</c> give for this card, this owner, this target, right
    /// now.
    ///
    /// The game computes it through <c>CardModel.UpdateDynamicVarPreview</c>,
    /// which is what fills in the number the player reads off the card face, and
    /// that is what runs here — the same call the UI makes, not a
    /// reimplementation of it. So the numbers reported are the numbers the game
    /// would print, and a simulator disagreeing with them is a real disagreement
    /// rather than an artifact of this file.
    ///
    /// One entry per distinct outcome. A targeted attack is previewed against
    /// each creature it may legally hit, because that is exactly where the
    /// numbers differ; identical entries collapse to a single one with no
    /// target, so an untargeted card and a card whose damage happens not to vary
    /// both report once.
    ///
    /// Running the hooks costs nothing, in the sense that matters: the
    /// observation is captured after the game is quiescent, <c>previewMode</c>
    /// tells every listener this is a preview, and <c>ClearPreview</c> puts the
    /// vars back. <see cref="Intents"/> has run <c>Hook.ModifyDamage</c> from
    /// this same file since it was written, and digests are stable across it.
    /// </remarks>
    private static List<Dictionary<string, object?>> Effects(CardModel card, List<Creature> alive)
    {
        // Only a card that names a single creature can vary by target. AllEnemies
        // and Self resolve the same way whoever is hovered, and previewing them
        // once per enemy would report the same numbers three times.
        List<Creature?> targets = card.TargetType is TargetType.AnyEnemy or TargetType.AnyAlly or TargetType.AnyPlayer
            ? alive.Where(card.CanPlayTargeting).Select(c => (Creature?)c).ToList()
            : new List<Creature?>();

        // A targeted card with nothing legal to hit still has numbers, and a
        // client that cannot see them cannot tell why it should not play it.
        if (targets.Count == 0)
        {
            targets.Add(null);
        }

        var previews = new List<Dictionary<string, object?>>();
        try
        {
            foreach (Creature? target in targets)
            {
                card.UpdateDynamicVarPreview(CardPreviewMode.Normal, target, card.DynamicVars);
                previews.Add(new Dictionary<string, object?>
                {
                    ["target"] = target?.CombatId,
                    ["text"] = card.GetDescriptionForPile(PileType.Hand, target),
                    ["vars"] = card.DynamicVars.ToDictionary(
                        v => v.Key, v => (object?)(int)v.Value.PreviewValue),

                    // What the card prints, beside what it will do. The two
                    // differ exactly when something is modifying the card, which
                    // is the thing a player wants to be told — and the only
                    // reliable way to say it. The game marks a moved number by
                    // wrapping it in [green] or [red] inside `text`, but those
                    // tags are also just colours: an event's "Lose [red]12[/red]
                    // Max HP" is red because losing Max HP is bad, not because
                    // 12 is not 12.
                    //
                    // BaseValue is the printed value including upgrades and
                    // enchantment, since both write to it. So this is "the card
                    // as it sits in the deck" against "the card as it would be
                    // played right now", which is the comparison worth making.
                    ["base"] = card.DynamicVars.ToDictionary(
                        v => v.Key, v => (object?)(int)v.Value.BaseValue),
                });
            }
        }
        finally
        {
            // Not optional. The preview writes over PreviewValue and, for an
            // enchanted card, EnchantedValue; leaving one card's hover state
            // behind would make the next observation depend on this one.
            card.DynamicVars.ClearPreview();
        }

        if (previews.Count > 1 && previews.Skip(1).All(p => Same(previews[0], p)))
        {
            previews[0]["target"] = null;
            previews.RemoveRange(1, previews.Count - 1);
        }

        return previews;
    }

    /// <summary>Two previews that differ only in who they were computed against.</summary>
    private static bool Same(Dictionary<string, object?> a, Dictionary<string, object?> b)
    {
        // `base` cannot differ between two previews of one card, so `vars` and
        // the text are the whole comparison.
        return (string?)a["text"] == (string?)b["text"]
            && ((Dictionary<string, object?>)a["vars"]!).OrderBy(kv => kv.Key)
                .SequenceEqual(((Dictionary<string, object?>)b["vars"]!).OrderBy(kv => kv.Key));
    }

    /// <summary>
    /// The choices the current room offers, if it is one that offers any.
    /// </summary>
    /// <remarks>
    /// Rest sites, events and shops are **not** pending decisions: their
    /// triggers are model-owned (`EnterInternal` calls `BeginRestSite`,
    /// `BeginEvent`, or populates `Inventories`), so after travelling the game
    /// is quiescent and idle exactly as at the start of a combat turn. They are
    /// state to be observed and answered with an ordinary command, which is why
    /// they need none of `Pending`'s machinery.
    ///
    /// Legality is taken from the model in every case — `RestSiteOption.IsEnabled`,
    /// `EventOption.IsLocked`, `MerchantEntry.EnoughGold`/`IsStocked` — rather
    /// than recomputed here. Unlike map travel, these rules do not live in a
    /// screen.
    ///
    /// Absent rather than empty when the room offers nothing, so "a shop with no
    /// stock" and "not in a shop" cannot serialise the same way (H10).
    /// </remarks>
    private static Dictionary<string, object?>? Room(RunState state, Player me)
    {
        switch (state.CurrentRoom)
        {
            case RestSiteRoom:
            {
                IReadOnlyList<RestSiteOption> options =
                    RunManager.Instance.RestSiteSynchronizer.GetLocalOptions();
                return new Dictionary<string, object?>
                {
                    ["type"] = "RestSite",
                    ["options"] = options.Select((o, i) => new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["id"] = o.OptionId,
                        ["enabled"] = o.IsEnabled,
                    }).ToList(),
                };
            }

            case TreasureRoom:
            {
                // The chest's relics, as options. Entering the room calls
                // `BeginRelicPicking`, which fills `CurrentRelics` and waits for
                // a vote — so unlike every other room here, a treasure room that
                // is never answered leaves the run holding an open session, and
                // the *next* chest throws.
                //
                // Skipping is one of the options rather than a separate command
                // because the screen offers it as a button beside the relics, and
                // because `OnRoomExited` only clears the session when it was
                // skipped: leaving without choosing is not a way out.
                IReadOnlyList<RelicModel>? relics =
                    RunManager.Instance.TreasureRoomRelicSynchronizer.CurrentRelics;
                bool answered = !Chest.Open(me);

                List<Dictionary<string, object?>> options = (relics ?? new List<RelicModel>())
                    .Select((r, i) => new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["id"] = r.Id.Entry,
                        ["relic"] = r.Id.Entry,
                        ["chosen"] = answered,
                    })
                    .ToList();

                if (relics != null)
                {
                    options.Add(new Dictionary<string, object?>
                    {
                        ["index"] = options.Count,
                        ["id"] = "SKIP",
                        ["skip"] = true,
                        ["chosen"] = answered,
                    });
                }

                return new Dictionary<string, object?>
                {
                    ["type"] = "Treasure",
                    ["finished"] = answered,
                    ["options"] = options,
                };
            }

            case EventRoom eventRoom:
            {
                EventModel ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
                return new Dictionary<string, object?>
                {
                    ["type"] = "Event",
                    ["id"] = ev.Id.Entry,
                    ["finished"] = ev.IsFinished,

                    // The words, beside the keys. An event is the one room whose
                    // whole content is prose: `BYRDONIS_NEST.pages.INITIAL.options.EAT`
                    // names an option without saying anything about it, and a
                    // client choosing between three of those is guessing.
                    //
                    // Localisation resolves headless — the tables load with the
                    // rest of the game and do not need a scene — so this is the
                    // game's own text rather than a second copy of it kept in
                    // sync by hand.
                    ["title"] = Words(ev.Title),
                    ["text"] = Words(ev, ev.Description),
                    ["options"] = ev.CurrentOptions.Select((o, i) => new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["key"] = o.TextKey,
                        ["title"] = Words(ev, o.Title),
                        ["text"] = Words(ev, o.Description),
                        ["locked"] = o.IsLocked,
                        ["proceed"] = o.IsProceed,
                        ["chosen"] = o.WasChosen,
                        ["relic"] = o.Relic?.Id.Entry,
                    }).ToList(),
                };
            }

            case MerchantRoom shop:
            {
                MerchantInventory inventory = shop.GetLocalInventory();
                return new Dictionary<string, object?>
                {
                    ["type"] = "Shop",
                    ["entries"] = inventory.AllEntries.Select((e, i) => new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["kind"] = e.GetType().Name,
                        ["id"] = EntryId(e),
                        ["cost"] = e.Cost,
                        ["stocked"] = e.IsStocked,
                        ["affordable"] = e.EnoughGold,
                        ["blocked"] = Blocked(e, me),
                    }).ToList(),
                };
            }

            default:
                return null;
        }
    }

    /// <summary>Why this entry cannot be bought, beyond stock and gold.</summary>
    /// <remarks>
    /// <c>IsStocked</c> and <c>EnoughGold</c> are the two checks
    /// <c>MerchantEntry.OnTryPurchaseWrapper</c> makes before it commits, and
    /// for most entries they are the whole story. A potion has a third: it is
    /// procured into a slot, and <c>PotionCmd.TryToProcure</c> fails with
    /// <c>TooFull</c> when there is none — so a shop can offer a potion that is
    /// stocked, affordable, and refused. That was found by playing at random,
    /// not by reading, which is the point of playing at random.
    ///
    /// The other way <c>TryToProcure</c> fails is <c>Hook.ShouldProcurePotion</c>
    /// saying no, which cannot be answered without running the hook and so is not
    /// reported. This says what is knowable, and a caller that trusts it further
    /// than that will be corrected by the game.
    ///
    /// <c>internal</c> because <c>Protocol.Buy</c> asks the same question before
    /// it commits: the purchase itself cannot be awaited, so every refusal that
    /// can be decided without awaiting has to be decided in front of it, and
    /// this is one of them.
    /// </remarks>
    internal static string? Blocked(MerchantEntry entry, Player me) => entry switch
    {
        MerchantPotionEntry when !me.HasOpenPotionSlots => "no potion slot",
        _ => null,
    };

    private static string? EntryId(MerchantEntry entry) => entry switch
    {
        MerchantCardEntry card => card.CreationResult?.Card is { } c ? Ident(c) : null,
        MerchantRelicEntry relic => relic.Model?.Id.Entry,
        MerchantPotionEntry potion => potion.Model?.Id.Entry,
        _ => null,
    };

    /// <summary>
    /// Where we are on the map, and which nodes may be travelled to next.
    /// </summary>
    /// <remarks>
    /// Legality is delegated to <see cref="MapLegality"/> rather than derived
    /// here. Two rules hide in it that a graph walk misses — free travel, and
    /// the boss sitting outside the grid — and both are invisible until the run
    /// reaches the state that triggers them. See that file for the details.
    /// </remarks>
    private static Dictionary<string, object?> MapPosition(RunState state)
    {
        MapPoint? current = state.CurrentMapPoint;
        return new Dictionary<string, object?>
        {
            ["at"] = current == null ? null : Coord(current),
            ["visited"] = state.VisitedMapCoords.Select(c => $"{c.col},{c.row}").ToList(),
            ["travelable"] = MapLegality.TravelableFrom(state)
                .Select(p => new Dictionary<string, object?>
                {
                    ["at"] = Coord(p),
                    ["type"] = p.PointType.ToString(),
                }).ToList(),
            ["points"] = Points(state.Map),

            // The one move the graph cannot express. An act's boss has no
            // children, so `travelable` is correctly empty once it is beaten and
            // the run has nowhere to go — `proceed` is how it leaves.
            ["proceed"] = MapLegality.CanProceed(state)
                && !CombatManager.Instance.IsInProgress,
        };
    }

    /// <summary>Every node in the act, and which nodes it leads on to.</summary>
    /// <remarks>
    /// The shape of the act, which does not change as it is played — not the
    /// legal moves, which do. <c>travelable</c> is the other half: it depends on
    /// where the run is standing and on free travel, and neither field can be
    /// derived from the other. Anything drawing the map wants both.
    ///
    /// Edges are <c>Children</c> verbatim, which is what
    /// <c>NMapScreen.DrawPaths</c> draws the lines from — so a picture built
    /// from this is the picture the game would show. Nothing is inferred here;
    /// in particular the edge from the last grid row to the boss is already in
    /// <c>Children</c>, which is the fallback <see cref="MapLegality"/> relies
    /// on.
    ///
    /// The starting node and both bosses sit outside <c>Grid</c>, so
    /// <c>GetAllMapPoints</c> never yields them and they are added by hand.
    /// </remarks>
    private static List<Dictionary<string, object?>> Points(ActMap map)
    {
        IEnumerable<MapPoint> outside = map.SecondBossMapPoint == null
            ? new[] { map.StartingMapPoint, map.BossMapPoint }
            : new[] { map.StartingMapPoint, map.BossMapPoint, map.SecondBossMapPoint };

        return map.GetAllMapPoints()
            .Concat(outside)
            .OrderBy(p => p.coord.row)
            .ThenBy(p => p.coord.col)
            .Select(p => new Dictionary<string, object?>
            {
                ["at"] = Coord(p),
                ["type"] = p.PointType.ToString(),
                ["to"] = p.Children
                    .OrderBy(c => c.coord.row)
                    .ThenBy(c => c.coord.col)
                    .Select(Coord)
                    .ToList(),
            })
            .ToList();
    }

    private static string Coord(MapPoint p) => $"{p.coord.col},{p.coord.row}";

    /// <summary>All fifteen RNG streams: twelve run-level, three per-player.</summary>
    /// <remarks>
    /// Two implementations can agree on every visible field while having
    /// consumed different numbers of draws. That divergence is invisible in the
    /// state and fatal three decisions later, so counters are compared directly.
    /// </remarks>
    private static Dictionary<string, int> Counters(RunState state, Player me)
    {
        var counters = new Dictionary<string, int>();
        foreach (KeyValuePair<RunRngType, int> c in GameApi.RunRngCounters(state))
        {
            counters[c.Key.ToString()] = c.Value;
        }
        foreach (KeyValuePair<PlayerRngType, int> c in GameApi.PlayerRngCounters(me))
        {
            counters["player." + c.Key] = c.Value;
        }
        return counters;
    }
}
