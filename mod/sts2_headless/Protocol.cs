using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Debug;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Entities.Ascension;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Entities.TreasureRelicPicking;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Models.Events;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;

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
    private static readonly JsonSerializerOptions Json = new JsonSerializerOptions { WriteIndented = false };

    private static Channel _channel = null!;
    private static RunState _state = null!;

    /// <summary>The run this process serves, for a command another mod registers.</summary>
    internal static RunState State => _state;

    /// <summary>The commands this mod answers itself, in the greeting's order.</summary>
    private static readonly string[] BuiltIn =
    {
        "start", "travel", "play_card", "end_turn", "use_potion", "answer", "choose_option", "buy",
        "proceed", "discard_potion", "observe", "quit",
    };

    /// <summary>Commands another mod answers, by name, as <see cref="Register"/> adds them.</summary>
    private static readonly Dictionary<string, Func<JsonElement, Task>> Registered = new();

    /// <summary>
    /// Answers <paramref name="cmd"/> with <paramref name="handler"/>, which
    /// replies through <see cref="Observe"/> or <see cref="Error"/> as this
    /// mod's own commands do. For a mod that adds commands, from its
    /// initializer, before the first command arrives.
    /// </summary>
    internal static void Register(string cmd, Func<JsonElement, Task> handler)
    {
        if (BuiltIn.Contains(cmd) || !Registered.TryAdd(cmd, handler))
        {
            throw new InvalidOperationException($"the command '{cmd}' is answered already");
        }
    }
    private static int _seq;

    public static async Task Serve(RunState state)
    {
        _state = state;
        _channel = await Channel.Open();

        Log.Info($"[headless] serving mod {ModIdentity.Identity} over {_channel.Describe}");

        // Derived, not transcribed. These were literals -- `"v0.107.1"` and
        // `1692500715` -- which made the handshake a constant validating
        // itself: a client checking them would agree with any build at all,
        // including one whose content had moved out from under its tables.
        // ReleaseInfo is where the literals were read from in the first place.
        ReleaseInfo? release = ReleaseInfoManager.Instance.ReleaseInfo;

        await Send(new Dictionary<string, object?>
        {
            ["type"] = "hello",
            // Which build of *this mod* is answering, as a hash of its own
            // source. The stamp below identifies the game and cannot catch a
            // mod that was edited and not redeployed, because redeploying a
            // mod moves neither the assembly nor the content hash. The client
            // refuses a mismatch at the handshake, which is two seconds in
            // rather than four hundred decisions in.
            ["mod"] = ModIdentity.Identity,
            ["game"] = release?.Version ?? "UNRELEASED",
            ["commit"] = release?.Commit ?? GitHelper.ShortCommitId ?? "UNKNOWN",
            ["assemblyHash"] = release?.MainAssemblyHash ?? 0,
            // The hash that actually matters to a simulator: XxHash32 over the
            // model-id namespace, which moves when content moves. An assembly
            // change that adds no content leaves generated id tables valid; a
            // content change invalidates them. The game uses this exact value
            // to refuse a replay recorded against different content.
            ["contentHash"] = ModelIdSerializationCache.Hash,
            ["commands"] = BuiltIn.Concat(Registered.Keys).ToArray(),
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
                await Error("?", Refusals.Malformed("request has no 'cmd' string"));
                return false;
            }

            cmd = cmdEl.GetString()!;
            GameErrors.Drain();
            Driver.Received(line);

            // While the game is stopped mid-action, the only things that make
            // sense are answering it, looking at it, starting over, and
            // leaving. Anything else would advance a client that has not
            // noticed the game is waiting, and the pending must survive the
            // refusal. A start abandons the run, and Restart cancels what it
            // was waiting on.
            // A reward screen leaves the belt open, and a full belt is where
            // discarding matters most, so a discard is the one other command
            // it takes.
            if (Pending.Current != null && cmd is not ("answer" or "observe" or "start" or "quit")
                && !(cmd == "discard_potion" && Pending.Current.Kind == "rewards"))
            {
                await Error(cmd, Refusals.Pending(Pending.Current.Kind));
                return false;
            }

            if (Pending.Current == null && cmd == "answer")
            {
                await Error(cmd, Refusals.NothingPending());
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
                await Error(cmd, Refusals.RunOver(won));
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

                case "choose_option":
                    await ChooseOption(req);
                    return false;

                case "buy":
                    await Buy(req);
                    return false;

                case "travel":
                    await Travel(req);
                    return false;

                case "proceed":
                    await Proceed(req);
                    return false;

                case "play_card":
                    await PlayCard(req);
                    return false;

                case "end_turn":
                    await EndTurn(req);
                    return false;

                case "discard_potion":
                    await DiscardPotion(req);
                    return false;

                case "use_potion":
                    await UsePotion(req);
                    return false;

                case "observe":
                    await Observe(req, "observe");
                    return false;

                case "quit":
                    await Send(new Dictionary<string, object?> { ["type"] = "bye" });
                    return true;

                default:
                    if (Registered.TryGetValue(cmd, out var handler))
                    {
                        await handler(req);
                        return false;
                    }
                    await Error(cmd, Refusals.Malformed($"unknown command '{cmd}'"));
                    return false;
            }
        }
        catch (JsonException e)
        {
            await Error(cmd, Refusals.Malformed($"malformed JSON: {e.Message}"));
            return false;
        }
        catch (Exception e)
        {
            // The game throwing must reach the client. Swallowing it here would
            // leave the simulator comparing against a state the oracle no longer
            // understands.
            //
            // As a fault and not a refusal. This is the adapter or the game
            // breaking, not the game refusing a move, and sending it as a
            // refusal filed a crash as a decision -- a client comparing
            // refusals would have counted it as the game saying no.
            await Fault(cmd, e);
            return false;
        }
    }

    /// <summary>
    /// The runs the profile has won with the character, as a count.
    /// </summary>
    /// <remarks>
    /// Absent means none, and anything but a count is refused.
    /// </remarks>
    private static bool Wins(JsonElement req, out int wins, out string malformed)
    {
        wins = 0;
        malformed = "";

        if (!req.TryGetProperty("wins", out JsonElement w))
        {
            return true;
        }

        if (w.ValueKind == JsonValueKind.Number && w.TryGetInt32(out wins) && wins >= 0)
        {
            return true;
        }

        malformed = "wins must be a count";
        return false;
    }

    /// <summary>
    /// The run's ascension, as a level name or as a number.
    /// </summary>
    /// <remarks>
    /// Names are the primary form, because every other content value on this
    /// protocol travels as the game's own name — a character, a room type, a
    /// modifier — and `"DeadlyEnemies"` says in a log what `9` does not. Numbers
    /// stay accepted: ascension is a scalar and callers that have one as an
    /// integer should not have to look up its name to send it.
    ///
    /// Absent means zero. A value that is neither is refused by name, and
    /// refused rather than thrown: a request this protocol cannot read is a
    /// malformed request, not the mod falling over.
    /// </remarks>
    private static bool Ascension(JsonElement req, out int ascension, out string malformed)
    {
        ascension = 0;
        malformed = "";

        if (!req.TryGetProperty("ascension", out JsonElement a))
        {
            return true;
        }

        if (a.ValueKind == JsonValueKind.Number && a.TryGetInt32(out int level))
        {
            ascension = level;
            return true;
        }

        if (a.ValueKind == JsonValueKind.String
            && Enum.TryParse(a.GetString(), ignoreCase: true, out AscensionLevel named))
        {
            ascension = (int)named;
            return true;
        }

        malformed =
            $"ascension must be a number or one of: {string.Join(" ", Enum.GetNames(typeof(AscensionLevel)))}";
        return false;
    }

    /// <summary>Discards the current run and starts another (H6's fresh-run case).</summary>
    private static async Task Start(JsonElement req)
    {
        string? seed = Str(req, "seed");
        string? character = Str(req, "character");

        if (!Ascension(req, out int ascension, out string malformed)
            || !Modifiers(req, out ModifierModel[] modifiers, out malformed)
            || !Wins(req, out int wins, out malformed))
        {
            await Error("start", Refusals.Malformed(malformed));
            return;
        }

        _state = await Driver.Restart(seed, character, ascension, modifiers);
        // The profile keeps every run the oracle won, and The Architect picks
        // the dialogue that ends a won run by the character's count. So the
        // run says what the count is. Nothing else in a run reads it.
        SaveManager.Instance.Progress
            .GetOrCreateCharacterStats(_state.Players[0].Character.Id).TotalWins = wins;
        await Observe(req, "start");
    }

    private static bool Modifiers(JsonElement req, out ModifierModel[] modifiers, out string malformed)
    {
        modifiers = Array.Empty<ModifierModel>();
        malformed = "";

        if (!req.TryGetProperty("modifiers", out JsonElement el) || el.ValueKind != JsonValueKind.Array)
        {
            return true;
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
            // An entry is the modifier's id, or `{"id", "character"}` for one
            // set up with a character, as Character Cards is.
            string? character = null;
            string id;
            if (m.ValueKind == JsonValueKind.Object)
            {
                id = m.TryGetProperty("id", out JsonElement idEl) ? idEl.GetString() ?? "" : "";
                if (m.TryGetProperty("character", out JsonElement charEl))
                {
                    character = charEl.GetString();
                }
            }
            else
            {
                id = m.GetString() ?? "";
            }

            ModifierModel? found = all.FirstOrDefault(x =>
                string.Equals(x.Id.Entry, id, StringComparison.OrdinalIgnoreCase));

            // Naming a modifier that does not exist is a client bug, and
            // silently starting an unmodified run would hide it behind a
            // divergence three decisions later.
            if (found == null)
            {
                malformed =
                    $"no such modifier '{id}'; known: {string.Join(" ", all.Select(x => x.Id.Entry).OrderBy(s => s))}";
                return false;
            }

            ModifierModel mutable = found.ToMutable();
            if (mutable is MegaCrit.Sts2.Core.Models.Modifiers.CharacterCards characterCards)
            {
                // The custom run screen offers one per character, and the
                // modifier throws on use without one.
                if (character == null)
                {
                    malformed = "CHARACTER_CARDS needs a 'character'";
                    return false;
                }

                try
                {
                    characterCards.CharacterModel = Driver.Character(character).Id;
                }
                catch (ArgumentException e)
                {
                    malformed = e.Message;
                    return false;
                }
            }
            else if (character != null)
            {
                malformed = $"'{id}' takes no 'character'";
                return false;
            }

            mods.Add(mutable);
        }

        modifiers = mods.ToArray();
        return true;
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
        Dictionary<string, object?>? rejected = pending.Accept(req);
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
            await Error("choose_option", Refusals.Malformed("choose_option requires an integer 'index'"));
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
                    await Error("choose_option", Refusals.OutOfRange(index, options.Count));
                    return;
                }

                // Checked here because the synchronizer does not: ChooseOption
                // validates the index but never consults IsEnabled, so a
                // disabled option would be taken rather than refused.
                if (!options[index].IsEnabled)
                {
                    await Error("choose_option", Refusals.OptionDisabled());
                    return;
                }

                // Tracked, never awaited here — the same rule the event branch
                // below follows, and for the same reason.
                //
                // ChooseLocalOption awaits the option's OnSelect, and
                // SmithRestSiteOption.OnSelect awaits
                // CardSelectCmd.FromDeckForUpgrade, which parks a question for
                // the client. The serve loop cannot read the answer to that
                // question until this handler returns, so awaiting here waits
                // for something only this handler's return can produce.
                //
                // Measured before the fix: `choose_option` on SMITH never
                // replied and the client timed out at 20s. HEAL was unaffected,
                // which is why the rest site looked fine — one of its two
                // options resolves by itself.
                //
                // WaitForDecisionPoint below handles both halves: for HEAL it
                // waits on BackgroundWork.InFlight until the heal lands, and for
                // SMITH it returns as soon as the selection parks.
                BackgroundWork.Track(sync.ChooseLocalOption(index));
                break;
            }

            case TreasureRoom:
            {
                IReadOnlyList<RelicModel>? relics =
                    RunManager.Instance.TreasureRoomRelicSynchronizer.CurrentRelics;
                if (relics == null || !Chest.Open(_state.Players[0]))
                {
                    await Error("choose_option", Refusals.AlreadyAnswered());
                    return;
                }

                // One past the relics is the skip, which is what the screen's
                // own button does.
                if (index < 0 || index > relics.Count)
                {
                    // One past the relics is the skip, so the count offered is
                    // one more than the relics on the screen.
                    await Error("choose_option", Refusals.OutOfRange(index, relics.Count + 1));
                    return;
                }

                await Chest.Pick(_state, index);
                bool settled = await Driver.WaitForDecisionPoint();
                await Observe(req, "choose_option", settled);
                return;
            }

            case EventRoom:
            {
                EventSynchronizer sync = RunManager.Instance.EventSynchronizer;
                IReadOnlyList<EventOption> options = sync.GetLocalEvent().CurrentOptions;
                if (index < 0 || index >= options.Count)
                {
                    await Error("choose_option", Refusals.OutOfRange(index, options.Count));
                    return;
                }

                if (options[index].IsLocked)
                {
                    await Error("choose_option", Refusals.OptionLocked());
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
                await Error("choose_option", Refusals.NothingToChoose(_state.CurrentRoom));
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
        // A merchant's room, or the Fake Merchant's event, which sells from an
        // inventory of its own.
        MerchantInventory? inventory = _state.CurrentRoom switch
        {
            MerchantRoom shop => shop.GetLocalInventory(),
            EventRoom when RunManager.Instance.EventSynchronizer.GetLocalEvent()
                is FakeMerchant { Inventory: { } fake } => fake,
            _ => null,
        };
        if (inventory == null)
        {
            await Error("buy", Refusals.WrongRoom(RoomType.Shop, _state.CurrentRoom));
            return;
        }

        if (!req.TryGetProperty("index", out JsonElement el) || !el.TryGetInt32(out int index))
        {
            await Error("buy", Refusals.Malformed("buy requires an integer 'index'"));
            return;
        }

        List<MerchantEntry> entries = inventory.AllEntries.ToList();
        if (index < 0 || index >= entries.Count)
        {
            await Error("buy", Refusals.OutOfRange(index, entries.Count));
            return;
        }

        MerchantEntry entry = entries[index];
        Player buyer = _state.Players[0];

        // Every refusal, decided in front of the purchase rather than read off
        // it. `OnTryPurchaseWrapper` makes the first two checks itself before it
        // commits to anything, and `Observation.Blocked` makes the third; all
        // three are the same questions the observation already answers, so a
        // client acting on what it was told still gets told when it is wrong.
        if (!entry.IsStocked || !entry.EnoughGold || Observation.Blocked(entry, buyer) != null)
        {
            await Error("buy", Refusals.Unpurchasable(index, entry, buyer));
            return;
        }

        // Tracked, never awaited -- the same rule `grant` and `choose_option`
        // follow, and for the same reason. A card-removal entry's OnTryPurchase
        // awaits DoMerchantCardRemoval, which awaits
        // CardSelectCmd.FromDeckForRemoval and parks a question for the client.
        // The serve loop cannot read the answer until this handler returns, so
        // awaiting here waits for something only this handler's return can
        // produce.
        //
        // Found by the fuzzer: `buy 13` in a shop -- the card removal entry --
        // never replied and the client timed out at its command deadline. Buying
        // a card, a relic or a potion was unaffected, which is why this looked
        // fine for as long as it did.
        BackgroundWork.Track(Buy(entry, inventory));

        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "buy", resolved);
    }

    /// <summary>
    /// A merchant entry's purchase, and what the shop screen does after it.
    /// </summary>
    /// <remarks>
    /// <c>MerchantCardRemovalEntry.OnTryPurchase</c> marks the removal used
    /// through <c>NRun.Instance?.MerchantRoom</c>, which is null headless. So
    /// without this a shop sold its card removal again. The screen's
    /// <c>NMerchantCardRemoval.OnCardRemovalUsed</c> calls <c>SetUsed</c>, and
    /// so does this.
    /// </remarks>
    private static async Task Buy(MerchantEntry entry, MerchantInventory? inventory)
    {
        bool bought = await entry.OnTryPurchaseWrapper(inventory);
        if (bought && entry is MerchantCardRemovalEntry removal)
        {
            removal.SetUsed();
        }
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
            await Error("travel", Refusals.Malformed("travel requires integer 'col' and 'row'"));
            return;
        }

        if (CombatManager.Instance.IsInProgress)
        {
            await Error("travel", Refusals.InCombat());
            return;
        }

        IReadOnlyList<MapPoint> legal = MapLegality.TravelableFrom(_state);
        MapPoint? target = legal.FirstOrDefault(p => p.coord.col == col && p.coord.row == row);
        if (target == null)
        {
            // The legal set is not repeated here: the observation the client
            // was acting on already reports it, and a refusal restating state
            // is a second copy that can disagree with the first.
            await Error("travel", Refusals.NotTravelable(col, row));
            return;
        }

        await RunManager.Instance.EnterMapCoord(target.coord);
        // Waited on like every other command that enters a room. A fight the
        // room opens can end before the player moves, and the rewards it owes
        // are offered here or by nothing.
        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "travel", resolved);
    }

    /// <summary>
    /// Leaves a finished act for the next one.
    /// </summary>
    /// <remarks>
    /// The one move that is not a map move. An act's boss sits outside the grid
    /// with no children, so once it is beaten <c>MapLegality</c> correctly reports
    /// nowhere to go — and without this the run simply stops there. Random play
    /// found it: every run stalled on floor 17 with the boss dead and nothing
    /// legal.
    ///
    /// The rule is <c>NRewardsScreen.OnProceedButtonPressed</c>'s, which is the
    /// only place the game decides it. The first of two bosses leads to the
    /// second by travel instead, which <see cref="MapLegality.CanProceed"/>
    /// says.
    ///
    /// Deliberately not a room option. It is available exactly when a boss room
    /// is finished, which is a property of the run rather than a list the room
    /// is offering, and folding it into <c>choose_option</c> would mean inventing
    /// an option the game does not have.
    /// </remarks>
    private static async Task Proceed(JsonElement req)
    {
        if (CombatManager.Instance.IsInProgress)
        {
            await Error("proceed", Refusals.InCombat());
            return;
        }

        if (!MapLegality.CanProceed(_state))
        {
            await Error("proceed", Refusals.CannotProceed());
            return;
        }

        if (MapLegality.ResumesEvent(_state))
        {
            await RunManager.Instance.ProceedFromTerminalRewardsScreen();
        }
        else
        {
            RunManager.Instance.ActChangeSynchronizer.SetLocalPlayerReady();
        }

        bool resolved = await Driver.WaitForDecisionPoint();
        await Observe(req, "proceed", resolved);
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
            await Error("play_card", Refusals.NotInCombat());
            return;
        }

        if (!req.TryGetProperty("index", out JsonElement idxEl) || !idxEl.TryGetInt32(out int index))
        {
            await Error("play_card", Refusals.Malformed("play_card requires an integer 'index'"));
            return;
        }

        if (index < 0 || index >= pcs.Hand.Cards.Count)
        {
            await Error("play_card", Refusals.OutOfRange(index, pcs.Hand.Cards.Count));
            return;
        }

        CardModel card = pcs.Hand.Cards[index];

        Creature? target = null;
        if (req.TryGetProperty("target", out JsonElement tEl) && tEl.ValueKind != JsonValueKind.Null)
        {
            if (!tEl.TryGetUInt32(out uint targetId))
            {
                await Error("play_card", Refusals.Malformed("'target' must be a creature id"));
                return;
            }

            target = combat.Creatures.FirstOrDefault(c => c.CombatId == targetId);
            if (target == null)
            {
                await Error("play_card", Refusals.NoSuchCreature(targetId));
                return;
            }
        }

        // Refuse rather than enqueue. The game would reject it too, but further
        // in and less legibly, and H2 exists so the client can be held to the
        // legal set the oracle publishes.
        if (!card.CanPlay(out UnplayableReason reason, out AbstractModel? preventer))
        {
            await Error("play_card", Refusals.Unplayable(card, reason, preventer));
            return;
        }

        if (!card.CanPlayTargeting(target))
        {
            await Error("play_card", Refusals.WrongTarget(card, target?.CombatId));
            return;
        }

        var action = new PlayCardAction(card, target);
        RunManager.Instance.ActionQueueSet.EnqueueWithoutSynchronizing(action);
        bool resolved = await Driver.Quiesce(action);
        await Observe(req, "play_card", resolved);
    }

    /// <summary>
    /// Drinks one potion from the belt, addressed by its index in the
    /// observation.
    /// </summary>
    /// <remarks>
    /// Addressed positionally for the same reason a card is: that is how the
    /// observation reports the belt, and it is how two identical potions are
    /// told apart at all. Targets are <c>CombatId</c>s, as a card's are.
    ///
    /// The body mirrors <c>PotionModel.EnqueueManualUse</c> rather than calling
    /// it. Two differences, both deliberate: the enqueue goes through
    /// <c>ActionQueueSet</c> the way every other command here does, instead of
    /// the multiplayer synchronizer; and the refusals are decided in front of
    /// the action rather than thrown from inside its constructor. What is
    /// mirrored exactly is the target defaulting to the drinker where that is a
    /// valid target, because a self-targeted potion is drunk with no argument.
    /// </remarks>
    private static async Task UsePotion(JsonElement req)
    {
        Player me = _state.Players[0];

        if (!req.TryGetProperty("index", out JsonElement idxEl) || !idxEl.TryGetInt32(out int index))
        {
            await Error("use_potion", Refusals.Malformed("use_potion requires an integer 'index'"));
            return;
        }

        // The belt is an `IEnumerable`, and the observation reports it by
        // walking it — so an index means a position in this walk and nothing
        // else. `GetPotionSlotIndex` counts empty slots too and is a different
        // number; `UsePotionAction` works it out from the model itself.
        List<PotionModel> belt = me.Potions.ToList();
        if (index < 0 || index >= belt.Count)
        {
            await Error("use_potion", Refusals.OutOfRange(index, belt.Count));
            return;
        }

        PotionModel potion = belt[index];

        Creature? target = null;
        if (req.TryGetProperty("target", out JsonElement tEl) && tEl.ValueKind != JsonValueKind.Null)
        {
            if (!tEl.TryGetUInt32(out uint targetId))
            {
                await Error("use_potion", Refusals.Malformed("'target' must be a creature id"));
                return;
            }

            CombatState? combat = (_state.CurrentRoom as CombatRoom)?.CombatState;
            target = combat?.Creatures.FirstOrDefault(c => c.CombatId == targetId);
            if (target == null)
            {
                await Error("use_potion", Refusals.NoSuchCreature(targetId));
                return;
            }
        }

        if (Observation.Undrinkable(potion, me) is { } why)
        {
            await Error("use_potion", Refusals.Undrinkable(potion, why));
            return;
        }

        // `EnqueueManualUse` does this, and a self-targeted potion relies on
        // it: the belt reports no targets outside combat, so `target` is null
        // and the drinker is who it lands on.
        if (target == null && potion.IsValidTarget(me.Creature))
        {
            target = me.Creature;
        }

        if (!potion.IsValidTarget(target))
        {
            await Error("use_potion", Refusals.WrongPotionTarget(potion, target?.CombatId));
            return;
        }

        var action = new UsePotionAction(potion, target, CombatManager.Instance.IsInProgress);
        RunManager.Instance.ActionQueueSet.EnqueueWithoutSynchronizing(action);
        bool resolved = await Driver.Quiesce(action);
        await Observe(req, "use_potion", resolved);
    }

    /// <summary>Discards the potion at a belt index, as the potion's popup does.</summary>
    /// <remarks>
    /// <c>NPotionPopup.OnDiscardButtonPressed</c> enqueues a
    /// <c>DiscardPotionGameAction</c>, which runs <c>PotionCmd.Discard</c> and
    /// <c>Hook.AfterPotionDiscarded</c>. The popup's button is disabled for a
    /// potion already queued, or an owner who is dead, and so is this. The
    /// index is into the belt as an observation reports it, which skips empty
    /// slots. The action takes the slot.
    /// </remarks>
    private static async Task DiscardPotion(JsonElement req)
    {
        Player me = _state.Players[0];

        if (!req.TryGetProperty("index", out JsonElement idxEl) || !idxEl.TryGetInt32(out int index))
        {
            await Error("discard_potion", Refusals.Malformed("discard_potion requires an integer 'index'"));
            return;
        }

        List<PotionModel> belt = me.Potions.ToList();
        if (index < 0 || index >= belt.Count)
        {
            await Error("discard_potion", Refusals.OutOfRange(index, belt.Count));
            return;
        }

        PotionModel potion = belt[index];
        if (Moves.Discardable(potion, me) is { } why)
        {
            await Error("discard_potion", Refusals.Undrinkable(potion, why));
            return;
        }

        int slot = me.PotionSlots.ToList().IndexOf(potion);
        var action = new DiscardPotionGameAction(me, (uint)slot, CombatManager.Instance.IsInProgress);
        RunManager.Instance.ActionQueueSet.EnqueueWithoutSynchronizing(action);
        bool resolved = await Driver.Quiesce(action);
        await Observe(req, "discard_potion", resolved);
    }

    private static async Task EndTurn(JsonElement req)
    {
        Player me = _state.Players[0];
        PlayerCombatState? pcs = me.PlayerCombatState;
        if (pcs == null || !CombatManager.Instance.IsInProgress)
        {
            await Error("end_turn", Refusals.NotInCombat());
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
    internal static async Task Observe(JsonElement req, string cmd, bool resolved = true)
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

    /// <summary>Refuses a command, saying why as a tagged reason.</summary>
    /// <remarks>
    /// No <c>message</c> beside the reason. The prose is the client's
    /// <c>Display</c>, written once from the data, so the two cannot drift --
    /// and the log line below serialises the same object, which is as readable
    /// as the sentence was and has one source. See <see cref="Refusals"/>.
    /// </remarks>
    internal static async Task Error(string cmd, Dictionary<string, object?> reason)
    {
        Log.Warn($"[headless] rejecting {cmd}: {JsonSerializer.Serialize(reason, Json)}");
        await Send(new Dictionary<string, object?>
        {
            ["type"] = "error",
            ["cmd"] = cmd,
            ["reason"] = reason,
        });
    }

    /// <summary>The adapter broke, which is not the game deciding anything.</summary>
    /// <remarks>
    /// Its own message type because it is a different kind of answer. An
    /// exception escaping a handler used to be sent as a refusal, which filed an
    /// adapter crash as a game decision -- a client comparing refusals would
    /// have counted a bug in this mod as the game saying no. A fault is the
    /// outer failure: the session is suspect and the run is where the exception
    /// left it.
    /// </remarks>
    private static async Task Fault(string cmd, Exception e)
    {
        Log.Error($"[headless] {cmd} failed: {e}");
        await Send(new Dictionary<string, object?>
        {
            ["type"] = "fault",
            ["cmd"] = cmd,
            ["error"] = e.GetType().Name,
            ["message"] = e.Message,
        });
    }

    private static async Task Send(Dictionary<string, object?> msg) =>
        await _channel.WriteLine(JsonSerializer.Serialize(msg, Json));

    internal static string? Str(JsonElement req, string name) =>
        req.TryGetProperty(name, out JsonElement el) && el.ValueKind == JsonValueKind.String
            ? el.GetString()
            : null;
}
