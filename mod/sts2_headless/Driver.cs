using System;
using MegaCrit.Sts2.Core.Rewards;
using HarmonyLib;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.DevConsole;
using MegaCrit.Sts2.Core.DevConsole.ConsoleCommands;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Reflection;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Assets;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Multiplayer;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models.Encounters;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Characters;
using MegaCrit.Sts2.Core.Multiplayer;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Nodes;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Unlocks;

using Sts2Determinism;

namespace Sts2Headless;

/// <summary>
/// Starts a run at the model layer and reports what the game does, bypassing
/// the main menu and every screen behind it.
///
/// This is the first vertical slice of the oracle driver. It exists to answer
/// the questions that reading the decompiled source cannot: whether the startup
/// sequence survives in a real engine with no run UI, whether Godot's loop
/// drives our continuations without a hand-pumped SynchronizationContext, and
/// what a step costs. It does not yet accept commands or emit observations.
/// </summary>
public static class Driver
{
    private static readonly Stopwatch Clock = new Stopwatch();
    private static int _actions;
    private static int _checksums;

    /// <summary>
    /// Runs the whole slice, logging each stage so a failure names its own step.
    /// </summary>
    /// <remarks>
    /// Deliberately not started from <see cref="Headless.Init"/> directly: mod
    /// init runs inside <c>NGame</c> startup, well before the model database is
    /// populated, which is what made the earlier AutoSlay attempt fault.
    /// </remarks>
    /// <summary>
    /// Brings a run up, hands it to <paramref name="body"/>, and ends the
    /// process when that returns.
    /// </summary>
    /// <remarks>
    /// Every consumer goes through here rather than assembling the lifecycle
    /// itself, which is what makes it possible for one to live in another mod:
    /// the selftest suite and the protocol server differ only in the body.
    ///
    /// It also means the reproducibility check happens once, in the one place
    /// every driver passes through, rather than in each entry point that might
    /// forget it.
    /// </remarks>
    public static async Task Run(Func<RunState, Task> body)
    {
        Clock.Start();
        try
        {
            // Before waiting, so anything the game logs while loading lands in
            // the first observation rather than nowhere.
            GameErrors.Watch();
            await WaitUntilLoaded();

            // Loading first is not the same as having succeeded: a build
            // mismatch leaves the determinism mod loaded, inert, and the game
            // untouched. Driving a game nobody made reproducible is worse than
            // not driving it.
            //
            // This is checked here rather than before the wait because refusing
            // has to end the process, and the only thing that can end it is
            // Quit -- which needs a scene tree, which is what the wait is for.
            // Environment.Exit does not work: it is called on Godot's main loop
            // and never completes, so an earlier version of this logged
            // "Exiting." and then hung until something external killed it.
            if (!Determinism.Active)
            {
                Log.Error(
                    "[headless] refusing to start: sts2_determinism loaded but did not activate, "
                    + "so the game has not been made reproducible");
                ExitCode = 1;
                return;
            }

            // Everything below can throw, and all of it belongs inside this try.
            // UseSelector throws outright if a selector is already installed, and
            // PatchAll throws when Harmony cannot patch -- and Run is started
            // with `_ = Driver.Run(...)`, so an escaping exception becomes an
            // unobserved faulted task and the process hangs with no message.
            new Harmony("sts2_headless").PatchAll(Assembly.GetExecutingAssembly());

            // Once per process, and here rather than in whichever body happens
            // to run. These used to sit inside Protocol.Serve, so selftest --
            // which runs instead of it -- drove a differently configured game
            // than the one it was testing.
            //
            // CardSelectCmd.Reset leaves the stack alone while TestMode is on,
            // so this survives every Restart. With TestMode on and no
            // testSelector, RewardsSet.Offer takes *every* reward itself -- so
            // installing one does not add a decision point, it takes one away
            // from the game and gives it to the client.
            CardSelectCmd.UseSelector(new HeadlessCardSelector());
            RewardsSet.testSelector = RewardChoice.TakeRewards;
            WatchForWins();

            RunState state = BuildRunState(_seed, Array.Empty<ModifierModel>(), Character(null), ascension: 0);
            await StartRun(state);
            Report(state);

            await body(state);
        }
        catch (Exception e)
        {
            // A driver that half-works is worse than one that stops: every
            // silent degradation turns a simulator bug into a passing test. And
            // it must exit non-zero, or `just selftest` reports success for a
            // run that crashed before asserting anything.
            Log.Error($"[headless] driver failed at {Stage}: {e}");
            ExitCode = 1;
        }
        finally
        {
            Quit();
        }
    }

