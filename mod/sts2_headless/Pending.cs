using System;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Events.Custom.CrystalSphereEvent;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.TestSupport;

namespace Sts2Headless;

/// <summary>
/// A question the game is stopped mid-action waiting for us to answer.
/// </summary>
/// <remarks>
/// Some decisions are not commands the client issues; they are callbacks the
/// game makes *during* an action it has already begun. Playing `Burning Pact`
/// enqueues one <c>PlayCardAction</c>, and partway through resolving it the game
/// calls <c>ICardSelector.GetSelectedCards</c> to ask which card to exhaust.
/// There is nothing to enumerate beforehand — whether the question is asked at
/// all depends on how the action resolves — and nothing to call afterwards,
/// because by then the action is parked.
///
/// So the selector does not choose. It records the question, hands the game an
/// unfinished task, and the driver's wait loop treats "a pending exists" as a
/// decision point exactly like "the player may act". Answering completes the
/// task and the action resumes where it left off.
///
/// Each subclass owns its own wire shape and its own validation, so adding a
/// kind does not mean editing the protocol's dispatch.
/// </remarks>
public abstract class Pending
{
    /// <summary>The outstanding question, or null when the game is not waiting.</summary>
    /// <remarks>
    /// At most one can exist: the game asks from inside an action, and that
    /// action cannot proceed to ask a second thing until this is answered.
    /// </remarks>
    public static Pending? Current { get; private set; }

    public abstract string Kind { get; }

    /// <summary>The wire form of the question.</summary>
    public abstract Dictionary<string, object?> Describe();

    /// <summary>
    /// Validates and applies an answer. Returns a refusal reason, or null on
    /// success — in which case the parked action has been resumed.
    /// </summary>
    /// <remarks>
    /// A tagged reason and not a sentence, for the reason <see cref="Refusals"/>
    /// gives: every way of getting an answer wrong is particular to the question
    /// asked, and a client that cannot tell them apart cannot be compared
    /// against a simulator that also refuses.
    /// </remarks>
    public abstract Dictionary<string, object?>? Accept(JsonElement req);

    /// <summary>Every answer this question accepts, as the commands that give them.</summary>
    /// <remarks>
    /// In the order <see cref="Moves"/> lists them, which is the simulator's.
    /// </remarks>
    public abstract List<Dictionary<string, object?>> Answers();

    /// <summary>One <c>answer</c> command, carrying the one field that tells it apart.</summary>
    protected static Dictionary<string, object?> Answering(string field, object? value) =>
        new Dictionary<string, object?> { ["cmd"] = "answer", [field] = value };

    protected abstract void Fail(Exception e);

    protected static void Park(Pending pending)
    {
        if (Current != null)
        {
            throw new InvalidOperationException(
                $"the game asked '{pending.Kind}' while '{Current.Kind}' is still unanswered");
        }

        Current = pending;
        Log.Info($"[headless] pending {pending.Kind}");
    }

    /// <summary>Cleared before completing, so a resumed action may park again.</summary>
    protected static void Release() => Current = null;

    /// <summary>
    /// Fails the question, for when the run is torn down beneath it.
    /// </summary>
    /// <remarks>
    /// Without this a <c>Restart</c> would leave the game awaiting a task nobody
    /// will ever complete. <c>CleanUp</c> calls <c>CardSelectCmd.Reset</c>, but
    /// that deliberately does nothing while <c>TestMode.IsOn</c> and knows
    /// nothing about our task in any case.
    /// </remarks>
    /// <summary>
    /// Drops the question once the game cancelled the action that asked it,
    /// as <c>ActionQueueSet.CombatEnded</c> cancels a play paused on a choice
    /// when the fight is won. The action never resumes, so its task is left
    /// as it is: the game asks nothing more of it.
    /// </summary>
    public static void DropFor(GameAction cancelled)
    {
        if (Current is CardSelection selection && selection.AskedBy(cancelled))
        {
            Log.Info($"[headless] dropping pending {selection.Kind}: {cancelled} was cancelled");
            Current = null;
        }
    }

    public static void CancelOutstanding(string why)
    {
        Pending? pending = Current;
        if (pending == null)
        {
            return;
        }

        Log.Warn($"[headless] cancelling pending {pending.Kind}: {why}");
        Current = null;
        pending.Fail(new OperationCanceledException(why));
    }
}

/// <summary>
/// Work the game has started that no queue or combat flag accounts for.
/// </summary>
/// <remarks>
/// The driver's readiness test asks whether the action queue has drained and
/// whether the player's turn is ready for input. Out of combat both are
/// trivially true, so any work started outside the queue is invisible to it and
/// the run is called quiescent while the work is still running. The client then
/// sees the state *before* its own decision took effect.
///
/// This has now happened four times — taking rewards, choosing an event option,
/// and twice in the shapes recorded under H3. They are all the same bug: a
/// state that is legitimately reachable, briefly, and indistinguishable from
/// the state being waited for. Rather than add a fifth ad-hoc flag, anything
/// that starts such work registers it here.
///
/// **Registering is not the same as awaiting it.** The handler must not block
/// on the work, because the work may itself park a question for the client —
/// and a handler blocked inside <c>AwaitPendingOptionTasks</c> can never send
/// that question, which deadlocks the protocol. So the wait loop checks a
/// counter and still exits early on a pending.
/// </remarks>
internal static class BackgroundWork
{
    private static int _inFlight;

    public static bool InFlight => Volatile.Read(ref _inFlight) > 0;

    /// <summary>Follows a task that was started but must not be awaited here.</summary>
    public static void Track(Task work)
    {
        Hold();
        work.ContinueWith(_ => Release(), TaskScheduler.Default);
    }

