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
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace StsOracle;

/// <summary>
/// The line-delimited JSON protocol the simulator drives the oracle with (R10).
/// </summary>
/// <remarks>
/// One request per line, one response per request, strictly in order. There is
/// no server-initiated traffic after the greeting, so the client never needs to
/// demultiplex — a decision and its observation are a matched pair, which is
/// what makes R4's "one command, one decision" checkable from outside.
///
/// **Every failure answers.** An unknown command, a bad index, an unplayable
/// card and an exception from the game all produce an <c>error</c> response
/// naming the cause. Nothing no-ops silently, because a no-op that returns the
/// previous observation is indistinguishable from a decision that legitimately
/// changed nothing — and that is precisely how a simulator bug turns into a
/// passing test (R12).
/// </remarks>
internal static class Protocol
{
    /// <summary>
    /// Bumped whenever a client that understood the old shape would misread the
    /// new one. The client refuses to proceed on mismatch rather than guessing.
    /// </summary>
    private const int Version = 1;

    private static readonly JsonSerializerOptions Json = new JsonSerializerOptions { WriteIndented = false };

    private static IChannel _channel = null!;
    private static RunState _state = null!;
    private static int _seq;

    public static async Task Serve(RunState state)
    {
        _state = state;
        _channel = await Channel.Open();

        // Once per process, not per run: UseSelector throws if one is already
        // active, and CardSelectCmd.Reset leaves the stack alone while TestMode
        // is on, so this survives every Restart.
        CardSelectCmd.UseSelector(new OracleCardSelector());
        GameErrors.Watch();

        Log.Info($"[oracle] serving protocol v{Version} over {_channel.Describe}");

        await Send(new Dictionary<string, object?>
        {
            ["type"] = "hello",
            ["protocol"] = Version,
            ["game"] = "v0.107.1",
            ["assemblyHash"] = 1692500715,
            ["commands"] = new[] { "start", "enter_fight", "travel", "play_card", "end_turn", "answer", "grant", "observe", "quit" },
        });

        while (true)
        {
            string? line = await _channel.ReadLine();
            if (line == null)
            {
                // The peer going away is a normal shutdown, not an error: a
                // client that crashes should not leave a game process behind
                // (R14), which is what a blocking read with no EOF handling does.
                Log.Info("[oracle] channel closed; shutting down");
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

            switch (cmd)
            {
                case "answer":
                    await Answer(req);
                    return false;

                case "start":
                    await Start(req);
                    return false;

                case "enter_fight":
                    await OracleDriver.EnterFight(_state);
                    await Observe(req, "enter_fight");
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
            Log.Error($"[oracle] {cmd} failed: {e}");
            await Error(cmd, $"{e.GetType().Name}: {e.Message}");
            return false;
        }
    }

    /// <summary>Discards the current run and starts another (R7's fresh-run case).</summary>
    private static async Task Start(JsonElement req)
    {
        string? seed = Str(req, "seed");
        _state = await OracleDriver.Restart(seed, Modifiers(req));
        await Observe(req, "start");
    }

    /// <summary>
    /// Puts a named card or relic into the run (R7).
    /// </summary>
    /// <remarks>
    /// Through the game's own commands — <c>CardPileCmd.Add</c> and
    /// <c>RelicCmd.Obtain</c> — never by pushing onto a collection. The load
    /// path skips <c>AfterObtained</c>, so a poked-in relic produces a state the
    /// game never produces, and the divergence surfaces later as a simulator
    /// bug that is not one.
    /// </remarks>
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
            CardModel found = Lookup<CardModel>(card).ToMutable();
            CombatState? combat = (_state.CurrentRoom as CombatRoom)?.CombatState;

            // Hand while a fight is on, deck otherwise: granting to hand out of
            // combat would put the card somewhere the game has no pile for.
            if (combat != null && CombatManager.Instance.IsInProgress)
            {
                // Registration first, and it is not optional. `CardPileCmd.Add`
                // only rejects an unregistered card when it already believes
                // itself in combat, so adding to the hand appears to succeed —
                // and the card then throws "must be added to a CombatState
                // before playing it" when played, several commands later.
                combat.AddCard(found, me);
                await CardPileCmd.Add(found, PileType.Hand.GetPile(me));
            }
            else
            {
                _state.AddCard(found, me);
                await CardPileCmd.Add(found, me.Deck);
            }
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

        if (!req.TryGetProperty("cards", out JsonElement el) || el.ValueKind != JsonValueKind.Array)
        {
            await Error("answer", $"'{pending.Kind}' expects a 'cards' array of option indices");
            return;
        }

        var chosen = new List<CardModel>();
        var seen = new HashSet<int>();
        foreach (JsonElement e in el.EnumerateArray())
        {
            if (!e.TryGetInt32(out int i) || i < 0 || i >= pending.Options.Count)
            {
                await Error("answer", $"option index {e} out of range; {pending.Options.Count} option(s) offered");
                return;
            }

            // The same card twice would be one selection to the game and two to
            // the client, which is a divergence waiting to happen.
            if (!seen.Add(i))
            {
                await Error("answer", $"option index {i} given twice");
                return;
            }

            chosen.Add(pending.Options[i]);
        }

        if (chosen.Count < pending.Min || chosen.Count > pending.Max)
        {
            await Error("answer",
                $"selected {chosen.Count}, but this decision takes {pending.Min}..{pending.Max}");
            return;
        }

        pending.Answer(chosen);

        // The action resumes and may park again, end the turn, or finish the
        // fight; the same wait decides which, so nesting needs no special case.
        bool resolved = await OracleDriver.WaitForDecisionPoint();
        await Observe(req, "answer", resolved);
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
    /// R3 requires the oracle to publish the legal set rather than trusting a
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
        // in and less legibly, and R3 exists so the client can be held to the
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
        bool resolved = await OracleDriver.Quiesce(action);
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

        int leaving = pcs.TurnNumber;
        var action = new EndPlayerTurnAction(me, leaving);
        RunManager.Instance.ActionQueueSet.EnqueueWithoutSynchronizing(action);
        bool resolved = await OracleDriver.Quiesce(action, turnMustAdvancePast: leaving);
        await Observe(req, "end_turn", resolved);
    }

    /// <summary>
    /// Reports the state after a decision.
    /// </summary>
    /// <remarks>
    /// The digest always ships; the full observation only when asked, per
    /// request or for the whole session via <c>STS_ORACLE_OBS=full</c>. That is
    /// R6's bargain: ~1.6KB a step is wasteful when almost every step agrees,
    /// and R1 means the one step that disagrees can be replayed in full.
    ///
    /// <paramref name="resolved"/> is false when combat ended mid-cascade and
    /// the decision was abandoned rather than completed. It is reported rather
    /// than smoothed over, because a simulator that resolved it fully has
    /// diverged even though the resulting state may look agreeable.
    /// </remarks>
    private static async Task Observe(JsonElement req, string cmd, bool resolved = true)
    {
        bool full = OracleDriver.FullObservations
            || (req.TryGetProperty("full", out JsonElement f) && f.ValueKind == JsonValueKind.True);

        // A parked game gets a question rather than an observation. Note this
        // replaces the reply to whatever command was issued: one request still
        // yields exactly one response, just not always the same kind.
        var msg = Pending.Current is { } pending
            ? new Dictionary<string, object?>
            {
                ["type"] = "pending",
                ["seq"] = ++_seq,
                ["cmd"] = cmd,
                ["kind"] = pending.Kind,
                ["min"] = pending.Min,
                ["max"] = pending.Max,
                ["asker"] = pending.Asker,
                ["options"] = pending.Options.Select((c, i) => new Dictionary<string, object?>
                {
                    ["index"] = i,
                    ["card"] = Observation.Ident(c),
                }).ToList(),
            }
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
            Log.Warn($"[oracle] {errors.Count} game error(s) during {cmd}");
            msg["gameErrors"] = errors;
        }

        await Send(msg);
    }

    private static async Task Error(string cmd, string message)
    {
        Log.Warn($"[oracle] rejecting {cmd}: {message}");
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
