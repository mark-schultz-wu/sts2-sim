using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>
/// The line-delimited JSON protocol the simulator drives the oracle with (H9).
/// </summary>
/// <remarks>
/// One request per line, one response per request, strictly in order. There is
/// no server-initiated traffic after the greeting, so the client never needs to
/// demultiplex — a decision and its observation are a matched pair, which is
/// what makes H3's "one command, one decision" checkable from outside.
///
/// **Every failure answers.** An unknown command, a bad index, an unplayable
/// card and an exception from the game all produce an <c>error</c> response
/// naming the cause. Nothing no-ops silently, because a no-op that returns the
/// previous observation is indistinguishable from a decision that legitimately
/// changed nothing — and that is precisely how a simulator bug turns into a
/// passing test (H10).
/// </remarks>
internal static class Protocol
{
    /// <summary>
    /// Bumped whenever a client that understood the old shape would misread the
    /// new one. The client refuses to proceed on mismatch rather than guessing.
    /// </summary>
    private const int Version = 1;

    private static readonly JsonSerializerOptions Json = new JsonSerializerOptions { WriteIndented = false };

    private static Channel _channel = null!;
    private static RunState _state = null!;
    private static int _seq;

    public static async Task Serve(RunState state)
    {
        _state = state;
        _channel = await Channel.Open();

        Log.Info($"[headless] serving protocol v{Version} over {_channel.Describe}");

        await Send(new Dictionary<string, object?>
        {
            ["type"] = "hello",
            ["protocol"] = Version,
            ["game"] = "v0.107.1",
            ["assemblyHash"] = 1692500715,
            ["commands"] = new[] { "start", "fight", "travel", "play_card", "end_turn", "answer", "grant", "enter_room", "choose_option", "buy", "observe", "quit" },
        });

        while (true)
        {
            string? line = await _channel.ReadLine();
            if (line == null)
            {
                // The peer going away is a normal shutdown, not an error: a
                // client that crashes should not leave a game process behind
                // (H11), which is what a blocking read with no EOF handling does.
                Log.Info("[headless] channel closed; shutting down");
                return;
            }

            if (line.Trim().Length == 0)
            {
                continue;
            }

            if (await Handle(line))
            {
                return;
            }
        }
    }

    /// <summary>Dispatches one request. Returns true when the client asked to stop.</summary>
    private static async Task<bool> Handle(string line)
    {
        string cmd = "?";
        try
        {
            using JsonDocument doc = JsonDocument.Parse(line);
            JsonElement req = doc.RootElement;

            if (!req.TryGetProperty("cmd", out JsonElement cmdEl) || cmdEl.ValueKind != JsonValueKind.String)
            {
                await Error("?", "request has no 'cmd' string");
                return false;
            }

            cmd = cmdEl.GetString()!;
            GameErrors.Drain();

            // While the game is stopped mid-action, the only things that make
            // sense are answering it, looking at it, and leaving. Anything else
            // would advance a client that has not noticed the game is waiting,
            // and the pending must survive the refusal.
            if (Pending.Current != null && cmd is not ("answer" or "observe" or "quit"))
            {
                await Error(cmd, $"a '{Pending.Current.Kind}' decision is pending; answer it first");
                return false;
            }

            if (Pending.Current == null && cmd == "answer")
            {
                await Error(cmd, "nothing is pending");
                return false;
            }

            // A finished run accepts nothing that would advance it. Without
            // this the oracle keeps taking commands after a defeat — a dead
            // player was observed healing at a rest site, which the real game
            // cannot reach, so every observation after that point described a
            // state no simulator should be asked to reproduce.
            if (_state.IsGameOver && cmd is not ("observe" or "start" or "quit"))
            {
                bool won = _state.CurrentRoom?.IsVictoryRoom ?? false;
                await Error(cmd,
                    $"the run is over ({(won ? "victory" : "defeat")}); start a new run or quit");
                return false;
            }

            switch (cmd)
            {
                case "answer":
                    await Answer(req);
                    return false;

                case "start":
                    await Start(req);
                    return false;

                case "fight":
                    await Fight(req);
                    return false;

                case "enter_room":
                    await EnterRoom(req);
                    return false;

                case "choose_option":
                    await ChooseOption(req);
                    return false;

                case "buy":
                    await Buy(req);
                    return false;

                case "grant":
                    await Grant(req);
                    return false;

                case "travel":
                    await Travel(req);
                    return false;

                case "play_card":
                    await PlayCard(req);
                    return false;

                case "end_turn":
                    await EndTurn(req);
                    return false;

                case "observe":
                    await Observe(req, "observe");
                    return false;

                case "quit":
                    await Send(new Dictionary<string, object?> { ["type"] = "bye" });
                    return true;

                default:
                    await Error(cmd, $"unknown command '{cmd}'");
                    return false;
            }
        }
        catch (JsonException e)
        {
            await Error(cmd, $"malformed JSON: {e.Message}");
            return false;
        }
        catch (Exception e)
        {
            // The game throwing must reach the client. Swallowing it here would
            // leave the simulator comparing against a state the oracle no longer
            // understands.
            Log.Error($"[headless] {cmd} failed: {e}");
            await Error(cmd, $"{e.GetType().Name}: {e.Message}");
            return false;
        }
    }