    /// <summary>
    /// For work with no task to follow — where the gap begins before the task
    /// exists. Taking rewards is the case: the applier resumes asynchronously,
    /// so between accepting the answer and the applier starting there is a
    /// window in which nothing at all looks busy.
    /// </summary>
    public static void Hold() => Interlocked.Increment(ref _inFlight);

    public static void Release() => Interlocked.Decrement(ref _inFlight);
}

/// <summary>Which cards to pick from a prompt — discard, exhaust, upgrade, and the rest.</summary>
internal sealed class CardSelection : Pending
{
    private readonly TaskCompletionSource<IEnumerable<CardModel>> _answer =
        // Without RunContinuationsAsynchronously the game's continuation would
        // resume synchronously inside SetResult — re-entrantly, inside the
        // protocol handler still writing its response. With it, the continuation
        // is posted to Godot's loop and the handler returns first.
        new TaskCompletionSource<IEnumerable<CardModel>>(TaskCreationOptions.RunContinuationsAsynchronously);

    private readonly IReadOnlyList<CardModel> _options;
    private readonly int _min;
    private readonly int _max;
    private readonly string? _asker;

    /// <summary>Whether a back button closes the selection, handing the game no cards.</summary>
    private readonly bool _cancelable;

    /// <summary>The action that was running as the question was asked.</summary>
    private readonly GameAction? _action;

    /// <summary>The options picked so far, in the order they were picked.</summary>
    /// <remarks>
    /// The game's screens take a selection a click at a time, and hand the
    /// cards back in click order. The hand keeps a list, and a grid keeps a set
    /// whose freed slot goes to the next pick, so taking back the last pick
    /// leaves either screen where it was before that pick.
    /// </remarks>
    private readonly List<int> _picked = new List<int>();

    private CardSelection(IReadOnlyList<CardModel> options, int min, int max, string? asker)
    {
        _options = options;
        _min = min;
        _max = max;
        _asker = asker;
        _cancelable = CancelableSelection.Cancelable == true;
        CancelableSelection.Cancelable = null;
        _action = RunManager.Instance.ActionExecutor.CurrentlyRunningAction;
    }

    /// <summary>Whether <paramref name="action"/> is the action that asked.</summary>
    internal bool AskedBy(GameAction action) => ReferenceEquals(_action, action);

    public override string Kind => "select_cards";

    public static Task<IEnumerable<CardModel>> Ask(
        IReadOnlyList<CardModel> options, int min, int max, string? asker)
    {
        var pending = new CardSelection(options, min, max, asker);
        Park(pending);
        return pending._answer.Task;
    }

    public override Dictionary<string, object?> Describe() => new Dictionary<string, object?>
    {
        ["kind"] = Kind,
        ["min"] = _min,
        ["max"] = _max,

        // ICardSelector receives only options and bounds — not the prompt, the
        // source card or the pile. CardSelectorPrefs.Prompt exists but is
        // consumed by the UI path and never reaches a selector, so the running
        // action is the best attribution available and is labelled as such
        // rather than dressed up as the game's own prompt text.
        ["asker"] = _asker,
        ["options"] = _options.Select((c, i) => new Dictionary<string, object?>
        {
            ["index"] = i,
            ["card"] = Observation.Ident(c),
            ["text"] = Observation.Text(c),
        }).ToList(),
        ["picked"] = _picked.ToList(),
        ["pile"] = SharedPile(),
    };

    /// <summary>
    /// The fight's pile every option stands in, when they all stand in one,
    /// and null otherwise.
    /// </summary>
    /// <remarks>
    /// The selector is never told the pile, but each card knows its own. A
    /// front end reads this to list a draw pile the way
    /// <c>NCombatPileCardSelectScreen</c> does: sorted, so the order of the
    /// draws does not show. The run's deck is not a fight's pile, so a deck
    /// selection reports none.
    /// </remarks>
    private string? SharedPile()
    {
        PileType? shared = null;
        foreach (CardModel card in _options)
        {
            PileType? pile = card.Pile?.Type;
            if (pile is null or PileType.None or PileType.Deck)
            {
                return null;
            }
            if (shared != null && shared != pile)
            {
                return null;
            }
            shared = pile;
        }
        return shared?.ToString();
    }

    /// <summary>
    /// Each card not yet picked while the selection takes more, then taking
    /// back the last pick, then handing the picks over once there are enough.
    /// </summary>
    public override List<Dictionary<string, object?>> Answers()
    {
        var answers = new List<Dictionary<string, object?>>();
        if (_picked.Count < _max)
        {
            for (int i = 0; i < _options.Count; i++)
            {
                if (!_picked.Contains(i))
                {
                    answers.Add(Answering("pick", i));
                }
            }
        }

        if (_picked.Count > 0)
        {
            answers.Add(Answering("undo", true));
        }

        if (_picked.Count >= _min)
        {
            answers.Add(Answering("done", true));
        }

        if (_cancelable)
        {
            answers.Add(Answering("cancel", true));
        }

        return answers;
    }