    /// <summary>
    /// Ends the process once the driver is done.
    /// </summary>
    /// <remarks>
    /// Without this the game returns to its main loop and runs forever: the
    /// driver's work took two seconds while the process stayed up until an
    /// external timeout, and SIGTERM is not reliably fatal — instances survived
    /// <c>pkill</c> for thirteen hours and needed SIGKILL. Quitting through the
    /// scene tree is how the game itself exits (<c>NGame</c> calls
    /// <c>GetTree().Quit(0)</c>), so shutdown runs its normal course.
    /// </remarks>
    /// <summary>
    /// What the process should exit with. Set it before returning from a body
    /// passed to <see cref="Run"/>.
    /// </summary>
    /// <remarks>
    /// A body that wants to fail the process sets this rather than calling
    /// <c>Environment.Exit</c>. Exiting from inside the body hangs: the driver
    /// runs on Godot's main thread, and tearing the runtime down from under it
    /// does not complete.
    /// </remarks>
    public static int ExitCode { get; set; }

    private static void Quit()
    {
        Log.Info($"[headless] quitting after {Clock.Elapsed.TotalSeconds:F1}s with {ExitCode}");

        // Nothing else can stop this process. Environment.Exit does not work
        // from Godot's main loop, so if there is no tree to quit there is no
        // way out and the only useful thing left is to say so loudly.
        if (NGame.Instance == null)
        {
            Log.Error("[headless] no scene tree to quit through; this process will not exit on its own");
            return;
        }

        NGame.Instance.GetTree().Quit(ExitCode);
    }

    private static string Stage = "start";

    /// <summary>The seed the current process was started at, for restarts.</summary>
    private static string _seed = "ORACLE1";

    public static readonly bool FullObservations =
        Environment.GetEnvironmentVariable("STS2_HEADLESS_OBS") == "full";

    /// <summary>
    /// Waits for the model database, which loads asynchronously during startup.
    /// </summary>
    /// <remarks>
    /// The awaits here are the experiment: if these continuations resume at all,
    /// Godot's main loop is driving them, and the driver needs no
    /// SynchronizationContext of its own the way an engineless harness does.
    ///
    /// The probe must be <c>ModelDb.Contains</c>, which is a plain dictionary
    /// lookup. The collection properties are memoized — <c>AllRelics</c> is
    /// <c>_allRelics ??= …</c> — so touching one before the game has registered
    /// its content does not merely fail early, it can cache a partial answer for
    /// the process lifetime. An earlier probe on <c>AllRelics</c> threw
    /// <c>KeyNotFoundException</c> for <c>CHARACTER.IRONCLAD</c> for this reason.
    /// </remarks>
    private static async Task WaitUntilLoaded()
    {
        Stage = "waiting for game load";
        for (int i = 0; i < 600; i++)
        {
            // Yield before the first probe: Init is called from inside NGame
            // startup, so the loop body would otherwise run there.
            await Task.Delay(50);

            if (NGame.Instance != null && ModelDb.Contains(typeof(Ironclad)))
            {
                Log.Info($"[headless] game loaded after {Clock.Elapsed.TotalSeconds:F1}s");
                return;
            }
        }

        throw new TimeoutException("model database never populated");
    }