    /// <summary>Discards the current run and starts another (H6's fresh-run case).</summary>
    private static async Task Start(JsonElement req)
    {
        string? seed = Str(req, "seed");
        string? character = Str(req, "character");
        int ascension = req.TryGetProperty("ascension", out JsonElement a) && a.TryGetInt32(out int lvl)
            ? lvl
            : 0;

        _state = await Driver.Restart(seed, character, ascension, Modifiers(req));
        await Observe(req, "start");
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
    private static async Task Fight(JsonElement req)
    {
        string? encounter = Str(req, "encounter");
        if (encounter == null)
        {
            await Error("fight", "fight requires an 'encounter' entry name");
            return;
        }

        string? refused = await Driver.EnterFight(_state, encounter);
        if (refused != null)
        {
            await Error("fight", refused);
            return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "fight", resolved);
    }

    private static async Task Grant(JsonElement req)
    {
        Player me = _state.Players[0];
        string? card = Str(req, "card");
        string? relic = Str(req, "relic");

        if (card == null && relic == null)
        {
            await Error("grant", "grant requires 'card' or 'relic'");
            return;
        }

        if (card != null)
        {
            await Driver.GrantCard(_state, Lookup<CardModel>(card).ToMutable());
        }

        if (relic != null)
        {
            await RelicCmd.Obtain(Lookup<RelicModel>(relic).ToMutable(), me);
        }

        await Observe(req, "grant");
    }

    /// <summary>Finds a canonical model by its id entry, or says what exists.</summary>
    private static T Lookup<T>(string id) where T : AbstractModel
    {
        List<T> all = ModelDb.AllAbstractModelSubtypes
            .Where(t => t.IsSubclassOf(typeof(T)))
            .Select(t => ModelDb.GetById<T>(ModelDb.GetId(t)))
            .ToList();

        T? found = all.FirstOrDefault(x => string.Equals(x.Id.Entry, id, StringComparison.OrdinalIgnoreCase));
        if (found == null)
        {
            // Naming something that does not exist is a client bug; starting
            // without it would hide that behind a divergence three steps later.
            throw new ArgumentException(
                $"no {typeof(T).Name} with id '{id}' ({all.Count} known)");
        }

        return found;
    }

    private static ModifierModel[] Modifiers(JsonElement req)
    {
        if (!req.TryGetProperty("modifiers", out JsonElement el) || el.ValueKind != JsonValueKind.Array)
        {
            return Array.Empty<ModifierModel>();
        }

        // Every modifier in the build, not the curated GoodModifiers/BadModifiers
        // pair — those are the run-setup offer lists and between them omit any
        // modifier the game does not currently offer, which is exactly the kind
        // a scenario would be written to probe. Mirrors ModelDb's own
        // DebugAfflictions pattern, since ModelDb.Get(Type) is private.
        List<ModifierModel> all = ModelDb.AllAbstractModelSubtypes
            .Where(t => t.IsSubclassOf(typeof(ModifierModel)))
            .Select(t => ModelDb.GetById<ModifierModel>(ModelDb.GetId(t)))
            .ToList();

        var mods = new List<ModifierModel>();
        foreach (JsonElement m in el.EnumerateArray())
        {
            string id = m.GetString() ?? "";
            ModifierModel? found = all.FirstOrDefault(x =>
                string.Equals(x.Id.Entry, id, StringComparison.OrdinalIgnoreCase));

            // Naming a modifier that does not exist is a client bug, and
            // silently starting an unmodified run would hide it behind a
            // divergence three decisions later.
            if (found == null)
            {
                throw new ArgumentException(
                    $"no such modifier '{id}'; known: {string.Join(" ", all.Select(x => x.Id.Entry).OrderBy(s => s))}");
            }

            mods.Add(found.ToMutable());
        }

        return mods.ToArray();
    }

    /// <summary>
    /// Answers the question the game is parked on, then resumes it.
    /// </summary>
    /// <remarks>
    /// Indices are into the options the pending stated, not into any pile: the
    /// option list is often a filtered subset of a pile, and two copies of one
    /// card are indistinguishable by name, which is the whole reason the answer
    /// is positional.
    ///
    /// Validation happens here rather than being left to the game. The selector
    /// contract is that whatever it returns is taken as the player's choice, so
    /// an out-of-range or over-long answer would not be rejected downstream —
    /// it would silently become a different move.
    /// </remarks>
    private static async Task Answer(JsonElement req)
    {
        Pending pending = Pending.Current!;

        // Validation belongs to the pending, not here: the selector contract is
        // that whatever it returns is taken as the player's choice, so an
        // out-of-range or over-long answer would not be rejected downstream —
        // it would silently become a different move.
        string? rejected = pending.Accept(req);
        if (rejected != null)
        {
            await Error("answer", rejected);
            return;
        }

        // The action resumes and may park again, end the turn, or finish the
        // fight; the same wait decides which, so nesting needs no special case.
        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "answer", resolved);
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
    private static async Task EnterRoom(JsonElement req)
    {
        // Unassigned is a RoomType the enum carries and nothing can usefully be
        // jumped to: it is the "no map point" placeholder `fight` lands on. The
        // game's own `room` command filters it out of its completions for the
        // same reason. Listing it as an option and then accepting it would be
        // offering a state with no meaning.
        string[] jumpable = Enum.GetNames(typeof(RoomType))
            .Where(n => n != nameof(RoomType.Unassigned))
            .ToArray();

        string? want = Str(req, "type");
        if (want == null
            || !Enum.TryParse(want, ignoreCase: true, out RoomType type)
            || !jumpable.Contains(type.ToString()))
        {
            await Error("enter_room",
                $"enter_room requires a 'type' from: {string.Join(" ", jumpable)}");
            return;
        }

        // The game's `room` command guards this and ours did not, so entering a
        // room before `start` reached EnterRoomDebug with no run behind it.
        if (!RunManager.Instance.IsInProgress)
        {
            await Error("enter_room", "a run is not in progress; start one first");
            return;
        }

        await RunManager.Instance.EnterRoomDebug(type);
        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "enter_room", resolved);
    }