    /// <remarks>
    /// <c>pick</c> adds one card, <c>undo</c> takes back the last, and
    /// <c>done</c> hands the picks to the game. <c>cards</c> is each of its
    /// cards picked and then done, in one answer. A refused answer leaves the
    /// picks as they were. A pick or an undo leaves the question parked, so the
    /// reply asks it again with the picks so far.
    /// </remarks>
    public override Dictionary<string, object?>? Accept(JsonElement req)
    {
        // The back button: the screen closes, and the game gets no cards.
        if (req.TryGetProperty("cancel", out JsonElement cancel))
        {
            if (cancel.ValueKind != JsonValueKind.True)
            {
                return Refusals.Malformed("'cancel' is only an answer when it is true");
            }
            if (!_cancelable)
            {
                return Refusals.Malformed("this selection cannot be cancelled");
            }
            Release();
            _answer.SetResult(Array.Empty<CardModel>());
            return null;
        }

        if (req.TryGetProperty("undo", out JsonElement undo))
        {
            if (undo.ValueKind != JsonValueKind.True)
            {
                return Refusals.Malformed("'undo' is only an answer when it is true");
            }

            if (_picked.Count == 0)
            {
                return Refusals.NothingPicked();
            }

            _picked.RemoveAt(_picked.Count - 1);
            return null;
        }

        var more = new List<int>();
        bool done;
        if (req.TryGetProperty("pick", out JsonElement pick))
        {
            if (!pick.TryGetInt32(out int i))
            {
                return Refusals.Malformed($"'{Kind}' expects a whole number in 'pick'");
            }

            more.Add(i);
            done = false;
        }
        else if (req.TryGetProperty("done", out JsonElement finished))
        {
            if (finished.ValueKind != JsonValueKind.True)
            {
                return Refusals.Malformed("'done' is only an answer when it is true");
            }

            done = true;
        }
        else if (req.TryGetProperty("cards", out JsonElement el) && el.ValueKind == JsonValueKind.Array)
        {
            foreach (JsonElement e in el.EnumerateArray())
            {
                if (!e.TryGetInt32(out int i))
                {
                    return Refusals.Malformed($"'{Kind}' expects whole numbers in its 'cards' array");
                }

                more.Add(i);
            }

            done = true;
        }
        else
        {
            return Refusals.Malformed(
                $"'{Kind}' expects 'pick', 'undo', 'done', or a 'cards' array of option indices");
        }

        var picked = new List<int>(_picked);
        foreach (int i in more)
        {
            if (i < 0 || i >= _options.Count)
            {
                return Refusals.OutOfRange(i, _options.Count);
            }

            // The same card twice would be one selection to the game and two to
            // the client, which is a divergence waiting to happen.
            if (picked.Contains(i))
            {
                return Refusals.Duplicate(i);
            }

            // The screen stops taking cards once it holds the most it asks for.
            if (picked.Count >= _max)
            {
                return Refusals.WrongCount(picked.Count + 1, _min, _max);
            }

            picked.Add(i);
        }

        if (!done)
        {
            _picked.Clear();
            _picked.AddRange(picked);
            return null;
        }

        if (picked.Count < _min)
        {
            return Refusals.WrongCount(picked.Count, _min, _max);
        }

        Release();
        _answer.SetResult(picked.Select(i => _options[i]).ToList());
        return null;
    }

    protected override void Fail(Exception e) => _answer.SetException(e);
}

/// <summary>Which bundle of cards to take, from Scroll Boxes' choose-a-bundle screen.</summary>
/// <remarks>
/// <c>CardSelectCmd.FromChooseABundleScreen</c> asks no selector. Outside
/// <c>TestMode</c> it shows its screen, and inside it takes the first bundle
/// unasked. So the patch that stands in for the screen asks this, and the
/// answer names one bundle, as a one-card <c>cards</c> array does.
/// </remarks>
internal sealed class BundleChoice : Pending
{
    private readonly TaskCompletionSource<int> _answer =
        new TaskCompletionSource<int>(TaskCreationOptions.RunContinuationsAsynchronously);

    private readonly IReadOnlyList<IReadOnlyList<CardModel>> _bundles;

    private BundleChoice(IReadOnlyList<IReadOnlyList<CardModel>> bundles)
    {
        _bundles = bundles;
    }

    public override string Kind => "choose_bundle";

    public static Task<int> Ask(IReadOnlyList<IReadOnlyList<CardModel>> bundles)
    {
        var pending = new BundleChoice(bundles);
        Park(pending);
        return pending._answer.Task;
    }

    public override Dictionary<string, object?> Describe() => new Dictionary<string, object?>
    {
        ["kind"] = Kind,
        ["bundles"] = _bundles.Select((bundle, i) => new Dictionary<string, object?>
        {
            ["index"] = i,
            ["cards"] = bundle.Select(c => new Dictionary<string, object?>
            {
                ["card"] = Observation.Ident(c),
                ["text"] = Observation.Text(c),
            }).ToList(),
        }).ToList(),
    };

    /// <summary>Each bundle, as a one-card selection names it.</summary>
    public override List<Dictionary<string, object?>> Answers() =>
        Enumerable.Range(0, _bundles.Count)
            .Select(i => Answering("cards", new List<int> { i }))
            .ToList();

    public override Dictionary<string, object?>? Accept(JsonElement req)
    {
        if (!req.TryGetProperty("cards", out JsonElement el) || el.ValueKind != JsonValueKind.Array)
        {
            return Refusals.Malformed($"'{Kind}' expects a 'cards' array holding one bundle index");
        }

        List<JsonElement> picked = el.EnumerateArray().ToList();
        if (picked.Count != 1)
        {
            return Refusals.WrongCount(picked.Count, 1, 1);
        }

        if (!picked[0].TryGetInt32(out int i))
        {
            return Refusals.Malformed($"'{Kind}' expects a whole number in its 'cards' array");
        }

        if (i < 0 || i >= _bundles.Count)
        {
            return Refusals.OutOfRange(i, _bundles.Count);
        }

        Release();
        _answer.SetResult(i);
        return null;
    }

    protected override void Fail(Exception e) => _answer.SetException(e);
}

