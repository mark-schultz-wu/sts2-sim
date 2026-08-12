using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.TestSupport;

namespace StsOracle;

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
/// </remarks>
internal sealed class Pending
{
    /// <summary>The outstanding question, or null when the game is not waiting.</summary>
    /// <remarks>
    /// At most one can exist: the game asks from inside an action, and that
    /// action cannot proceed to ask a second thing until this is answered.
    /// </remarks>
    public static Pending? Current { get; private set; }

    public string Kind { get; }
    public int Min { get; }
    public int Max { get; }
    public IReadOnlyList<CardModel> Options { get; }

    /// <summary>The action that asked, for diagnostics.</summary>
    /// <remarks>
    /// <c>ICardSelector.GetSelectedCards</c> receives only options and bounds —
    /// not the prompt, the source card or the pile. <c>CardSelectorPrefs.Prompt</c>
    /// exists but is consumed by the UI path and never reaches a selector, so
    /// the running action is the best available attribution and is reported as
    /// such rather than dressed up as the game's own prompt text.
    /// </remarks>
    public string? Asker { get; }

    // RunContinuationsAsynchronously matters: without it the game's continuation
    // would resume synchronously inside our SetResult, i.e. re-entrantly inside
    // the protocol handler that is still writing its response. With it, the
    // continuation is posted to Godot's loop and the handler returns first.
    private readonly TaskCompletionSource<IEnumerable<CardModel>> _answer =
        new TaskCompletionSource<IEnumerable<CardModel>>(TaskCreationOptions.RunContinuationsAsynchronously);

    private Pending(string kind, IReadOnlyList<CardModel> options, int min, int max, string? asker)
    {
        Kind = kind;
        Options = options;
        Min = min;
        Max = max;
        Asker = asker;
    }

    public static Task<IEnumerable<CardModel>> Park(
        string kind, IReadOnlyList<CardModel> options, int min, int max, string? asker)
    {
        if (Current != null)
        {
            throw new InvalidOperationException(
                $"the game asked '{kind}' while '{Current.Kind}' is still unanswered");
        }

        var pending = new Pending(kind, options, min, max, asker);
        Current = pending;
        Log.Info($"[oracle] pending {kind}: {options.Count} option(s), select {min}..{max}");
        return pending._answer.Task;
    }

    /// <summary>Completes the question, letting the parked action resume.</summary>
    public void Answer(IEnumerable<CardModel> chosen)
    {
        // Cleared before completing, so the resumed action may immediately park
        // a new question without tripping the one-at-a-time check.
        Current = null;
        _answer.SetResult(chosen);
    }

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

        Log.Warn($"[oracle] cancelling pending {pending.Kind}: {why}");
        Current = null;
        pending._answer.SetException(new OperationCanceledException(why));
    }
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
internal sealed class OracleCardSelector : ICardSelector
{
    public Task<IEnumerable<CardModel>> GetSelectedCards(IEnumerable<CardModel> options, int minSelect, int maxSelect)
    {
        List<CardModel> list = options.ToList();
        return Pending.Park("select_cards", list, minSelect, maxSelect,
            RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString());
    }

    /// <summary>
    /// Not implemented, and loud about it.
    /// </summary>
    /// <remarks>
    /// This one is synchronous, so the client cannot be consulted from inside
    /// it; the answer has to be gathered earlier, in the async
    /// <c>RewardsSet.testSelector</c>, and handed back here. Until that path
    /// exists there is no correct value to return — returning the first card
    /// would be the oracle choosing, and returning null would be a silent
    /// skip — so it throws and the failure reaches the client (R12).
    /// </remarks>
    public CardRewardSelection GetSelectedCardReward(
        IReadOnlyList<CardCreationResult> options, IReadOnlyList<CardRewardAlternative> alternatives)
    {
        throw new NotSupportedException(
            "card rewards are not implemented yet; the oracle will not choose one for you");
    }
}
