using System;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.Cards;
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
internal abstract class Pending
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
    /// Validates and applies an answer. Returns an error message, or null on
    /// success — in which case the parked action has been resumed.
    /// </summary>
    public abstract string? Accept(JsonElement req);

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
/// and twice in the shapes recorded under R4. They are all the same bug: a
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

    private CardSelection(IReadOnlyList<CardModel> options, int min, int max, string? asker)
    {
        _options = options;
        _min = min;
        _max = max;
        _asker = asker;
    }

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
        }).ToList(),
    };

    public override string? Accept(JsonElement req)
    {
        if (!req.TryGetProperty("cards", out JsonElement el) || el.ValueKind != JsonValueKind.Array)
        {
            return $"'{Kind}' expects a 'cards' array of option indices";
        }

        var chosen = new List<CardModel>();
        var seen = new HashSet<int>();
        foreach (JsonElement e in el.EnumerateArray())
        {
            if (!e.TryGetInt32(out int i) || i < 0 || i >= _options.Count)
            {
                return $"option index {e} out of range; {_options.Count} option(s) offered";
            }

            // The same card twice would be one selection to the game and two to
            // the client, which is a divergence waiting to happen.
            if (!seen.Add(i))
            {
                return $"option index {i} given twice";
            }

            chosen.Add(_options[i]);
        }

        if (chosen.Count < _min || chosen.Count > _max)
        {
            return $"selected {chosen.Count}, but this decision takes {_min}..{_max}";
        }

        Release();
        _answer.SetResult(chosen);
        return null;
    }

    protected override void Fail(Exception e) => _answer.SetException(e);
}

/// <summary>What to take from a set of rewards, and which card within a card reward.</summary>
/// <remarks>
/// The whole choice is asked at once because <c>GetSelectedCardReward</c> is
/// **synchronous** — by the time the game asks which card, there is no await to
/// park on. The async <c>RewardsSet.testSelector</c> runs first, so the card
/// index is gathered here and handed back later by
/// <see cref="HeadlessCardSelector"/>. This is the one place the protocol's shape
/// is dictated by a method signature rather than chosen.
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
    private static Dictionary<string, object?> Detail(Reward reward, int index)
    {
        var d = new Dictionary<string, object?>
        {
            ["index"] = index,
            ["type"] = reward.GetType().Name,
            ["populated"] = reward.IsPopulated,
        };

        switch (reward)
        {
            case GoldReward gold:
                d["amount"] = gold.Amount;
                break;
            case CardReward card:
                d["cards"] = card.Cards.Select(Observation.Ident).ToList();
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

    public override string? Accept(JsonElement req)
    {
        bool skip = req.TryGetProperty("skip", out JsonElement s) && s.ValueKind == JsonValueKind.True;
        if (skip)
        {
            if (_set.DisallowSkipping)
            {
                return "this reward set cannot be skipped";
            }

            Release();
            BackgroundWork.Hold();
            _answer.SetResult(new RewardPlan { Skip = true });
            return null;
        }

        if (!req.TryGetProperty("take", out JsonElement el) || el.ValueKind != JsonValueKind.Array)
        {
            return "'rewards' expects a 'take' array, or \"skip\": true";
        }

        var plan = new RewardPlan();
        var seen = new HashSet<int>();
        foreach (JsonElement e in el.EnumerateArray())
        {
            if (!e.TryGetProperty("index", out JsonElement iEl) || !iEl.TryGetInt32(out int i)
                || i < 0 || i >= _set.Rewards.Count)
            {
                return $"reward index out of range; {_set.Rewards.Count} reward(s) offered";
            }

            if (!seen.Add(i))
            {
                return $"reward index {i} given twice";
            }

            Reward reward = _set.Rewards[i];
            int? cardIndex = null;

            if (reward is CardReward card)
            {
                int count = card.Cards.Count();
                if (!e.TryGetProperty("card", out JsonElement cEl) || !cEl.TryGetInt32(out int c)
                    || c < 0 || c >= count)
                {
                    // Refused rather than defaulted to the first card: choosing
                    // for the client is the one thing the oracle must not do.
                    return $"reward {i} is a card reward and needs a 'card' index in 0..{count - 1}";
                }

                cardIndex = c;
            }

            plan.Take.Add((reward, cardIndex));
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
    /// Asks the client what to take from a reward set, then takes it.
    /// </summary>
    /// <remarks>
    /// Runs on the game's stack, inside <c>RewardsSet.Offer</c>, which is the
    /// point: <c>Offer</c> has already registered the set with the synchronizer,
    /// and <c>SelectLocalReward</c> throws without that registration. Generating
    /// rewards and driving selection ourselves would mean reimplementing that
    /// bookkeeping, which is the class of mistake this project keeps finding.
    /// </remarks>
    internal static async Task TakeRewards(RewardsSet set)
    {
        // A real player may leave a rewards screen having taken some and not
        // others, so an incomplete set is not an error here. Left at its default
        // of true, Offer would throw the moment the client skipped anything.
        set.ThrowInTestIfRewardsNotTaken = false;

        RewardPlan plan = await RewardChoice.Ask(set);
        RewardsSetSynchronizer sync = RunManager.Instance.RewardsSetSynchronizer;

        try
        {
            if (plan.Skip)
            {
                sync.SkipLocalRewardsSet();
                return;
            }

            foreach ((Reward reward, int? cardIndex) in plan.Take)
            {
                // Read back by the synchronous GetSelectedCardReward, which
                // cannot ask anyone anything. Cleared straight after so a later
                // reward cannot silently inherit this one's answer.
                RewardPlan.CardInFlight = cardIndex;
                try
                {
                    await sync.SelectLocalReward(reward);
                }
                finally
                {
                    RewardPlan.CardInFlight = null;
                }
            }
        }
        finally
        {
            BackgroundWork.Release();
        }
    }
}

/// <summary>What the client decided to do with a reward set.</summary>
internal sealed class RewardPlan
{
    public bool Skip { get; set; }

    public List<(Reward Reward, int? CardIndex)> Take { get; } = new List<(Reward, int?)>();

    /// <summary>
    /// The card index for the reward currently being taken, read by the
    /// synchronous <c>GetSelectedCardReward</c> callback.
    /// </summary>
    public static int? CardInFlight { get; set; }

    /// <summary>
    /// True while a plan is being applied, so the driver does not call the run
    /// quiescent halfway through it.
    /// </summary>
    /// <remarks>
    /// Rewards are taken after combat has ended, and the readiness test treats
    /// "not in combat" as ready — so without this the wait returns the instant
    /// the answer is accepted, and the client sees the state *before* its own
    /// rewards are applied: gold and deck unchanged, exactly as if nothing had
    /// been taken. Nothing else marks the work in progress, because the game
    /// fires `RewardsSet.Offer` without awaiting it.
    /// </remarks>
    public static bool Applying { get; set; }
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
        return CardSelection.Ask(options.ToList(), minSelect, maxSelect,
            RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString());
    }

    /// <summary>
    /// Hands back the card the client already chose, in <see cref="RewardChoice"/>.
    /// </summary>
    /// <remarks>
    /// Synchronous, so it cannot consult anyone. If no choice was gathered the
    /// oracle has been asked something it never offered, and it throws rather
    /// than picking: returning the first card would be the oracle choosing, and
    /// returning null would be a silent skip (R12).
    /// </remarks>
    public CardRewardSelection GetSelectedCardReward(
        IReadOnlyList<CardCreationResult> options, IReadOnlyList<CardRewardAlternative> alternatives)
    {
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