/// <summary>A yes or a no, to a popup that asks for confirmation.</summary>
/// <remarks>
/// The abandon-run popup is the one: Trial's Double Down opens it. Its two
/// buttons are the whole question, so the answer carries one bool.
/// </remarks>
internal sealed class Confirmation : Pending
{
    private readonly TaskCompletionSource<bool> _answer =
        new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);

    public override string Kind => "confirm";

    public static Task<bool> Ask()
    {
        var pending = new Confirmation();
        Park(pending);
        return pending._answer.Task;
    }

    public override Dictionary<string, object?> Describe() => new Dictionary<string, object?>
    {
        ["kind"] = Kind,
    };

    /// <summary>Yes, then no.</summary>
    public override List<Dictionary<string, object?>> Answers() => new List<Dictionary<string, object?>>
    {
        Answering("confirm", true),
        Answering("confirm", false),
    };

    public override Dictionary<string, object?>? Accept(JsonElement req)
    {
        if (!req.TryGetProperty("confirm", out JsonElement el)
            || (el.ValueKind != JsonValueKind.True && el.ValueKind != JsonValueKind.False))
        {
            return Refusals.Malformed($"'{Kind}' expects a 'confirm' of true or false");
        }

        Release();
        _answer.SetResult(el.GetBoolean());
        return null;
    }

    protected override void Fail(Exception e) => _answer.SetException(e);
}

/// <summary>What to take from a set of rewards, and which card within a card reward.</summary>
/// <remarks>
/// Asked once per decision, not once per set. The client takes what it wants
/// and is asked again with the set as it now stands, until every reward has
/// been taken or it says <c>skip</c>. Nothing is forfeited by omission: the
/// only way to leave a reward behind is to ask for that.
///
/// This is the game's own shape rather than a convenience. A set completes when
/// <c>AllRewardsSuccessfullySelected</c> — <c>RewardsSetSynchronizer</c> checks
/// after every selection — and <c>SkipLocalRewardsSet</c> is what abandons the
/// rest. The loop below is those two facts and nothing more.
///
/// It is possible at all because <c>RewardsSet.Offer</c> <em>awaits</em>
/// <c>testSelector</c>, so the selector may park between rewards. The screen
/// the real game shows cannot be reached headless — <c>ShowScreen</c> returns
/// null while <c>TestMode.IsOn</c>, its scene needs a rendering device that
/// <c>--headless</c> replaces with a dummy, and <c>NOverlayStack.Instance</c> is
/// <c>NRun.Instance?.GlobalUi.Overlays</c>, which no headless run has — so this
/// is the interleaving, and it turns out to be enough.
///
/// The card <em>within</em> a card reward is still gathered up front, because
/// <c>GetSelectedCardReward</c> is <strong>synchronous</strong>: by the time the
/// game asks which card, there is no await to park on. It is handed back later
/// by <see cref="HeadlessCardSelector"/>. That is the one place the protocol's
/// shape is dictated by a method signature rather than chosen.
/// </remarks>
internal sealed class RewardChoice : Pending
{
    private readonly TaskCompletionSource<RewardPlan> _answer =
        new TaskCompletionSource<RewardPlan>(TaskCreationOptions.RunContinuationsAsynchronously);

    private readonly RewardsSet _set;

    private RewardChoice(RewardsSet set) => _set = set;

    public override string Kind => "rewards";

    public static Task<RewardPlan> Ask(RewardsSet set)
    {
        var pending = new RewardChoice(set);
        Park(pending);
        return pending._answer.Task;
    }

    public override Dictionary<string, object?> Describe() => new Dictionary<string, object?>
    {
        ["kind"] = Kind,
        ["skippable"] = !_set.DisallowSkipping,
        ["rewards"] = _set.Rewards.Select((r, i) => Detail(r, i)).ToList(),
    };

    /// <summary>
    /// One reward, described by what it actually offers.
    /// </summary>
    /// <remarks>
    /// <c>IsPopulated</c> is reported rather than assumed: a reward can exist in
    /// the set without content — `RelicReward.Populate` can fail to find a relic
    /// — and an unpopulated reward is not the same as an absent one.
    /// </remarks>
    internal static Dictionary<string, object?> Detail(Reward reward, int index)
    {
        var d = new Dictionary<string, object?>
        {
            ["index"] = index,
            ["type"] = reward.GetType().Name,
            ["populated"] = reward.IsPopulated,

            // A reward already taken stays in the set and stays in this list,
            // because the indices a client answers with are indices into the
            // set. Renumbering the survivors between questions would make the
            // same reward answer to a different number each time.
            //
            // It is also how a *failed* selection shows: OnSelect can return
            // false — a potion with no free slot — and the reward is then
            // neither taken nor gone, so the set never completes on its own and
            // the client is asked again with this still false.
            ["taken"] = reward.SuccessfullySelected,
        };

        switch (reward)
        {
            case GoldReward gold:
                d["amount"] = gold.Amount;
                break;
            case CardReward card:
                d["cards"] = card.Cards.Select(c => new Dictionary<string, object?>
                {
                    ["card"] = Observation.Ident(c),
                    ["text"] = Observation.Text(c),
                }).ToList();
                // What OnSelect offers besides a card, less Skip, which is
                // leaving the reward untaken. Absent where there is nothing
                // else, so a plain card reward reads as it always has.
                List<string> alternatives = Alternatives(card);
                if (alternatives.Count > 0)
                {
                    d["alternatives"] = alternatives;
                }
                break;
            case RelicReward relic:
                d["relic"] = relic.Relic?.Id.Entry;
                d["rarity"] = relic.Rarity.ToString();
                break;
            case PotionReward potion:
                d["potion"] = potion.Potion?.Id.Entry;
                break;
        }

        return d;
    }