    /// <summary>
    /// Builds a fresh run state at an explicit seed.
    /// </summary>
    /// <remarks>
    /// <c>CreateForNewRun</c> rather than <c>CreateForTest</c>: the latter takes
    /// a nullable seed and silently falls back to <c>SeedHelper.GetRandomSeed</c>,
    /// which draws from the wall-clock <c>Rng.Chaotic</c>, and it defaults its
    /// player to the unplayable <c>Deprived</c>.
    ///
    /// Acts are cloned to mutable because the RunState constructor asserts it;
    /// only <c>CreateForTest</c> does that mapping for you.
    /// </remarks>
    private static RunState BuildRunState(
        string seed, IReadOnlyList<ModifierModel> modifiers, CharacterModel character, int ascension)
    {
        Stage = "building run state";

        // UnlockState.all so no scenario is unreachable because a profile has
        // not earned the content yet — D1 wants a run that does not depend on
        // which machine's save file it started from.
        var players = new List<Player> { Player.CreateForNewRun(character, UnlockState.all, 1uL) };
        List<ActModel> acts = ActModel.GetDefaultList().Select(a => a.ToMutable()).ToList();

        RunState state = RunState.CreateForNewRun(
            players, acts, modifiers, GameMode.Standard, ascension, seed);

        Log.Info($"[headless] run state built: seed={seed} character={character.Id.Entry} "
            + $"ascension={ascension} deck={players[0].Deck.Cards.Count} acts={acts.Count} "
            + $"modifiers=[{string.Join(",", modifiers.Select(m => m.Id.Entry))}]");
        return state;
    }

    /// <summary>The character a run starts with, by id, or the default.</summary>
    /// <remarks>
    /// Named rather than hardcoded because four of the five playable characters
    /// were unreachable, and `Defect` is the only way orbs are ever exercised —
    /// an observation gap H4 could not close without this.
    /// </remarks>
    public static CharacterModel Character(string? id)
    {
        if (id == null)
        {
            return ModelDb.Character<Ironclad>();
        }

        CharacterModel? found = ModelDb.AllCharacters.FirstOrDefault(
            c => string.Equals(c.Id.Entry, id, StringComparison.OrdinalIgnoreCase));

        if (found == null)
        {
            throw new ArgumentException(
                $"no such character '{id}'; known: "
                + string.Join(" ", ModelDb.AllCharacters.Select(c => c.Id.Entry)));
        }

        return found;
    }

    /// <summary>
    /// Tears the current run down and starts another in the same process.
    /// </summary>
    /// <remarks>
    /// Needed because some rules are fixed when the run is created and cannot be
    /// granted to a run in progress. Run modifiers are the case in hand: the
    /// `Flight` modifier is only ever installed by
    /// <c>RunManager.InitializeNewRun</c> iterating <c>State.Modifiers</c>, so a
    /// scenario that tests it has to construct the run differently rather than
    /// mutate the one already running.
    ///
    /// <c>RunState.AddModifierDebug</c> looks like a shortcut and was rejected:
    /// it appends to the list without calling <c>OnRunCreated</c>, leaving the
    /// modifier's <c>RunState</c> unset and its <c>ClearsPlayerDeck</c> pass
    /// skipped. `Flight` happens to survive that, and relying on the happenstance
    /// would make the case a poor template for the modifiers that do not.
    ///
    /// <c>SetUpTest</c> refuses to run twice — it throws on a non-null
    /// <c>State</c> — so <c>CleanUp</c> must come first; that is what nulls it.
    /// This is also the only exercise the oracle's multi-scenario claim gets: if
    /// the second run reproduces the first's floor, deck and RNG counters, no
    /// state leaked across the boundary.
    /// </remarks>
    public static Task<RunState> Restart(params ModifierModel[] modifiers) =>
        Restart(null, null, 0, modifiers);