    /// <summary>
    /// Picks an option in a rest site or an event.
    /// </summary>
    /// <remarks>
    /// One command for both because both are `ChooseLocalOption(int)` on their
    /// respective synchronizer, and the room type disambiguates. Options are
    /// addressed by index into what the observation reported — and that list
    /// *shrinks*: choosing a rest site option removes it, and clears the rest
    /// entirely when `ShouldDisableRemainingRestSiteOptions` fires. So an index
    /// is only meaningful against the observation it came from.
    /// </remarks>
    private static async Task ChooseOption(JsonElement req)
    {
        if (!req.TryGetProperty("index", out JsonElement el) || !el.TryGetInt32(out int index))
        {
            await Error("choose_option", "choose_option requires an integer 'index'");
            return;
        }

        switch (_state.CurrentRoom)
        {
            case RestSiteRoom:
            {
                RestSiteSynchronizer sync = RunManager.Instance.RestSiteSynchronizer;
                IReadOnlyList<RestSiteOption> options = sync.GetLocalOptions();
                if (index < 0 || index >= options.Count)
                {
                    await Error("choose_option",
                        $"option {index} out of range; {options.Count} offered");
                    return;
                }

                // Checked here because the synchronizer does not: ChooseOption
                // validates the index but never consults IsEnabled, so a
                // disabled option would be taken rather than refused.
                if (!options[index].IsEnabled)
                {
                    await Error("choose_option", $"rest site option '{options[index].OptionId}' is disabled");
                    return;
                }

                await sync.ChooseLocalOption(index);
                break;
            }

            case EventRoom:
            {
                EventSynchronizer sync = RunManager.Instance.EventSynchronizer;
                IReadOnlyList<EventOption> options = sync.GetLocalEvent().CurrentOptions;
                if (index < 0 || index >= options.Count)
                {
                    await Error("choose_option", $"option {index} out of range; {options.Count} offered");
                    return;
                }

                if (options[index].IsLocked)
                {
                    await Error("choose_option", $"event option '{options[index].TextKey}' is locked");
                    return;
                }

                sync.ChooseLocalOption(index);

                // Tracked, never awaited here. ChooseLocalOption returns void
                // and starts its work in the background, so without this the
                // option reads as never chosen. But awaiting the tasks directly
                // deadlocks: an event option may park a question for the client,
                // and a handler blocked here could never send it.
                BackgroundWork.Track(sync.AwaitPendingOptionTasks());
                break;
            }

            default:
                await Error("choose_option",
                    $"the current room ({_state.CurrentRoom?.GetType().Name ?? "none"}) has no options");
                return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "choose_option", resolved);
    }