    /// <summary>
    /// A card reward's alternatives other than <c>Skip</c>, by <c>OptionId</c>.
    /// </summary>
    /// <remarks>
    /// <c>Generate</c> is what <c>OnSelect</c> calls, and the hook it raises only
    /// adds to a list, so asking it here changes nothing.
    /// </remarks>
    /// <summary>
    /// Each reward not yet taken: a card reward's cards, then its
    /// alternatives, and anything else whole. Then the skip, where the set
    /// allows one.
    /// </summary>
    public override List<Dictionary<string, object?>> Answers()
    {
        var answers = new List<Dictionary<string, object?>>();
        IReadOnlyList<Reward> rewards = _set.Rewards.ToList();
        for (int i = 0; i < rewards.Count; i++)
        {
            Reward reward = rewards[i];
            if (reward.SuccessfullySelected)
            {
                continue;
            }

            if (reward is CardReward card)
            {
                for (int c = 0; c < card.Cards.Count(); c++)
                {
                    answers.Add(Taking(new Dictionary<string, object?> { ["index"] = i, ["card"] = c }));
                }

                foreach (string alternative in Alternatives(card))
                {
                    answers.Add(Taking(new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["alternative"] = alternative,
                    }));
                }
            }
            else
            {
                answers.Add(Taking(new Dictionary<string, object?> { ["index"] = i }));
            }
        }

        if (!_set.DisallowSkipping)
        {
            answers.Add(Answering("skip", true));
        }

        return answers;
    }

    /// <summary>A <c>take</c> answer, which carries a list holding the one reward.</summary>
    private static Dictionary<string, object?> Taking(Dictionary<string, object?> reward) =>
        Answering("take", new List<Dictionary<string, object?>> { reward });

    internal static List<string> Alternatives(CardReward card) =>
        CardRewardAlternative.Generate(card)
            .Select(a => a.OptionId)
            .Where(id => id != "Skip")
            .ToList();

    public override Dictionary<string, object?>? Accept(JsonElement req)
    {
        bool skip = req.TryGetProperty("skip", out JsonElement s) && s.ValueKind == JsonValueKind.True;
        if (skip)
        {
            if (_set.DisallowSkipping)
            {
                return Refusals.NotSkippable();
            }

            Release();
            BackgroundWork.Hold();
            _answer.SetResult(new RewardPlan { Skip = true });
            return null;
        }

        if (!req.TryGetProperty("take", out JsonElement el) || el.ValueKind != JsonValueKind.Array)
        {
            return Refusals.Malformed("'rewards' expects a 'take' array, or \"skip\": true");
        }

        var plan = new RewardPlan();
        var seen = new HashSet<int>();
        foreach (JsonElement e in el.EnumerateArray())
        {
            if (!e.TryGetProperty("index", out JsonElement iEl) || !iEl.TryGetInt32(out int i))
            {
                return Refusals.Malformed("each 'take' entry needs a whole-number 'index'");
            }

            if (i < 0 || i >= _set.Rewards.Count)
            {
                return Refusals.OutOfRange(i, _set.Rewards.Count);
            }

            if (!seen.Add(i))
            {
                return Refusals.Duplicate(i);
            }

            Reward reward = _set.Rewards[i];
            if (reward.SuccessfullySelected)
            {
                return Refusals.AlreadyTaken(i);
            }

            int? cardIndex = null;
            string? alternative = null;

            if (reward is CardReward alt
                && e.TryGetProperty("alternative", out JsonElement aEl)
                && aEl.ValueKind == JsonValueKind.String)
            {
                alternative = aEl.GetString()!;
                if (!Alternatives(alt).Contains(alternative))
                {
                    return Refusals.NoSuchAlternative(i, alternative);
                }
            }
            else if (reward is CardReward card)
            {
                int count = card.Cards.Count();
                if (!e.TryGetProperty("card", out JsonElement cEl) || !cEl.TryGetInt32(out int c)
                    || c < 0 || c >= count)
                {
                    // Refused rather than defaulted to the first card: choosing
                    // for the client is the one thing the oracle must not do.
                    return Refusals.NeedsCardChoice(i, count);
                }

                cardIndex = c;
            }

            plan.Take.Add((reward, cardIndex, alternative));
        }

        if (plan.Take.Count == 0)
        {
            // An empty take is the shape that used to mean "and forfeit the
            // rest", so it is refused rather than treated as a no-op: a client
            // still sending it is asking for the old behaviour and should hear
            // that it is gone.
            return Refusals.EmptyTake();
        }

        Release();

        // Held here rather than in the applier: the continuation runs
        // asynchronously, so between accepting the answer and the applier
        // actually starting there is a window in which the run looks idle —
        // and the driver would report the state before the rewards land.
        BackgroundWork.Hold();
        _answer.SetResult(plan);
        return null;
    }

    protected override void Fail(Exception e) => _answer.SetException(e);

    /// <summary>
    /// Asks the client what to take from a reward set, until nothing is left to
    /// ask about.
    /// </summary>
    /// <remarks>
    /// Runs on the game's stack, inside <c>RewardsSet.Offer</c>, which is the
    /// point: <c>Offer</c> has already registered the set with the synchronizer,
    /// and <c>SelectLocalReward</c> throws without that registration. Generating
    /// rewards and driving selection ourselves would mean reimplementing that
    /// bookkeeping, which is the class of mistake this project keeps finding.
    ///
    /// The loop's exit is the synchronizer's, not ours: <c>SelectLocalReward</c>
    /// completes the set the moment every reward in it has been taken, and
    /// <c>SkipLocalRewardsSet</c> completes it with the rest abandoned. Asking
    /// again while it is incomplete is therefore always a question the client
    /// can still answer.
    ///
    /// <c>BackgroundWork</c> is held across each application and released as the
    /// next question is parked, with no await in between — so the run never
    /// looks idle between two halves of one reward screen, and the client never
    /// sees a state where its last reward has not landed yet.
    /// </remarks>
    internal static async Task TakeRewards(RewardsSet set)
    {
        // A real player may leave a rewards screen having taken some and not
        // others, so an incomplete set is not an error here. Left at its default
        // of true, Offer would throw the moment the client skipped anything.
        set.ThrowInTestIfRewardsNotTaken = false;

        RewardsSetSynchronizer sync = RunManager.Instance.RewardsSetSynchronizer;

        while (!sync.IsRewardsSetCompleted(set))
        {
            RewardPlan plan = await RewardChoice.Ask(set);

            try
            {
                if (plan.Skip)
                {
                    sync.SkipLocalRewardsSet();
                    return;
                }

                foreach ((Reward reward, int? cardIndex, string? alternative) in plan.Take)
                {
                    // Read back by the synchronous GetSelectedCardReward, which
                    // cannot ask anyone anything. Cleared straight after so a
                    // later reward cannot silently inherit this one's answer.
                    RewardPlan.CardInFlight = cardIndex;
                    RewardPlan.AlternativeInFlight = alternative;
                    try
                    {
                        await sync.SelectLocalReward(reward);
                    }
                    finally
                    {
                        RewardPlan.CardInFlight = null;
                        RewardPlan.AlternativeInFlight = null;
                        RewardPlan.AlternativeAnswered = false;
                    }
                }
            }
            finally
            {
                BackgroundWork.Release();
            }
        }
    }
}

