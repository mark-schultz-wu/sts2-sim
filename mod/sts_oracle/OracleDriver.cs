using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
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

namespace StsOracle;

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
internal static class OracleDriver
{
    private static readonly Stopwatch Clock = new Stopwatch();
    private static int _actions;
    private static int _checksums;

    /// <summary>
    /// Runs the whole slice, logging each stage so a failure names its own step.
    /// </summary>
    /// <remarks>
    /// Deliberately not started from <see cref="Oracle.Init"/> directly: mod
    /// init runs inside <c>NGame</c> startup, well before the model database is
    /// populated, which is what made the earlier AutoSlay attempt fault.
    /// </remarks>
    public static async Task Start(string seed)
    {
        Clock.Start();
        try
        {
            await WaitUntilLoaded();
            RunState state = BuildRunState(seed);
            await StartRun(state);
            Report(state);

            if (Environment.GetEnvironmentVariable("STS_ORACLE_MODE") == "selftest")
            {
                await SelfTest.Run(state, () => EnterFight(state));
            }
            else
            {
                await PlayOneFight(state);
                Report(state);
            }
        }
        catch (Exception e)
        {
            // A driver that half-works is worse than one that stops: every
            // silent degradation turns a simulator bug into a passing test.
            Log.Error($"[oracle] driver failed at {Stage}: {e}");
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
    ///
    /// Set <c>STS_ORACLE_QUIT=0</c> to keep the process up for inspection.
    /// </remarks>
    private static void Quit()
    {
        if (Environment.GetEnvironmentVariable("STS_ORACLE_QUIT") == "0")
        {
            Log.Info("[oracle] STS_ORACLE_QUIT=0, staying up");
            return;
        }

        Log.Info($"[oracle] quitting after {Clock.Elapsed.TotalSeconds:F1}s");
        NGame.Instance.GetTree().Quit(0);
    }

    private static string Stage = "start";

    private static readonly bool FullObservations =
        Environment.GetEnvironmentVariable("STS_ORACLE_OBS") == "full";

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
                Log.Info($"[oracle] game loaded after {Clock.Elapsed.TotalSeconds:F1}s");
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
    private static RunState BuildRunState(string seed)
    {
        Stage = "building run state";
        var players = new List<Player> { Player.CreateForNewRun<Ironclad>(UnlockState.all, 1uL) };
        List<ActModel> acts = ActModel.GetDefaultList().Select(a => a.ToMutable()).ToList();

        RunState state = RunState.CreateForNewRun(
            players, acts, Array.Empty<ModifierModel>(), GameMode.Standard, ascensionLevel: 0, seed);

        Log.Info($"[oracle] run state built: seed={seed} character=Ironclad "
            + $"deck={players[0].Deck.Cards.Count} acts={acts.Count}");
        return state;
    }

    /// <summary>
    /// Drives the game's own test entry point through to standing on the map.
    /// </summary>
    private static async Task StartRun(RunState state)
    {
        // First-time-user prompts both gate progress and write to disk: a driven
        // fight logged "Player has seen ftue shuffle_ftue!" and then wrote
        // progress.save. R13 wants the oracle to leave no trace, and an oracle
        // whose behaviour depends on which tutorials a profile has already seen
        // is not reproducible either.
        Stage = "disabling FTUEs";
        SaveManager.Instance.SetFtuesEnabled(enabled: false);

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
        Log.Info($"[oracle] run started after {Clock.Elapsed.TotalSeconds:F1}s");
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
    private static async Task EnterFight(RunState state)
    {
        Stage = "entering fight";
        EncounterModel encounter = ModelDb.Encounter<BowlbugsWeak>().ToMutable();
        await RunManager.Instance.EnterRoomDebug(RoomType.Monster, MapPointType.Unassigned, encounter);
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

    private static async Task PlayOneFight(RunState state)
    {
        await EnterFight(state);

        Player me = state.Players[0];
        Log.Info($"[oracle] in fight: combat={CombatManager.Instance.IsInProgress} "
            + $"enemies={(state.CurrentRoom as CombatRoom)?.CombatState.Creatures.Count(c => !c.IsPlayer) ?? -1}");

        Stage = "playing fight";
        var clock = Stopwatch.StartNew();
        int decisions = 0;

        while (CombatManager.Instance.IsInProgress && decisions < 300)
        {
            PlayerCombatState? pcs = me.PlayerCombatState;
            CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
            if (pcs == null || combat == null)
            {
                Log.Info("[oracle] no combat state; stopping");
                break;
            }

            Creature? target = combat.Creatures.FirstOrDefault(c => !c.IsPlayer && c.IsAlive);
            CardModel? card = pcs.Hand.Cards.FirstOrDefault(c => c.CanPlayTargeting(target))
                ?? pcs.Hand.Cards.FirstOrDefault(c => c.CanPlayTargeting(null));

            GameAction action = card != null
                ? new PlayCardAction(card, card.TargetType == TargetType.AnyEnemy ? target : null)
                : new EndPlayerTurnAction(me, pcs.TurnNumber);

            RunManager.Instance.ActionQueueSet.EnqueueWithoutSynchronizing(action);
            decisions++;
            bool resolved = await Quiesce(action);
            // Digest by default; the full observation is ~1.6KB a step and is
            // only needed for the step that disagrees, which R1 lets us
            // reproduce on demand.
            Log.Info(FullObservations
                ? $"[obs] {Observation.Capture(state)}"
                : $"[obs] {Observation.Digest(state)}");
            if (!resolved)
            {
                break;
            }
        }

        double s = clock.Elapsed.TotalSeconds;
        Log.Info($"[oracle] fight over: decisions={decisions} in {s:F2}s "
            + $"({decisions / Math.Max(s, 0.001):F1}/s) combat={CombatManager.Instance.IsInProgress}");
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
    /// </remarks>
    private static async Task<bool> Quiesce(GameAction issued)
    {
        ActionQueueSet queue = RunManager.Instance.ActionQueueSet;
        ActionExecutor executor = RunManager.Instance.ActionExecutor;

        for (int i = 0; !issued.CompletionTask.IsCompleted; i++)
        {
            // A cancelled action never completes. When combat ends mid-cascade
            // the executor stops with actions still queued, and the decision we
            // issued is abandoned rather than finished — observed as
            // "EndPlayerTurnAction turn 5 not completed: running=False
            // current=<null> empty=False combat=False". Waiting on its
            // CompletionTask alone hangs forever, so treat the fight ending as
            // a legitimate reason for our decision never to resolve.
            if (!CombatManager.Instance.IsInProgress)
            {
                Log.Info($"[oracle] {issued} abandoned: combat ended");
                return false;
            }

            if (i > 15000)
            {
                throw new TimeoutException($"{issued} never completed");
            }

            await Task.Delay(1);
        }

        for (int i = 0; i < 20000; i++)
        {
            if (queue.IsEmpty && !executor.IsRunning)
            {
                return true;
            }

            // Say what we are stuck behind rather than just hanging. A wedged
            // driver and a slow one look identical from outside the process.
            if (i > 0 && i % 2000 == 0)
            {
                Log.Info($"[oracle] still waiting after {i}ms: empty={queue.IsEmpty} "
                    + $"running={executor.IsRunning} current={executor.CurrentlyRunningAction} "
                    + $"paused={executor.IsPaused} combat={CombatManager.Instance.IsInProgress} "
                    + $"actionsDisabled={CombatManager.Instance.PlayerActionsDisabled}");
            }

            await Task.Delay(1);
        }

        throw new TimeoutException($"action queue never drained after {issued}");
    }

    private static void Subscribe(RunManager run)
    {
        run.ActionExecutor.AfterActionExecuted += _ => _actions++;
        run.ChecksumTracker.ChecksumGenerated += (_, context, _) =>
        {
            _checksums++;
            if (_checksums <= 5)
            {
                Log.Info($"[oracle] checksum: {context}");
            }
        };
        run.RoomEntered += () => Log.Info("[oracle] room entered");
        run.RoomExited += () => Log.Info("[oracle] room exited");
        run.ActEntered += () => Log.Info("[oracle] act entered");
    }

    /// <summary>
    /// Dumps enough of the reached state to tell a real run from a hollow one.
    /// </summary>
    private static void Report(RunState state)
    {
        Player me = state.Players[0];
        Log.Info($"[oracle] REACHED act={state.CurrentActIndex} floor={state.TotalFloor} "
            + $"hp={me.Creature.CurrentHp}/{me.Creature.MaxHp} gold={me.Gold} "
            + $"relics=[{string.Join(",", me.Relics.Select(r => r.Id.Entry))}] "
            + $"deck={me.Deck.Cards.Count} combat={CombatManager.Instance.IsInProgress}");
        // ToSerializable is the only public way to read every stream at once;
        // RunRngSet exposes the individual streams but no indexer.
        var counters = state.Rng.ToSerializable().Counters;
        Log.Info($"[oracle] counters: {string.Join(" ", counters.OrderBy(c => c.Key).Select(c => $"{c.Key}={c.Value}"))}");
        Log.Info($"[oracle] actions={_actions} checksums={_checksums} elapsed={Clock.Elapsed.TotalSeconds:F1}s");
    }
}