    /// <summary>
    /// Buys one shop entry, addressed by index into the reported inventory.
    /// </summary>
    /// <remarks>
    /// <c>OnTryPurchaseWrapper</c> already refuses when out of stock or short of
    /// gold, and reports it through <c>PurchaseFailed</c> — an event the oracle
    /// does not subscribe to. Its return value is the whole signal available
    /// here, so a false is turned into a protocol error rather than an
    /// observation that silently shows nothing bought.
    /// </remarks>
    private static async Task Buy(JsonElement req)
    {
        if (_state.CurrentRoom is not MerchantRoom shop)
        {
            await Error("buy", "not in a shop");
            return;
        }

        if (!req.TryGetProperty("index", out JsonElement el) || !el.TryGetInt32(out int index))
        {
            await Error("buy", "buy requires an integer 'index'");
            return;
        }

        MerchantInventory inventory = shop.GetLocalInventory();
        List<MerchantEntry> entries = inventory.AllEntries.ToList();
        if (index < 0 || index >= entries.Count)
        {
            await Error("buy", $"entry {index} out of range; {entries.Count} on offer");
            return;
        }

        MerchantEntry entry = entries[index];
        if (!await entry.OnTryPurchaseWrapper(inventory))
        {
            await Error("buy",
                $"purchase refused: cost {entry.Cost}, gold {_state.Players[0].Gold}, "
                + $"stocked {entry.IsStocked}");
            return;
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "buy", resolved);
    }