/// <summary>
/// A card reward selected on its own, as <c>Draft</c> selects each of its ten,
/// asked in the shape of a reward screen holding that one reward.
/// </summary>
/// <remarks>
/// <c>Reward.SelectUnsynchronized</c> is awaited outside any set, so nothing
/// gathered the card first, and <c>GetSelectedCardReward</c> cannot ask. This
/// asks before the selection runs, and the card goes in
/// <see cref="RewardPlan.CardInFlight"/> as a screen's does. The reward's
/// <c>CanSkip</c> is what the question allows.
/// </remarks>
internal sealed class LoneCardReward : Pending
{
    private readonly TaskCompletionSource<int?> _answer =
        new TaskCompletionSource<int?>(TaskCreationOptions.RunContinuationsAsynchronously);

    private readonly CardReward _reward;

    private LoneCardReward(CardReward reward) => _reward = reward;

    public override string Kind => "rewards";

    /// <summary>The card taken, or null for a skip.</summary>
    public static Task<int?> Ask(CardReward reward)
    {
        if (RewardChoice.Alternatives(reward).Count > 0)
        {
            throw new NotSupportedException(
                "a card reward selected alone offers alternatives; the oracle asks only for its cards");
        }

        var pending = new LoneCardReward(reward);
        Park(pending);
        return pending._answer.Task;
    }

    public override Dictionary<string, object?> Describe() => new Dictionary<string, object?>
    {
        ["kind"] = Kind,
        ["skippable"] = _reward.CanSkip,
        ["rewards"] = new List<Dictionary<string, object?>> { RewardChoice.Detail(_reward, 0) },
    };

    public override List<Dictionary<string, object?>> Answers()
    {
        var answers = new List<Dictionary<string, object?>>();
        for (int c = 0; c < _reward.Cards.Count(); c++)
        {
            answers.Add(Answering("take", new List<Dictionary<string, object?>>
            {
                new Dictionary<string, object?> { ["index"] = 0, ["card"] = c },
            }));
        }

        if (_reward.CanSkip)
        {
            answers.Add(Answering("skip", true));
        }

        return answers;
    }

    public override Dictionary<string, object?>? Accept(JsonElement req)
    {
        if (req.TryGetProperty("skip", out JsonElement s) && s.ValueKind == JsonValueKind.True)
        {
            if (!_reward.CanSkip)
            {
                return Refusals.NotSkippable();
            }

            Release();
            BackgroundWork.Hold();
            _answer.SetResult(null);
            return null;
        }

        if (!req.TryGetProperty("take", out JsonElement el) || el.ValueKind != JsonValueKind.Array
            || el.GetArrayLength() != 1)
        {
            return Refusals.Malformed("a lone card reward expects a 'take' of one entry, or \"skip\": true");
        }

        JsonElement e = el[0];
        if (!e.TryGetProperty("index", out JsonElement iEl) || !iEl.TryGetInt32(out int i))
        {
            return Refusals.Malformed("each 'take' entry needs a whole-number 'index'");
        }

        if (i != 0)
        {
            return Refusals.OutOfRange(i, 1);
        }

        int count = _reward.Cards.Count();
        if (!e.TryGetProperty("card", out JsonElement cEl) || !cEl.TryGetInt32(out int c)
            || c < 0 || c >= count)
        {
            return Refusals.NeedsCardChoice(i, count);
        }

        Release();
        BackgroundWork.Hold();
        _answer.SetResult(c);
        return null;
    }

    protected override void Fail(Exception e) => _answer.SetException(e);
}

/// <summary>What the client decided to do with a reward set.</summary>
internal sealed class RewardPlan
{
    public bool Skip { get; set; }

    public List<(Reward Reward, int? CardIndex, string? Alternative)> Take { get; } =
        new List<(Reward, int?, string?)>();

    /// <summary>
    /// The card index for the reward currently being taken, read by the
    /// synchronous <c>GetSelectedCardReward</c> callback.
    /// </summary>
    public static int? CardInFlight { get; set; }

    /// <summary>
    /// The alternative for the reward currently being taken, by
    /// <c>OptionId</c>, read the same way.
    /// </summary>
    public static string? AlternativeInFlight { get; set; }

