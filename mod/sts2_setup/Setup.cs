using HarmonyLib;
using MegaCrit.Sts2.Core.Assets;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Debug;
using MegaCrit.Sts2.Core.DevConsole.ConsoleCommands;
using MegaCrit.Sts2.Core.DevConsole;
using MegaCrit.Sts2.Core.Entities.Ascension;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.Multiplayer;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Entities.TreasureRelicPicking;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Modding;
using MegaCrit.Sts2.Core.Models.Characters;
using MegaCrit.Sts2.Core.Models.Encounters;
using MegaCrit.Sts2.Core.Models.Events;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;
using MegaCrit.Sts2.Core.Multiplayer;
using MegaCrit.Sts2.Core.Nodes;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Unlocks;
using Sts2Determinism;
using Sts2Headless;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text.Json;
using System.Threading.Tasks;
using System;

namespace Sts2Setup;

/// <summary>
/// Puts a run into a position without playing to it, as the game's developer
/// console does: into a fight or an event, with a power, an enchantment, a
/// number of hit points, or something granted. Every command here is one a
/// player cannot give, so a run that needs none of them loads without this.
/// </summary>
[ModInitializer(nameof(Init))]
public static class Setup
{
    public static void Init()
    {
        Protocol.Register("fight", Fight);
        Protocol.Register("event", Event);
        Protocol.Register("power", Power);
        Protocol.Register("enchant", Enchant);
        Protocol.Register("hp", Hp);
        Protocol.Register("grant", Grant);
        Protocol.Register("enter_room", EnterRoom);
    }