    /// <summary>
    /// Travels to a map node, addressed by coordinate.
    /// </summary>
    /// <remarks>
    /// Coordinates rather than an index into the reported list, because the
    /// travelable set is not a stable sequence — it changes size and order with
    /// the rules that apply, and an index into it would silently mean a
    /// different node under `Winged Boots` than without.
    ///
    /// The legality check is not redundant with the game's. <c>EnterMapCoord</c>
    /// refuses only coords already visited (via <c>AddVisitedMapCoord</c>
    /// returning false) and otherwise enters *any* coordinate handed to it,
    /// silently, including one no rule permits. It is the one command here where
    /// the model layer would let an illegal move through, which is exactly why
    /// H2 requires the oracle to publish the legal set rather than trusting a
    /// rejection that never comes.
    /// </remarks>
    private static async Task Travel(JsonElement req)
    {
        if (!req.TryGetProperty("col", out JsonElement colEl) || !colEl.TryGetInt32(out int col)
            || !req.TryGetProperty("row", out JsonElement rowEl) || !rowEl.TryGetInt32(out int row))
        {
            await Error("travel", "travel requires integer 'col' and 'row'");
            return;
        }

        if (CombatManager.Instance.IsInProgress)
        {
            await Error("travel", "cannot travel during combat");
            return;
        }

        IReadOnlyList<MapPoint> legal = MapLegality.TravelableFrom(_state);
        MapPoint? target = legal.FirstOrDefault(p => p.coord.col == col && p.coord.row == row);
        if (target == null)
        {
            await Error("travel",
                $"({col},{row}) is not travelable from here; legal: "
                + string.Join(" ", legal.Select(p => $"({p.coord.col},{p.coord.row})")));
            return;
        }

        await RunManager.Instance.EnterMapCoord(target.coord);
        await Observe(req, "travel");
    }

    /// <summary>
    /// Plays one card from hand, addressed by its index in the observation.
    /// </summary>
    /// <remarks>
    /// Cards are addressed positionally because that is how the observation
    /// reports the hand and how a hand containing two identical cards can be
    /// disambiguated at all. Targets are addressed by <c>CombatId</c> instead —
    /// a stable identity that does not shift when a creature dies, which an
    /// index into the living-creature list would.
    /// </remarks>
    private static async Task PlayCard(JsonElement req)
    {
        PlayerCombatState? pcs = _state.Players[0].PlayerCombatState;
        CombatState? combat = (_state.CurrentRoom as CombatRoom)?.CombatState;
        if (pcs == null || combat == null)
        {
            await Error("play_card", "not in combat");
            return;
        }

        if (!req.TryGetProperty("index", out JsonElement idxEl) || !idxEl.TryGetInt32(out int index))
        {
            await Error("play_card", "play_card requires an integer 'index'");
            return;
        }

        if (index < 0 || index >= pcs.Hand.Cards.Count)
        {
            await Error("play_card", $"index {index} out of range; hand holds {pcs.Hand.Cards.Count}");
            return;
        }

        CardModel card = pcs.Hand.Cards[index];

        Creature? target = null;
        if (req.TryGetProperty("target", out JsonElement tEl) && tEl.ValueKind != JsonValueKind.Null)
        {
            if (!tEl.TryGetUInt32(out uint targetId))
            {
                await Error("play_card", "'target' must be a creature id");
                return;
            }

            target = combat.Creatures.FirstOrDefault(c => c.CombatId == targetId);
            if (target == null)
            {
                await Error("play_card", $"no creature with id {targetId}");
                return;
            }
        }

        // Refuse rather than enqueue. The game would reject it too, but further
        // in and less legibly, and H2 exists so the client can be held to the
        // legal set the oracle publishes.
        if (!card.CanPlay(out UnplayableReason reason, out AbstractModel? preventer))
        {
            await Error("play_card",
                $"{card.Id.Entry} is not playable: {reason}"
                + (preventer != null ? $" (blocked by {preventer.Id.Entry})" : ""));
            return;
        }

        if (!card.CanPlayTargeting(target))
        {
            await Error("play_card",
                $"{card.Id.Entry} cannot target {(target == null ? "nothing" : target.CombatId.ToString())}"
                + $"; it targets {card.TargetType}");
            return;
        }

        var action = new PlayCardAction(card, target);
        RunManager.Instance.ActionQueueSet.EnqueueWithoutSynchronizing(action);
        bool resolved = await Driver.Quiesce(action);
        await Observe(req, "play_card", resolved);
    }