    /// <summary>
    /// Whether the alternative in flight has been handed over. An alternative
    /// that keeps the selection open, as <c>REROLL</c> does, asks again, and
    /// the answer then is nothing: the reward stays on the screen, and the
    /// client chooses from its new pack as a fresh question.
    /// </summary>
    public static bool AlternativeAnswered { get; set; }
}

/// <summary>
/// Routes every card selection to the client instead of choosing one.
/// </summary>
/// <remarks>
/// Installed once per process via <c>CardSelectCmd.UseSelector</c>, which throws
/// if a selector is already active — so it must not be re-installed per run.
/// It survives <c>Restart</c> because <c>CardSelectCmd.Reset</c> clears the
/// stack only when <c>TestMode.IsOn</c> is false, which for the oracle it never
/// is.
///
/// Installing this changes what the game does: with no selector, `CardSelectCmd`
/// falls through to the UI path and would block forever headless. With the
/// game's own <c>TestCardSelector</c> it would answer from a pre-seeded queue,
/// which would mean the oracle choosing. Neither is acceptable, and this is the
/// seam the game provides for exactly that reason.
/// </remarks>
internal sealed class HeadlessCardSelector : ICardSelector
{
    public Task<IEnumerable<CardModel>> GetSelectedCards(IEnumerable<CardModel> options, int minSelect, int maxSelect)
    {
        // A choose-a-card screen asks for one, unless it can be skipped.
        if (ChooseACardSkip.CanSkip is bool canSkip)
        {
            minSelect = canSkip ? 0 : 1;
            ChooseACardSkip.CanSkip = null;
        }
        List<CardModel> offered = options.ToList();
        return GameApi.Sent(offered, CardSelection.Ask(offered, minSelect, maxSelect,
            RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString()));
    }

    /// <summary>
    /// Hands back the card the client already chose, in <see cref="RewardChoice"/>.
    /// </summary>
    /// <remarks>
    /// Synchronous, so it cannot consult anyone. If no choice was gathered the
    /// oracle has been asked something it never offered, and it throws rather
    /// than picking: returning the first card would be the oracle choosing, and
    /// returning null would be a silent skip (H10).
    /// </remarks>
    public CardRewardSelection GetSelectedCardReward(
        IReadOnlyList<CardCreationResult> options, IReadOnlyList<CardRewardAlternative> alternatives)
    {
        if (RewardPlan.AlternativeAnswered)
        {
            return new CardRewardSelection();
        }

        if (RewardPlan.AlternativeInFlight is string id)
        {
            CardRewardAlternative chosen = alternatives.FirstOrDefault(a => a.OptionId == id)
                ?? throw new NotSupportedException(
                    $"the card reward offers no alternative {id}; the oracle will not choose another");
            RewardPlan.AlternativeInFlight = null;
            RewardPlan.AlternativeAnswered = true;
            return new CardRewardSelection { alternative = chosen };
        }

        int? index = RewardPlan.CardInFlight;
        if (index == null || index >= options.Count)
        {
            throw new NotSupportedException(
                $"a card reward was offered with no choice supplied (index {index?.ToString() ?? "none"}, "
                + $"{options.Count} option(s)); the oracle will not choose one for you");
        }

        return new CardRewardSelection { card = options[index.Value].Card };
    }
}

/// <summary>Where to divine, in the Crystal Sphere's minigame.</summary>
/// <remarks>
/// The one event in the game with a screen of its own. Both of its options
/// <c>await CrystalSphereMinigame.PlayMinigame()</c>, which is
/// <c>NCrystalSphereScreen.ShowScreen(this)</c> and then a wait on a
/// <c>TaskCompletionSource</c> that only a click ever completes. Headless the
/// scene is not in the cache, and instantiating it takes the process down
/// without an exception — the last line in the log is
/// <c>Asset not cached: crystal_sphere_screen.tscn</c>.
///
/// So the screen is skipped (see <c>DivineWithoutAScreen</c>) and the clicks
/// arrive here instead. This is not the <c>NScreenShake</c> case: a click has
/// consequences. Each one costs a divination, uncovers cells, and what those
/// cells hold is what <c>DoLocalCrystalSphereRewards</c> pays out. A minigame
/// resolved by the oracle would be the oracle choosing.
///
/// Parked across clicks rather than once per click, for the same reason
/// <see cref="RewardChoice"/> is: the client answers, the count drops, and it is
/// asked again with the grid as it now stands. <c>DivinationCount</c> is
/// decremented at the top of <c>CellClicked</c>, before its first await, so
/// whether that was the last one is known by the time this returns.
/// </remarks>
internal sealed class Divination : Pending
{
    private readonly CrystalSphereMinigame _game;

    private Divination(CrystalSphereMinigame game) => _game = game;

    public override string Kind => "divine";

    public static void Ask(CrystalSphereMinigame game) => Park(new Divination(game));

    /// <summary>
    /// The grid, as what has been uncovered rather than as every cell.
    /// </summary>
    /// <remarks>
    /// A hidden cell is the default and there are 121 of them, so listing the
    /// cleared ones says the same thing in a line instead of a page. The client
    /// has the dimensions and can take the complement.
    ///
    /// <c>revealed</c> is what the divinations have turned up so far, which is
    /// the whole state of play: <c>IsGood</c> is false for a
    /// <c>CrystalSphereCurse</c>, and it is why <c>Big</c> is a gamble rather
    /// than a strictly better tool.
    /// </remarks>
    public override Dictionary<string, object?> Describe()
    {
        var cleared = new List<Dictionary<string, object?>>();
        for (int x = 0; x < _game.GridSize.X; x++)
        {
            for (int y = 0; y < _game.GridSize.Y; y++)
            {
                if (!_game.cells[x, y].IsHidden)
                {
                    cleared.Add(new Dictionary<string, object?> { ["x"] = x, ["y"] = y });
                }
            }
        }

        return new Dictionary<string, object?>
        {
            ["kind"] = Kind,
            ["left"] = _game.DivinationCount,
            ["width"] = _game.GridSize.X,
            ["height"] = _game.GridSize.Y,
            ["tool"] = _game.CrystalSphereTool.ToString(),
            ["cleared"] = cleared,
            ["revealed"] = _game.Items
                .Where(Uncovered)
                .Select(i => new Dictionary<string, object?>
                {
                    ["item"] = i.GetType().Name,
                    ["good"] = i.IsGood,
                    ["x"] = i.Position.X,
                    ["y"] = i.Position.Y,
                    ["width"] = i.Size.X,
                    ["height"] = i.Size.Y,
                })
                .ToList(),
        };
    }