    /// <summary>
    /// Puts a named card or relic into the run (H6).
    /// </summary>
    /// <remarks>
    /// Through the game's own commands — <c>CardPileCmd.Add</c> and
    /// <c>RelicCmd.Obtain</c> — never by pushing onto a collection. The load
    /// path skips <c>AfterObtained</c>, so a poked-in relic produces a state the
    /// game never produces, and the divergence surfaces later as a simulator
    /// bug that is not one.
    /// </remarks>
    /// <summary>
    /// Jumps into a named encounter (H6).
    /// </summary>
    /// <remarks>
    /// The encounter is a model entry name, refused by the game rather than
    /// validated here: this mod has no list to check against, and the console
    /// command already answers "Encounter 'X' not found" better than a guess
    /// would.
    /// </remarks>
    internal static async Task Fight(JsonElement req)
    {
        string? encounter = Protocol.Str(req, "encounter");
        if (encounter == null)
        {
            await Protocol.Error("fight", Refusals.Malformed("fight requires an 'encounter' entry name"));
            return;
        }

        string? refused = await Cheats.EnterFight(Protocol.State, encounter);
        if (refused != null)
        {
            await Protocol.Error("fight", Refusals.Console(refused));
            return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Protocol.Observe(req, "fight", resolved);
    }

    /// <summary>Jumps the run to a named event, off the map.</summary>
    /// <remarks>
    /// The same shape as <see cref="Fight"/>, and refused by the game for the
    /// same reason: <c>EventConsoleCmd</c> answers "Event 'X' not found" over a
    /// list this mod does not hold.
    ///
    /// The run lands at an appended map point, so map and room disagree by
    /// construction -- as they do after a <c>fight</c>.
    /// </remarks>
    internal static async Task Event(JsonElement req)
    {
        string? entry = Protocol.Str(req, "event");
        if (entry == null)
        {
            await Protocol.Error("event", Refusals.Malformed("event requires an 'event' entry name"));
            return;
        }

        string? refused = await Cheats.EnterEvent(Protocol.State, entry);
        if (refused != null)
        {
            await Protocol.Error("event", Refusals.Console(refused));
            return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Protocol.Observe(req, "event", resolved);
    }

    /// <summary>Enchants the card at a hand index in the fight.</summary>
    /// <remarks>
    /// Scenario construction, like <see cref="Power"/>. Refused by the console
    /// command, which knows the enchantments and which cards each takes.
    /// </remarks>
    internal static async Task Enchant(JsonElement req)
    {
        string? enchantment = Protocol.Str(req, "enchantment");
        if (enchantment == null)
        {
            await Protocol.Error("enchant", Refusals.Malformed("enchant requires an 'enchantment' entry name"));
            return;
        }

        if (!req.TryGetProperty("amount", out JsonElement amountEl)
            || !amountEl.TryGetInt32(out int amount))
        {
            await Protocol.Error("enchant", Refusals.Malformed("enchant requires an integer 'amount'"));
            return;
        }

        if (!req.TryGetProperty("index", out JsonElement indexEl)
            || !indexEl.TryGetInt32(out int index))
        {
            await Protocol.Error("enchant", Refusals.Malformed("enchant requires a hand 'index'"));
            return;
        }

        string? refused = Cheats.Enchant(Protocol.State, enchantment, amount, index);
        if (refused != null)
        {
            await Protocol.Error("enchant", Refusals.Console(refused));
            return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Protocol.Observe(req, "enchant", resolved);
    }

    /// <summary>Puts a power on a creature in the fight.</summary>
    /// <remarks>
    /// Scenario construction, like <see cref="Fight"/>. A position with two
    /// debuffs already on the player is an hour of play away, and four lines
    /// here.
    ///
    /// The amount is the game's own overloaded number: a count for most
    /// powers, and below zero a single endless instance for the one power that
    /// decides its stacking at the time. The client spells what it means; this
    /// passes the number through.
    ///
    /// Refused by the console command rather than validated here, for the
    /// reason <see cref="Fight"/> gives: it knows the power list and this does
    /// not.
    /// </remarks>
    internal static async Task Power(JsonElement req)
    {
        string? power = Protocol.Str(req, "power");
        if (power == null)
        {
            await Protocol.Error("power", Refusals.Malformed("power requires a 'power' entry name"));
            return;
        }

        if (!req.TryGetProperty("amount", out JsonElement amountEl)
            || !amountEl.TryGetInt32(out int amount))
        {
            await Protocol.Error("power", Refusals.Malformed("power requires an integer 'amount'"));
            return;
        }

        if (!req.TryGetProperty("target", out JsonElement targetEl)
            || !targetEl.TryGetUInt32(out uint target))
        {
            await Protocol.Error("power", Refusals.Malformed("power requires a creature id in 'target'"));
            return;
        }

        string? refused = await Cheats.ApplyPower(Protocol.State, power, amount, target);
        if (refused != null)
        {
            await Protocol.Error("power", Refusals.Console(refused));
            return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Protocol.Observe(req, "power", resolved);
    }

    /// <summary>Sets the hit points of the player, or of a creature in the fight.</summary>
    /// <remarks>
    /// Scenario construction, like <see cref="Power"/>. A position at three hit
    /// points is a fight's worth of damage away, and every hit on the way
    /// wakes the relics that count them. This sets them through
    /// <c>CreatureCmd.SetCurrentHp</c>, which raises
    /// <c>AfterCurrentHpChanged</c> and nothing else, in a fight or out.
    ///
    /// Without a <c>target</c> it is the player, which is the only creature
    /// outside a fight.
    /// </remarks>
    internal static async Task Hp(JsonElement req)
    {
        if (!req.TryGetProperty("hp", out JsonElement hpEl)
            || !hpEl.TryGetInt32(out int hp)
            || hp < 1)
        {
            await Protocol.Error("hp", Refusals.Malformed("hp requires a positive integer 'hp'"));
            return;
        }

        uint? target = null;
        if (req.TryGetProperty("target", out JsonElement targetEl))
        {
            if (!targetEl.TryGetUInt32(out uint id))
            {
                await Protocol.Error("hp", Refusals.Malformed("hp takes a creature id in 'target'"));
                return;
            }
            target = id;
        }

        string? refused = await Cheats.SetHp(Protocol.State, hp, target);
        if (refused != null)
        {
            await Protocol.Error("hp", Refusals.Console(refused));
            return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Protocol.Observe(req, "hp", resolved);
    }

    internal static async Task Grant(JsonElement req)
    {
        Player me = Seat.Me(Protocol.State);
        string? card = Protocol.Str(req, "card");
        string? relic = Protocol.Str(req, "relic");
        string? potion = Protocol.Str(req, "potion");

        if (card == null && relic == null && potion == null)
        {
            await Protocol.Error("grant", Refusals.Malformed("grant requires 'card', 'relic' or 'potion'"));
            return;
        }

        // Tracked, never awaited. Obtaining a relic runs its AfterObtained, and
        // an AfterObtained may park a question: Royal Stamp opens a one-card
        // selection to enchant. The serve loop cannot read the answer until this
        // handler returns, so awaiting here waits for something only this
        // handler's return can produce -- the same deadlock `choose_option` on
        // SMITH had, and it hung for the same reason.
        //
        // Measured before the fix: `grant` with ROYAL_STAMP never replied and
        // the client timed out. Granting a card was unaffected, which is why
        // this looked fine.
        //
        // WaitForDecisionPoint handles both halves. For a relic nothing hangs
        // off it waits on BackgroundWork.InFlight until the obtain lands; for
        // Royal Stamp it returns as soon as the selection parks.
        // Both looked up before either is granted, so a request naming a good
        // card and a bad relic grants neither. Half a grant is a state no
        // client asked for and none can undo.
        CardModel? grantCard = null;
        if (card != null && (grantCard = Lookup<CardModel>(card, out string missing)) == null)
        {
            await Protocol.Error("grant", Refusals.Malformed(missing));
            return;
        }

        RelicModel? grantRelic = null;
        if (relic != null && (grantRelic = Lookup<RelicModel>(relic, out string absent)) == null)
        {
            await Protocol.Error("grant", Refusals.Malformed(absent));
            return;
        }

        PotionModel? grantPotion = null;
        if (potion != null && (grantPotion = Lookup<PotionModel>(potion, out string nothing)) == null)
        {
            await Protocol.Error("grant", Refusals.Malformed(nothing));
            return;
        }

        if (grantCard != null)
        {
            BackgroundWork.Track(Cheats.GrantCard(Protocol.State, grantCard));
        }

        if (grantRelic != null)
        {
            BackgroundWork.Track(RelicCmd.Obtain(grantRelic.ToMutable(), me));
        }

        // The same procure the game runs when a potion is picked up, so a full
        // belt refuses this exactly as it refuses a reward — reported through
        // the observation, where `potions` will simply not have grown.
        if (grantPotion != null)
        {
            BackgroundWork.Track(PotionCmd.TryToProcure(grantPotion.ToMutable(), me));
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Protocol.Observe(req, "grant", resolved);
    }

    /// <summary>Finds a canonical model by its id entry, or says what exists.</summary>
    /// <remarks>
    /// Null and a message rather than an exception. Naming something that does
    /// not exist is a client bug and the client has to be told so, but it is a
    /// <em>refusal</em> -- the request was not a request -- and throwing filed
    /// it as a fault, which is the mod admitting it fell over. Granting nothing
    /// silently would be worse than either: it hides the bug behind a
    /// divergence three steps later.
    /// </remarks>
    private static T? Lookup<T>(string id, out string missing) where T : AbstractModel
    {
        List<T> all = ModelDb.AllAbstractModelSubtypes
            .Where(t => t.IsSubclassOf(typeof(T)))
            .Select(t => ModelDb.GetById<T>(ModelDb.GetId(t)))
            .ToList();

        T? found = all.FirstOrDefault(x => string.Equals(x.Id.Entry, id, StringComparison.OrdinalIgnoreCase));
        missing = found == null ? $"no {typeof(T).Name} with id '{id}' ({all.Count} known)" : "";
        return found;
    }

    /// <summary>
    /// Enters a room of a given type directly, without walking the map to one.
    /// </summary>
    /// <remarks>
    /// Scenario construction (H6): reaching a shop by travelling means finding a
    /// seed that puts one on floor 2, which is the seed-hunting this project
    /// exists to avoid.
    ///
    /// <c>EnterRoomDebug</c> directly rather than through <c>RoomConsoleCmd</c>.
    /// The stated reason used to be that the console path reseeds from the wall
    /// clock, and that was never true of this one: <c>RoomConsoleCmd</c> calls
    /// the same <c>EnterRoomDebug</c> and never touched
    /// <c>DebugRandomizeRng</c> — that was <c>FightConsoleCmd</c>, which
    /// <c>fight</c> now uses. Adopting it would buy the game's own argument
    /// parsing and refusal message; it simply has not been done.
    /// </remarks>
    internal static async Task EnterRoom(JsonElement req)
    {
        // Unassigned is a RoomType the enum carries and nothing can usefully be
        // jumped to: it is the "no map point" placeholder `fight` lands on. The
        // game's own `room` command filters it out of its completions for the
        // same reason. Listing it as an option and then accepting it would be
        // offering a state with no meaning.
        string[] jumpable = Enum.GetNames(typeof(RoomType))
            .Where(n => n != nameof(RoomType.Unassigned))
            .ToArray();

        string? want = Protocol.Str(req, "type");
        if (want == null
            || !Enum.TryParse(want, ignoreCase: true, out RoomType type)
            || !jumpable.Contains(type.ToString()))
        {
            await Protocol.Error("enter_room", Refusals.Malformed(
                $"enter_room requires a 'type' from: {string.Join(" ", jumpable)}"));
            return;
        }

        // The game's `room` command guards this and ours did not, so entering a
        // room before `start` reached EnterRoomDebug with no run behind it.
        if (!RunManager.Instance.IsInProgress)
        {
            await Protocol.Error("enter_room", Refusals.NoRun());
            return;
        }

        await RunManager.Instance.EnterRoomDebug(type);
        bool resolved = await Driver.WaitForDecisionPoint();
        await Protocol.Observe(req, "enter_room", resolved);
    }
}

/// <summary>The game calls each setup command makes.</summary>
public static class Cheats
{
    /// <summary>
    /// Enters one fight and plays it out, greedily, to measure a decision.
    /// </summary>
    /// <remarks>
    /// Uses <c>EnterRoomDebug</c> rather than the <c>fight</c> console command:
    /// that command calls <c>EncounterModel.DebugRandomizeRng()</c>, which
    /// reseeds from the wall clock at one-second granularity and would make
    /// every scenario built on it intermittently irreproducible.
    ///
    /// The policy here is "play the first legal card, else end turn". It is not
    /// meant to play well — the oracle never chooses. It exists to drive enough
    /// decisions to time one.
    /// </remarks>
    /// <summary>
    /// Jumps into a named encounter, through the game's own console command.
    /// </summary>
    /// <remarks>
    /// This used to hardcode <c>BowlbugsWeak</c> and call
    /// <c>EnterRoomDebug</c> directly, because <c>FightConsoleCmd</c> calls
    /// <c>DebugRandomizeRng</c> and H6 concluded the command should therefore be
    /// avoided. <c>sts2_determinism</c> patches that to nothing now, so the
    /// command is deterministic and there is no reason to reimplement it.
    ///
    /// It still lands at <c>MapPointType.Unassigned</c> — the console command
    /// does that itself — so the run reaches combat off the map. For scenario
    /// construction that is the point: reaching a chosen fight by travelling
    /// means seed-hunting for a seed that puts one where you want it, which is
    /// what H6 exists to avoid.
    ///
    /// Returns the console's own refusal message when it declines, rather than a
    /// message of ours: it knows why better than we do.
    /// </remarks>
    public static async Task<string?> EnterFight(RunState state, string encounter)
    {
        Driver.Stage = "entering fight";
        CmdResult result = new FightConsoleCmd().Process(Seat.Me(state), new[] { encounter });
        if (!result.success)
        {
            return result.msg;
        }

        if (result.task != null)
        {
            await result.task;
        }

        return null;
    }

    /// <summary>Puts a power on a creature in the fight.</summary>
    /// <remarks>
    /// Through <c>ApplyPowerConsoleCmd</c>, which goes to <c>PowerCmd.Apply</c>
    /// or <c>ModifyAmount</c> the way a card does. So a constructed debuff gets
    /// its <c>SkipNextDurationTick</c> set, and the state reached is a state the
    /// game reaches -- which is the whole reason to hold a simulator to it.
    ///
    /// The console command addresses a creature by its position in the combat
    /// state's list, and an observation addresses one by <c>CombatId</c>. The
    /// translation belongs here, so a scenario names the creature the
    /// observation named.
    /// </remarks>
    public static async Task<string?> ApplyPower(RunState state, string power, int amount, uint target)
    {
        Driver.Stage = "applying power";
        CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
        if (combat == null)
        {
            return "This doesn't appear to be a combat!";
        }

        int at = -1;
        for (int i = 0; i < combat.Creatures.Count; i++)
        {
            if (combat.Creatures[i].CombatId == target)
            {
                at = i;
                break;
            }
        }
        if (at < 0)
        {
            return $"No creature {target} in this combat.";
        }

        CmdResult result = new ApplyPowerConsoleCmd().Process(
            Seat.Me(state),
            new[]
            {
                power,
                amount.ToString(CultureInfo.InvariantCulture),
                at.ToString(CultureInfo.InvariantCulture),
            });
        if (!result.success)
        {
            return result.msg;
        }

        if (result.task != null)
        {
            await result.task;
        }

        return null;
    }

    /// <summary>Sets a creature's hit points through <c>CreatureCmd.SetCurrentHp</c>.</summary>
    /// <remarks>
    /// The player when <paramref name="target"/> is null, in a fight or out.
    /// Otherwise the creature in the fight with that <c>CombatId</c>.
    /// </remarks>
    public static async Task<string?> SetHp(RunState state, int hp, uint? target)
    {
        Driver.Stage = "setting hit points";
        Creature creature = Seat.Me(state).Creature;
        if (target is uint id)
        {
            CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
            if (combat == null)
            {
                return "This doesn't appear to be a combat!";
            }
            Creature? found = combat.Creatures.FirstOrDefault(c => c.CombatId == id);
            if (found == null)
            {
                return $"No creature {id} in this combat.";
            }
            creature = found;
        }

        await CreatureCmd.SetCurrentHp(creature, hp);
        return null;
    }

    /// <summary>Enchants the card at a hand index.</summary>
    /// <remarks>
    /// Through <c>EnchantConsoleCmd</c>, which is <c>CardCmd.Enchant</c> on the
    /// fight's copy. <c>CardCmd.Enchant</c> throws on a card the enchantment
    /// does not take, and that comes back as the console's refusal.
    /// </remarks>
    public static string? Enchant(RunState state, string enchantment, int amount, int index)
    {
        Driver.Stage = "enchanting";
        try
        {
            CmdResult result = new EnchantConsoleCmd().Process(
                Seat.Me(state),
                new[]
                {
                    enchantment,
                    amount.ToString(CultureInfo.InvariantCulture),
                    index.ToString(CultureInfo.InvariantCulture),
                });
            return result.success ? null : result.msg;
        }
        catch (InvalidOperationException e)
        {
            return e.Message;
        }
    }

    /// <summary>Jumps the run to a named event, off the map.</summary>
    /// <remarks>
    /// The same shape as <see cref="EnterFight"/>, and through the game's own
    /// <c>EventConsoleCmd</c> for the same reason: it is what
    /// <c>AppendToMapPointHistory</c> and <c>EnterRoom(new EventRoom(...))</c>
    /// are called from, so the room the run lands in is the one the game builds
    /// rather than one assembled here.
    ///
    /// It exists because one event cannot be reached any other way worth
    /// testing. <c>enter_room</c> takes a <c>RoomType</c>, so `Event` rolls
    /// whichever event the seed offers, and reaching the Crystal Sphere that way
    /// meant 515 recorded steps to floor 29.
    /// </remarks>
    public static async Task<string?> EnterEvent(RunState state, string entry)
    {
        Driver.Stage = "entering event";
        if (entry == ModelDb.Event<TheArchitect>().Id.Entry)
        {
            // The console's list leaves out the run's last event, which the
            // game enters from EnterNextAct with no map point of its own.
            await RunManager.Instance.EnterRoom(new EventRoom(ModelDb.Event<TheArchitect>()));
            return null;
        }

        CmdResult result = new EventConsoleCmd().Process(Seat.Me(state), new[] { entry });
        if (!result.success)
        {
            return result.msg;
        }

        if (result.task != null)
        {
            await result.task;
        }

        return null;
    }

    /// <summary>
    /// Waits until our decision has fully resolved and the game wants another.
    /// </summary>
    /// <remarks>
    /// Two phases, because an empty queue is ambiguous. <c>ActionExecutor</c>
    /// reports <c>IsRunning == false</c> and <c>FinishedExecutingActions()</c>
    /// hands back <c>Task.CompletedTask</c> both when the queue has drained and
    /// when it has not yet picked our action up, so waiting on those alone
    /// returns immediately and lets the caller run ahead of the game. An
    /// earlier version did exactly that: 300 decisions "executed" in 70ms while
    /// only five actions actually ran.
    ///
    /// So phase one waits on the action we issued, via its own completion
    /// source, which cannot be satisfied early. Phase two then drains the
    /// cascade it triggered — enemy turns, triggers, draws.
    ///
    /// **Draining is still not enough, and the protocol is what proved it.**
    /// <c>EndPlayerTurnAction.ExecuteAction</c> calls <c>PlayerCmd.EndTurn</c>
    /// and returns <c>Task.CompletedTask</c> immediately, so the action reports
    /// itself finished while the turn transition — enemy turn, next draw — has
    /// not begun. The queue is genuinely empty in that gap. The in-process
    /// driver never noticed because it re-read the live model on the next
    /// iteration; over the protocol the observation is authoritative, and a
    /// client acting on it was found a whole turn behind the oracle (it believed
    /// turn 2 while the game was on turn 3) with no command in between.
    ///
    /// So phase three waits for the game to say it wants input again, using its
    /// own notion rather than a settling delay: actions re-enabled and the turn
    /// phase back to <c>Play</c>. For a turn-ending decision that is still
    /// ambiguous — those hold at the moment of issue too — so the caller passes
    /// the turn number it is leaving and the wait requires the counter to have
    /// advanced past it. A monotonic counter cannot be faked by a transient.
    /// </remarks>
    /// <param name="turnMustAdvancePast">
    /// For decisions that end the turn: the turn number being left. Quiescence
    /// then requires a strictly greater turn number, not merely a drained queue.
    /// </param>
    /// <summary>
    /// Puts a card into the run, where the game would put it.
    /// </summary>
    /// <remarks>
    /// Hand while a fight is on, deck otherwise: granting to hand out of combat
    /// would put the card somewhere the game has no pile for.
    ///
    /// Registration first, and it is not optional. <c>CardPileCmd.Add</c> only
    /// rejects an unregistered card when it already believes itself in combat,
    /// so adding to the hand appears to succeed — and the card then throws
    /// "must be added to a CombatState before playing it" when played, several
    /// commands later.
    ///
    /// <c>CreateCard</c> registers it, as the game's own <c>card</c> console
    /// command does, and then runs <c>AfterCreated</c>. A card built and only
    /// registered skips that, which leaves Spoils Map's act at -1.
    /// </remarks>
    public static async Task GrantCard(RunState state, CardModel canonical)
    {
        Player me = Seat.Me(state);
        CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;

        if (combat != null && CombatManager.Instance.IsInProgress)
        {
            CardModel card = combat.CreateCard(canonical, me);
            await CardPileCmd.Add(card, PileType.Hand.GetPile(me));
        }
        else
        {
            CardModel card = state.CreateCard(canonical, me);
            await CardPileCmd.Add(card, me.Deck);
        }
    }
}