    private static async Task EndTurn(JsonElement req)
    {
        Player me = _state.Players[0];
        PlayerCombatState? pcs = me.PlayerCombatState;
        if (pcs == null || !CombatManager.Instance.IsInProgress)
        {
            await Error("end_turn", "not in combat");
            return;
        }

        bool resolved = await Driver.EndTurn(_state);
        await Observe(req, "end_turn", resolved);
    }

    /// <summary>
    /// Reports the state after a decision.
    /// </summary>
    /// <remarks>
    /// The digest always ships; the full observation only when asked, per
    /// request or for the whole session via <c>STS2_HEADLESS_OBS=full</c>. That is
    /// H5's bargain: ~1.6KB a step is wasteful when almost every step agrees,
    /// and D1 means the one step that disagrees can be replayed in full.
    ///
    /// <paramref name="resolved"/> is false when combat ended mid-cascade and
    /// the decision was abandoned rather than completed. It is reported rather
    /// than smoothed over, because a simulator that resolved it fully has
    /// diverged even though the resulting state may look agreeable.
    /// </remarks>
    private static async Task Observe(JsonElement req, string cmd, bool resolved = true)
    {
        bool full = Driver.FullObservations
            || (req.TryGetProperty("full", out JsonElement f) && f.ValueKind == JsonValueKind.True);

        // A parked game gets a question rather than an observation. Note this
        // replaces the reply to whatever command was issued: one request still
        // yields exactly one response, just not always the same kind.
        var msg = Pending.Current is { } pending
            ? Described(pending, cmd)
            : new Dictionary<string, object?>
            {
                ["type"] = "observation",
                ["seq"] = ++_seq,
                ["cmd"] = cmd,
                ["resolved"] = resolved,
            };

        if (full)
        {
            (string json, string digest) = Observation.Snapshot(_state);
            msg["digest"] = digest;

            // Parsed back rather than nested as a string, so the client sees one
            // document rather than JSON inside JSON.
            using JsonDocument doc = JsonDocument.Parse(json);
            msg["state"] = doc.RootElement.Clone();
        }
        else
        {
            msg["digest"] = Observation.Digest(_state);
        }

        List<string> errors = GameErrors.Drain();
        if (errors.Count > 0)
        {
            // Reported, never suppressed: the command may have half-happened,
            // and only the client can decide whether to trust the state.
            Log.Warn($"[headless] {errors.Count} game error(s) during {cmd}");
            msg["gameErrors"] = errors;
        }

        await Send(msg);
    }

    /// <summary>A pending's own wire form, plus the protocol's envelope.</summary>
    private static Dictionary<string, object?> Described(Pending pending, string cmd)
    {
        var msg = new Dictionary<string, object?>
        {
            ["type"] = "pending",
            ["seq"] = ++_seq,
            ["cmd"] = cmd,
        };

        foreach (KeyValuePair<string, object?> field in pending.Describe())
        {
            msg[field.Key] = field.Value;
        }

        return msg;
    }

    private static async Task Error(string cmd, string message)
    {
        Log.Warn($"[headless] rejecting {cmd}: {message}");
        await Send(new Dictionary<string, object?>
        {
            ["type"] = "error",
            ["cmd"] = cmd,
            ["message"] = message,
        });
    }

    private static async Task Send(Dictionary<string, object?> msg) =>
        await _channel.WriteLine(JsonSerializer.Serialize(msg, Json));

    private static string? Str(JsonElement req, string name) =>
        req.TryGetProperty(name, out JsonElement el) && el.ValueKind == JsonValueKind.String
            ? el.GetString()
            : null;
}