    /// <summary>Whether every cell an item sits on has been uncovered.</summary>
    /// <remarks>
    /// Derived rather than read. <c>CrystalSphereItem.Revealed</c> is an event,
    /// and the minigame's <c>_revealed</c> list is private, but
    /// <c>AreAllOccupiedCellsClear</c> is the same test this makes: an item is
    /// found once nothing covers it.
    /// </remarks>
    /// <summary>Every hidden cell, column by column, with the small tool and then the big.</summary>
    public override List<Dictionary<string, object?>> Answers()
    {
        var answers = new List<Dictionary<string, object?>>();
        for (int x = 0; x < _game.GridSize.X; x++)
        {
            for (int y = 0; y < _game.GridSize.Y; y++)
            {
                if (!_game.cells[x, y].IsHidden)
                {
                    continue;
                }

                foreach (string tool in new[] { "Small", "Big" })
                {
                    answers.Add(Answering("divine", new Dictionary<string, object?>
                    {
                        ["x"] = x,
                        ["y"] = y,
                        ["tool"] = tool,
                    }));
                }
            }
        }

        return answers;
    }

    private bool Uncovered(CrystalSphereItem item)
    {
        for (int x = item.Position.X; x < item.Position.X + item.Size.X; x++)
        {
            for (int y = item.Position.Y; y < item.Position.Y + item.Size.Y; y++)
            {
                if (x < 0 || x >= _game.GridSize.X || y < 0 || y >= _game.GridSize.Y)
                {
                    return false;
                }

                if (_game.cells[x, y].IsHidden)
                {
                    return false;
                }
            }
        }

        return true;
    }

    public override Dictionary<string, object?>? Accept(JsonElement req)
    {
        if (!req.TryGetProperty("divine", out JsonElement at)
            || at.ValueKind != JsonValueKind.Object)
        {
            return Refusals.Malformed($"'{Kind}' expects a 'divine' object of 'x', 'y' and 'tool'");
        }

        if (!at.TryGetProperty("x", out JsonElement xe) || !xe.TryGetInt32(out int x)
            || !at.TryGetProperty("y", out JsonElement ye) || !ye.TryGetInt32(out int y))
        {
            return Refusals.Malformed("'divine' needs integer 'x' and 'y'");
        }

        if (x < 0 || x >= _game.GridSize.X || y < 0 || y >= _game.GridSize.Y)
        {
            return Refusals.OffGrid(x, y, _game.GridSize.X, _game.GridSize.Y);
        }

        // A cleared cell holds nothing left to uncover, so clicking one would
        // spend a divination on nothing. The screen does not offer it either.
        if (!_game.cells[x, y].IsHidden)
        {
            return Refusals.AlreadyUncovered(x, y);
        }

        // `None` is left out on purpose: `CellClicked` tests only for `Big`, so
        // `None` and `Small` are one move under two names.
        string tool = at.TryGetProperty("tool", out JsonElement te)
            && te.ValueKind == JsonValueKind.String
                ? te.GetString()!
                : "";
        CrystalSphereMinigame.CrystalSphereToolType picked = tool switch
        {
            "Small" => CrystalSphereMinigame.CrystalSphereToolType.Small,
            "Big" => CrystalSphereMinigame.CrystalSphereToolType.Big,
            _ => (CrystalSphereMinigame.CrystalSphereToolType)(-1),
        };

        if (picked == (CrystalSphereMinigame.CrystalSphereToolType)(-1))
        {
            return Refusals.Malformed($"'{tool}' is not a tool; 'Small' uncovers the cell, 'Big' its neighbours too");
        }

        _game.SetTool(picked);

        // Released *before* the click, not after. The minigame's completion
        // source is a bare TaskCompletionSource -- no
        // RunContinuationsAsynchronously -- so the click that spends the last
        // divination resumes `PlayMinigame` synchronously from inside
        // `CellClicked`, which runs `CompleteMinigame` and offers the rewards.
        // Releasing afterwards is too late: `RewardChoice.Ask` finds this
        // question still parked and throws
        // "the game asked 'rewards' while 'divine' is still unanswered".
        //
        // Knowing which click is the last is what makes this possible.
        // `DivinationCount` is public and decremented once per click, so one
        // remaining means this one ends it.
        if (_game.DivinationCount <= 1)
        {
            Release();
        }

        // Not awaited: the click resumes the event, and a handler blocked on it
        // could never send the question the event asks next. The wait loop
        // follows it through BackgroundWork instead.
        BackgroundWork.Track(_game.CellClicked(_game.cells[x, y]));

        return null;
    }

    /// <summary>
    /// Cancels the minigame's own wait, which is what <c>PlayMinigame</c> is
    /// parked on. Nothing here holds a task of its own to fail.
    /// </summary>
    protected override void Fail(Exception e) => _game.ForceMinigameEnd();
}
