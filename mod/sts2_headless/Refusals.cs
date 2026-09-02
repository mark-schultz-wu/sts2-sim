using System.Collections.Generic;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Rooms;

namespace Sts2Headless;

/// <summary>
/// Why a command was refused, as a tagged object rather than a sentence.
/// </summary>
/// <remarks>
/// A refusal is a legitimate answer and not an error: the run did not move, and
/// a client that was told a move was legal and then refused has found the bug
/// this whole project exists to find. What it was not, until now, is
/// <em>comparable</em> — the reason was English prose, and a simulator would
/// have had to write the same English to be seen agreeing.
///
/// So the wire carries <c>{"kind": ..., ...}</c> and nothing else. There is no
/// <c>message</c> beside it: the prose is the Rust side's <c>Display</c>,
/// written once from the data, so the two cannot drift. The mod's own log line
/// serialises this object, which is as readable as the sentence was and has one
/// source.
///
/// <b>Every field is a fact the client already has.</b> An index and a count, a
/// coordinate, a card id, the numbers a purchase turned on — all of it is in
/// the observation the client was acting on, which is what makes a refusal
/// checkable rather than merely informative.
///
/// <c>malformed</c> is the exception and is the measure of what the typed
/// vocabulary bought. About half of the sites below answer a request the Rust
/// client's types cannot build — a missing field, a command that is not a
/// command, a number where a name should be. Nothing driving the game through
/// <c>Play</c> can produce one; only <c>send_raw</c> can, which exists to test
/// exactly that.
/// </remarks>
internal static class Refusals
{
    // ---- wrong moment: the command is fine, the run is not there ----

    public static Dictionary<string, object?> NoRun() => Kind("no_run");

    public static Dictionary<string, object?> RunOver(bool victory) =>
        Kind("run_over", ("victory", victory));

    public static Dictionary<string, object?> NotInCombat() => Kind("not_in_combat");

    public static Dictionary<string, object?> InCombat() => Kind("in_combat");

    public static Dictionary<string, object?> WrongRoom(RoomType wanted, AbstractRoom? found) =>
        Kind("wrong_room", ("wanted", wanted.ToString()), ("found", Named(found)));

    public static Dictionary<string, object?> NothingToChoose(AbstractRoom? found) =>
        Kind("nothing_to_choose", ("found", Named(found)));

    public static Dictionary<string, object?> Pending(string kind) =>
        Kind("pending", ("asking", kind));

    public static Dictionary<string, object?> NothingPending() => Kind("nothing_pending");

    // ---- no such thing: an index or an id that names nothing ----

    public static Dictionary<string, object?> OutOfRange(long index, int count) =>
        Kind("out_of_range", ("index", index), ("count", count));

    public static Dictionary<string, object?> NoSuchCreature(uint id) =>
        Kind("no_such_creature", ("id", id));

    // ---- the rules say no: the game looked, and said no ----

    public static Dictionary<string, object?> Unplayable(
        CardModel card,
        UnplayableReason why,
        AbstractModel? preventer) =>
        Kind("unplayable",
            // The packed spelling, the same one the hand this card came out of
            // uses. `Entry` alone cannot tell two Strikes apart when one is
            // upgraded, and telling them apart is the whole of what makes a
            // refusal checkable against the hand the client was given.
            ("card", Observation.Ident(card)),
            // The same spelling the observation gives the same fact: a comma
            // separated list of the [Flags] members, which is what
            // `UnplayableReason.ToString()` writes, and null for the empty set
            // rather than the member named `None`. One spelling for one fact, so
            // the client reads it with the parser it already has.
            ("why", why == UnplayableReason.None ? null : why.ToString()),
            ("preventer", preventer == null ? null : Observation.Id(preventer.Id)));

    public static Dictionary<string, object?> WrongTarget(CardModel card, uint? target) =>
        Kind("wrong_target",
            ("card", Observation.Ident(card)),
            ("target", target),
            ("takes", card.TargetType.ToString()));

    /// <summary>A potion that cannot be drunk right now, and why.</summary>
    /// <remarks>
    /// The reason is the same vocabulary the observation reports on the belt,
    /// so a client that read the belt and asked anyway is told the same word it
    /// was already holding.
    /// </remarks>
    public static Dictionary<string, object?> Undrinkable(PotionModel potion, string why) =>
        Kind("undrinkable",
            ("potion", potion.Id.Entry),
            ("reason", why));