    /// <param name="seed">
    /// The seed for the new run, or null to keep the one this process started
    /// at. Never defaulted to a fresh random value: H1 exists because
    /// <c>CreateForTest</c> silently falls back to the wall clock.
    /// </param>
    public static async Task<RunState> Restart(
        string? seed, string? character, int ascension, params ModifierModel[] modifiers)
    {
        Stage = "restarting";
        Log.Info("[headless] restarting run");

        // Before CleanUp, or the game is left awaiting a task nobody completes.
        Pending.CancelOutstanding("the run was restarted");
        RunManager.Instance.CleanUp();

        _seed = seed ?? _seed;
        RunState state = BuildRunState(_seed, modifiers, Character(character), ascension);
        await StartRun(state);
        Report(state);
        return state;
    }

    /// <summary>
    /// Drives the game's own test entry point through to standing on the map.
    /// </summary>
    private static async Task StartRun(RunState state)
    {
        Stage = "SetUpTest";
        RunManager run = RunManager.Instance;
        run.SetUpTest(state, new NetSingleplayerGameService());

        // InitializeShared computes IsEnabled as
        //   !TestMode.IsOn && NetService.Type is Host/Client/Replay
        // so a singleplayer service disables checksums, and TestMode disables
        // them again. Both are silent, and we want the game's own divergence
        // detector, so turn it back on.
        run.ChecksumTracker.IsEnabled = true;
        Subscribe(run);

        Stage = "GenerateRooms";
        run.GenerateRooms();

        // The rest mirrors NGame.StartRun, minus the run scene.
        Stage = "LoadRunAssets";
        await PreloadManager.LoadRunAssets(state.Players.Select(p => p.Character));

        Stage = "LoadActAssets";
        await PreloadManager.LoadActAssets(state.Acts[0]);

        Stage = "FinalizeStartingRelics";
        await run.FinalizeStartingRelics();

        Stage = "Launch";
        run.Launch();

        // No run scene, deliberately. Instantiating res://scenes/run.tscn
        // segfaults under --headless, which forces Godot's dummy rendering
        // device regardless of --rendering-driver. Without it the game logs
        // "Missing screenShake target!" where the model layer calls presentation
        // inline, but those are caught and combat runs real turns to completion.
        Stage = "EnterAct";
        await run.EnterAct(0, doTransition: false);

        Stage = "running";
        Log.Info($"[headless] run started after {Clock.Elapsed.TotalSeconds:F1}s");
    }

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
        Stage = "entering fight";
        CmdResult result = new FightConsoleCmd().Process(state.Players[0], new[] { encounter });
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
    /// Plays one card through the real action queue, for tests that need a
    /// card to have been played this turn.
    /// </summary>
    /// <remarks>
    /// Several of the game's <c>ShouldPlay</c> hooks — `Chains of Binding`,
    /// `Ringing`, `Sloth`, `Normality`, `Velvet Choker` — only forbid a card
    /// *after* one has already been played this turn, so a scenario that merely
    /// applies the power proves nothing about them.
    /// </remarks>
    public static async Task<bool> PlayOneCard(RunState state, Func<CardModel, bool>? pick = null)
    {
        Player me = state.Players[0];
        PlayerCombatState? pcs = me.PlayerCombatState;
        CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
        if (pcs == null || combat == null)
        {
            return false;
        }

        Creature? target = combat.Creatures.FirstOrDefault(c => !c.IsPlayer && c.IsAlive);
        CardModel? card = pcs.Hand.Cards.FirstOrDefault(c =>
            (pick == null || pick(c)) && (c.CanPlayTargeting(target) || c.CanPlayTargeting(null)));

        if (card == null)
        {
            return false;
        }

        var action = new PlayCardAction(card, card.TargetType == TargetType.AnyEnemy ? target : null);
        RunManager.Instance.ActionQueueSet.EnqueueWithoutSynchronizing(action);
        await Quiesce(action);
        return true;
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
    public static async Task<bool> Quiesce(GameAction issued, int? turnMustAdvancePast = null)
    {
        ActionQueueSet queue = RunManager.Instance.ActionQueueSet;
        ActionExecutor executor = RunManager.Instance.ActionExecutor;

        for (int i = 0; !issued.CompletionTask.IsCompleted; i++)
        {
            // Checked before anything else, and in both loops: a parked action
            // holds the queue open, so every drained-based test below is false
            // by construction while the game is waiting on us. This is also what
            // keeps the bounded waits honest — they exist to catch a wedged
            // engine, and must never be counting down while a client thinks.
            if (Pending.Current != null)
            {
                return true;
            }

            // A cancelled action never completes. When combat ends mid-cascade
            // the executor stops with actions still queued, and the decision we
            // issued is abandoned rather than finished — observed as
            // "EndPlayerTurnAction turn 5 not completed: running=False
            // current=<null> empty=False combat=False". Waiting on its
            // CompletionTask alone hangs forever, so treat the fight ending as
            // a legitimate reason for our decision never to resolve.
            if (!CombatManager.Instance.IsInProgress)
            {
                Log.Info($"[headless] {issued} abandoned: combat ended");
                return false;
            }

            if (i > 15000)
            {
                throw new TimeoutException($"{issued} never completed");
            }

            await Task.Delay(1);
        }

        return await WaitForDecisionPoint(turnMustAdvancePast, issued);
    }

    /// <summary>
    /// Waits until the game either wants a decision or is asking a question.
    /// </summary>
    /// <remarks>
    /// Separate from <see cref="Quiesce"/> because answering a pending resumes
    /// an action that is *already running* — there is no newly issued action to
    /// wait on, only the same cascade continuing. Both paths end here, which is
    /// what lets a question that leads to another question need no special case.
    /// </remarks>
    /// <summary>The room whose rewards have not been offered yet, if any.</summary>
    private static CombatRoom? _owedRewards;

    /// <summary>
    /// Starts offering combat rewards, which nothing else will.
    /// </summary>
    /// <remarks>
    /// <c>CombatRoom.OfferRoomEndRewards</c> is public and does the whole job,
    /// but on the live path its only caller is <c>NCombatUi.OnCombatWon</c> —
    /// the *trigger* is in the UI, not the model, which is why a driven run
    /// finished fights with gold and deck untouched. Rest sites, events and
    /// shops are not like this; they begin from their room's
    /// <c>EnterInternal</c>. Rewards are the exception.
    ///
    /// Subscribed once per process rather than per run: <c>CombatManager</c> is
    /// a singleton that outlives <c>CleanUp</c>, so re-subscribing on each
    /// restart would offer every reward set as many times as the process had
    /// started runs.
    /// </remarks>
    public static void WatchForWins()
    {
        CombatManager.Instance.CombatWon += room => _owedRewards = room;
    }

    public static async Task<bool> WaitForDecisionPoint(
        int? turnMustAdvancePast = null, GameAction? issued = null)
    {
        while (true)
        {
            bool resolved = await WaitOnce(turnMustAdvancePast, issued);

            // A question outranks owed rewards: the game is mid-action and the
            // reward offer, when it comes, will park in its turn.
            if (Pending.Current != null)
            {
                return resolved;
            }

            CombatRoom? owed = _owedRewards;
            if (owed == null)
            {
                return resolved;
            }

            _owedRewards = null;
            Stage = "offering rewards";

            // Returns as soon as the offer is *started* — the game fires
            // RewardsSet.Offer without awaiting it — so the loop goes round and
            // waits again, which is where the reward question shows up.
            await owed.OfferRoomEndRewards();
        }
    }

    private static async Task<bool> WaitOnce(
        int? turnMustAdvancePast = null, GameAction? issued = null)
    {
        ActionQueueSet queue = RunManager.Instance.ActionQueueSet;
        ActionExecutor executor = RunManager.Instance.ActionExecutor;

        for (int i = 0; i < 20000; i++)
        {
            if (Pending.Current != null)
            {
                return true;
            }

            if (queue.IsEmpty && !executor.IsRunning && ReadyForInput(turnMustAdvancePast))
            {
                return true;
            }

            // Say what we are stuck behind rather than just hanging. A wedged
            // driver and a slow one look identical from outside the process.
            if (i > 0 && i % 2000 == 0)
            {
                Log.Info($"[headless] still waiting after {i}ms: empty={queue.IsEmpty} "
                    + $"running={executor.IsRunning} current={executor.CurrentlyRunningAction} "
                    + $"paused={executor.IsPaused} combat={CombatManager.Instance.IsInProgress} "
                    + $"actionsDisabled={CombatManager.Instance.PlayerActionsDisabled} "
                    + $"phase={RunManager.Instance.DebugOnlyGetState()?.Players[0].PlayerCombatState?.Phase}");
            }

            await Task.Delay(1);
        }

        throw new TimeoutException(
            $"action queue never drained{(issued == null ? "" : $" after {issued}")}");
    }

    /// <summary>
    /// Whether the game is back to wanting a decision from the player.
    /// </summary>
    /// <remarks>
    /// A fight that has ended counts as ready: the decision resolved, there is
    /// simply no next one in this room, and the caller reads combat state as
    /// absent rather than waiting for a turn that will never start.
    /// </remarks>
    private static bool ReadyForInput(int? turnMustAdvancePast)
    {
        // Work started outside the action queue — taking rewards, resolving an
        // event option — is invisible to every other test here, and out of
        // combat they are all trivially true.
        if (BackgroundWork.InFlight)
        {
            return false;
        }

        if (!CombatManager.Instance.IsInProgress)
        {
            return true;
        }

        PlayerCombatState? pcs = RunManager.Instance.DebugOnlyGetState()?.Players[0].PlayerCombatState;
        if (pcs == null)
        {
            return true;
        }

        if (CombatManager.Instance.PlayerActionsDisabled || pcs.Phase != PlayerTurnPhase.Play)
        {
            return false;
        }

        return turnMustAdvancePast == null || pcs.TurnNumber > turnMustAdvancePast;
    }

    private static void Subscribe(RunManager run)
    {
        run.ActionExecutor.AfterActionExecuted += _ => _actions++;
        run.ChecksumTracker.ChecksumGenerated += (_, context, _) =>
        {
            _checksums++;
            if (_checksums <= 5)
            {
                Log.Info($"[headless] checksum: {context}");
            }
        };
        run.RoomEntered += () => Log.Info("[headless] room entered");
        run.RoomExited += () => Log.Info("[headless] room exited");
        run.ActEntered += () => Log.Info("[headless] act entered");
    }

    /// <summary>
    /// Dumps enough of the reached state to tell a real run from a hollow one.
    /// </summary>
    private static void Report(RunState state)
    {
        Player me = state.Players[0];
        Log.Info($"[headless] REACHED act={state.CurrentActIndex} floor={state.TotalFloor} "
            + $"hp={me.Creature.CurrentHp}/{me.Creature.MaxHp} gold={me.Gold} "
            + $"relics=[{string.Join(",", me.Relics.Select(r => r.Id.Entry))}] "
            + $"deck={me.Deck.Cards.Count} combat={CombatManager.Instance.IsInProgress}");
        // ToSerializable is the only public way to read every stream at once;
        // RunRngSet exposes the individual streams but no indexer.
        var counters = state.Rng.ToSerializable().Counters;
        Log.Info($"[headless] counters: {string.Join(" ", counters.OrderBy(c => c.Key).Select(c => $"{c.Key}={c.Value}"))}");
        Log.Info($"[headless] actions={_actions} checksums={_checksums} elapsed={Clock.Elapsed.TotalSeconds:F1}s");
    }
}