    public static Dictionary<string, object?> WrongPotionTarget(PotionModel potion, uint? target) =>
        Kind("wrong_potion_target",
            ("potion", potion.Id.Entry),
            ("target", target),
            ("takes", potion.TargetType.ToString()));

    /// <summary>
    /// Not a node the run may enter from where it stands.
    /// </summary>
    /// <remarks>
    /// The coordinate spelled the way the observation spells one, so a client
    /// can hold this against the <c>travelable</c> list it was given. The legal
    /// set itself is not repeated: it is already in that observation, and a
    /// refusal restating state is a second copy that can disagree with the
    /// first.
    /// </remarks>
    public static Dictionary<string, object?> NotTravelable(int col, int row) =>
        Kind("not_travelable", ("at", $"{col},{row}"));

    public static Dictionary<string, object?> CannotProceed() => Kind("cannot_proceed");

    public static Dictionary<string, object?> OptionDisabled() => Kind("option_disabled");

    public static Dictionary<string, object?> OptionLocked() => Kind("option_locked");

    public static Dictionary<string, object?> AlreadyAnswered() => Kind("already_answered");

    public static Dictionary<string, object?> Unpurchasable(
        int index,
        MerchantEntry entry,
        Player buyer) =>
        Kind("unpurchasable",
            ("index", index),
            ("cost", entry.Cost),
            ("gold", (int)buyer.Gold),
            ("stocked", entry.IsStocked),
            ("blocked", Observation.Blocked(entry, buyer)));

    // ---- answering a question ----

    public static Dictionary<string, object?> Duplicate(int index) =>
        Kind("duplicate", ("index", index));

    public static Dictionary<string, object?> WrongCount(int chosen, int min, int max) =>
        Kind("wrong_count", ("chosen", chosen), ("min", min), ("max", max));

    public static Dictionary<string, object?> NotSkippable() => Kind("not_skippable");

    public static Dictionary<string, object?> AlreadyTaken(int index) =>
        Kind("already_taken", ("index", index));

    public static Dictionary<string, object?> NeedsCardChoice(int index, int count) =>
        Kind("needs_card_choice", ("index", index), ("count", count));

    public static Dictionary<string, object?> EmptyTake() => Kind("empty_take");

    public static Dictionary<string, object?> OffGrid(int x, int y, int width, int height) =>
        Kind("off_grid", ("x", x), ("y", y), ("width", width), ("height", height));

    public static Dictionary<string, object?> AlreadyUncovered(int x, int y) =>
        Kind("already_uncovered", ("x", x), ("y", y));

    // ---- the game's own answer, and the one a typed client cannot reach ----

    /// <summary>
    /// The game's own console declined, in its own words.
    /// </summary>
    /// <remarks>
    /// <c>FightConsoleCmd</c> and <c>EventConsoleCmd</c> know why they declined
    /// better than an adapter wrapping them does, so the message travels
    /// verbatim rather than being classified. Classifying it would be reading
    /// MegaCrit's source and guessing.
    /// </remarks>
    public static Dictionary<string, object?> Console(string message) =>
        Kind("console", ("message", message));

    /// <summary>
    /// A request the typed vocabulary cannot build.
    /// </summary>
    /// <remarks>
    /// Half the sites in <c>Protocol</c>, and none of them reachable through
    /// <c>Decision</c>: a missing field, a command that is not a command, a
    /// number where a name should be. The message stays prose because there is
    /// nothing to compare — a client that sent this was not speaking the
    /// vocabulary in the first place.
    /// </remarks>
    public static Dictionary<string, object?> Malformed(string message) =>
        Kind("malformed", ("message", message));

    /// <summary>The room type a room is, for the rooms this protocol names.</summary>
    /// <remarks>
    /// Null for a room with no <c>RoomType</c> worth reporting — a combat room,
    /// or no room at all. The four here are the four <c>choose_option</c> and
    /// <c>buy</c> care about, which is every room that offers anything.
    /// </remarks>
    private static string? Named(AbstractRoom? room) => room switch
    {
        RestSiteRoom => nameof(RoomType.RestSite),
        TreasureRoom => nameof(RoomType.Treasure),
        EventRoom => nameof(RoomType.Event),
        MerchantRoom => nameof(RoomType.Shop),
        _ => null,
    };

    private static Dictionary<string, object?> Kind(
        string kind,
        params (string Name, object? Value)[] fields)
    {
        var reason = new Dictionary<string, object?> { ["kind"] = kind };
        foreach ((string name, object? value) in fields)
        {
            reason[name] = value;
        }

        return reason;
    }
}
