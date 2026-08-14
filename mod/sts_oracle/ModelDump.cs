using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Text.Json;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Localization;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Models;
// The namespace and the class share a name, so the machine is aliased.
using MegaCrit.Sts2.Core.MonsterMoves.MonsterMoveStateMachine;
using StateMachine = MegaCrit.Sts2.Core.MonsterMoves.MonsterMoveStateMachine.MonsterMoveStateMachine;

namespace StsOracle;

/// <summary>
/// Writes out every model id the game knows, so the client can be generated
/// from it rather than guessing.
/// </summary>
/// <remarks>
/// Ids are not written down anywhere in the game: <c>ModelDb.GetId</c> derives
/// them from the C# type — <c>Slugify(type.Name)</c> for the entry and the
/// slugified base type for the category — and <c>ModelDb.Init</c> registers one
/// instance per type in <c>AllAbstractModelSubtypes</c>. So the authoritative
/// list is whatever that reflection walk produces at runtime, on this build.
///
/// Reconstructing it from decompiled filenames would come to nearly the same
/// answer and be wrong in ways nobody would notice: it would miss the
/// <c>_MODEL</c> suffix rule on categories, miss types the loader adds, and
/// keep types the loader skips. Asking the game is the same reason
/// <see cref="MapLegality"/> mirrors the screen's own rule rather than
/// re-deriving travel.
///
/// This mode starts no run. It waits for the model database, writes the file,
/// and quits.
/// </remarks>
internal static class ModelDump
{
    public static void Write(string path)
    {
        var categories = new SortedDictionary<string, SortedDictionary<string, string>>(
            StringComparer.Ordinal);
        var cardTypes = new SortedDictionary<string, Type>(StringComparer.Ordinal);
        var monsterTypes = new SortedDictionary<string, Type>(StringComparer.Ordinal);
        var eventTypes = new SortedDictionary<string, Type>(StringComparer.Ordinal);

        foreach (Type type in ModelDb.AllAbstractModelSubtypes)
        {
            string category = ModelDb.GetCategory(type);
            string entry = ModelDb.GetEntry(type);

            if (category == "CARD")
            {
                cardTypes[entry] = type;
            }
            if (category == "MONSTER")
            {
                monsterTypes[entry] = type;
            }
            if (category == "EVENT")
            {
                eventTypes[entry] = type;
            }

            if (!categories.TryGetValue(category, out SortedDictionary<string, string>? entries))
            {
                entries = new SortedDictionary<string, string>(StringComparer.Ordinal);
                categories[category] = entries;
            }

            // Two types that slugify alike would already have collided inside
            // ModelDb's own dictionary, with the second silently replacing the
            // first. If that ever happens, generating a client from this dump
            // would bake the collision in, so it stops here instead.
            if (entries.TryGetValue(entry, out string? existing) && existing != type.FullName)
            {
                throw new InvalidOperationException(
                    $"two model types share the id {category}/{entry}: {existing} and {type.FullName}");
            }

            entries[entry] = type.FullName ?? type.Name;
        }

        int total = 0;
        foreach (KeyValuePair<string, SortedDictionary<string, string>> c in categories)
        {
            total += c.Value.Count;
        }

        var doc = new Dictionary<string, object?>
        {
            // Stamped so a generated client can refuse a dump from a build it
            // was not generated against, the same way the mod refuses to load
            // against an unpinned assembly.
            ["game"] = "v0.107.1",
            ["assemblyHash"] = 1692500715,
            ["categories"] = categories,
            ["enums"] = Enums(),
            ["cards"] = CardData(cardTypes),
            ["intents"] = MonsterIntents(monsterTypes),
            ["restSiteOptions"] = RestSiteOptions(),
            ["eventOptions"] = EventOptionKeys(eventTypes),
        };

        string json = JsonSerializer.Serialize(doc, new JsonSerializerOptions { WriteIndented = true });
        File.WriteAllText(path, json + "\n");
        Log.Info($"[oracle] wrote {total} ids in {categories.Count} categories to {path}");
    }

    /// <summary>
    /// Every event option key, from the localisation tables.
    /// </summary>
    /// <remarks>
    /// <c>EventOption.TextKey</c> looks like
    /// <c>AROMA_OF_CHAOS.pages.INITIAL.options.LET_GO</c>, built by
    /// <c>$"{Slugify(GetType().Name)}.pages.{pageName}.options.{optionName}"</c>
    /// — though plenty of events pass the literal instead.
    ///
    /// The obvious registry, <c>AllPossibleOptions</c>, is the wrong one: it is
    /// abstract on <c>AncientEventModel</c>, not <c>EventModel</c>, so only nine
    /// of sixty-nine events implement it. The real registry is the loc table.
    /// <c>EventModel.GameInfoOptions</c> reads its own options straight out of
    /// it and throws <c>LocException</c> when an event has none, so every event
    /// is *required* to have its options there — which makes the table complete
    /// in a way that scraping literals out of the assembly is not.
    ///
    /// Keys carry a trailing <c>.name</c> or <c>.description</c>; the TextKey is
    /// the part before that. Anything shaped differently is reported rather than
    /// dropped, since a key that does not fit means the format assumption is
    /// wrong and the generated client would be built on it.
    /// </remarks>
    private static SortedDictionary<string, object?> EventOptionKeys(
        SortedDictionary<string, Type> eventTypes)
    {
        var keys = new SortedSet<string>(StringComparer.Ordinal);
        var odd = new SortedSet<string>(StringComparer.Ordinal);
        var problems = new List<string>();

        foreach (string table in new[] { "events", "ancients" })
        {
            foreach (string key in LocManager.Instance.GetTable(table).Keys)
            {
                string[] parts = key.Split('.');
                // EVENT . pages . PAGE . options . OPTION [ . name|description ]
                bool shaped = parts.Length >= 5 && parts[1] == "pages" && parts[3] == "options";
                if (!shaped)
                {
                    // Only the ones claiming to be options; page descriptions and
                    // event titles legitimately look nothing like this.
                    if (Array.IndexOf(parts, "options") >= 0)
                    {
                        odd.Add(key);
                    }
                    continue;
                }
                if (parts.Length > 6)
                {
                    odd.Add(key);
                    continue;
                }
                keys.Add(string.Join(".", parts[0], parts[1], parts[2], parts[3], parts[4]));
            }
        }

        int fromLoc = keys.Count;

        // The loc table alone is not the whole domain. Events that synthesise
        // their options -- Neow and Darv build one per relic -- have no static
        // loc entry per option, so their keys appear only here. Found because a
        // live run refused NEOW.pages.INITIAL.options.BOOMING_CONCH while every
        // offline test passed against a loc table that agreed with itself.
        foreach (KeyValuePair<string, Type> pair in eventTypes)
        {
            PropertyInfo? all = pair.Value.GetProperty(
                "AllPossibleOptions",
                BindingFlags.Instance | BindingFlags.Public | BindingFlags.FlattenHierarchy);
            if (all is null)
            {
                continue;
            }
            try
            {
                var model = ModelDb.GetByIdOrNull<EventModel>(ModelDb.GetId(pair.Value));
                if (model is null)
                {
                    continue;
                }
                if (all.GetValue(model) is not IEnumerable<EventOption> options)
                {
                    continue;
                }
                foreach (EventOption option in options)
                {
                    string[] parts = option.TextKey.Split('.');
                    if (parts.Length == 5 && parts[1] == "pages" && parts[3] == "options")
                    {
                        keys.Add(option.TextKey);
                    }
                    else
                    {
                        odd.Add(option.TextKey);
                    }
                }
            }
            catch (Exception e)
            {
                Exception cause = e is TargetInvocationException { InnerException: { } inner }
                    ? inner
                    : e;
                problems.Add($"{pair.Key}: {cause.GetType().Name}: {cause.Message}");
            }
        }

        if (problems.Count > 0)
        {
            throw new InvalidOperationException(
                $"could not read AllPossibleOptions for {problems.Count} event(s):\n  "
                + string.Join("\n  ", problems));
        }

        if (keys.Count == 0)
        {
            throw new InvalidOperationException(
                "no event option keys in the loc tables; the format assumption is wrong");
        }

        Log.Info($"[oracle] read {keys.Count} event option keys "
            + $"({fromLoc} from loc, {keys.Count - fromLoc} only from AllPossibleOptions, "
            + $"{odd.Count} unrecognised)");
        return new SortedDictionary<string, object?>(StringComparer.Ordinal)
        {
            ["keys"] = new List<string>(keys),
            ["unrecognised"] = new List<string>(odd),
        };
    }

    /// <summary>
    /// Every option a rest site can offer.
    /// </summary>
    /// <remarks>
    /// <c>RestSiteOption.OptionId</c> is an abstract instance property whose
    /// overrides are constants — <c>public override string OptionId => "HEAL"</c>
    /// — so the domain is the set of subclasses, and reading it needs an
    /// instance rather than a static lookup.
    ///
    /// Built with a null owner, which is sound because the base constructor only
    /// stores it and no override reads it. Checked rather than assumed: the
    /// instance is constructed and the property is called, so an override that
    /// did touch the owner would throw here instead of returning something
    /// plausible.
    ///
    /// These are not <c>AbstractModel</c>s, so they are absent from
    /// <c>ModelDb</c> and this walks the game assembly directly.
    /// </remarks>
    private static List<string> RestSiteOptions()
    {
        var ids = new List<string>();
        var problems = new List<string>();

        foreach (Type type in typeof(RestSiteOption).Assembly.GetTypes())
        {
            if (type.IsAbstract || !typeof(RestSiteOption).IsAssignableFrom(type))
            {
                continue;
            }

            try
            {
                object? option = Activator.CreateInstance(
                    type,
                    BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic,
                    binder: null,
                    args: new object?[] { null },
                    culture: null);
                if (option is not RestSiteOption rest)
                {
                    problems.Add($"{type.Name}: could not be constructed");
                    continue;
                }
                ids.Add(rest.OptionId);
            }
            catch (Exception e)
            {
                Exception cause = e is TargetInvocationException { InnerException: { } inner }
                    ? inner
                    : e;
                problems.Add($"{type.Name}: {cause.GetType().Name}: {cause.Message}");
            }
        }

        if (problems.Count > 0)
        {
            throw new InvalidOperationException(
                $"could not read {problems.Count} rest site option(s):\n  "
                + string.Join("\n  ", problems));
        }

        ids.Sort(StringComparer.Ordinal);
        Log.Info($"[oracle] read {ids.Count} rest site options");
        return ids;
    }

    /// <summary>
    /// Every move a monster can declare, per monster.
    /// </summary>
    /// <remarks>
    /// A creature's reported intent is <c>Monster.NextMove.Id</c>, which is
    /// <c>MoveState.StateId</c> — a string literal passed to a constructor
    /// inside each monster's <c>GenerateMoveStateMachine</c>. There is no enum
    /// and no registry, so the only way to learn the domain is to build every
    /// monster's state machine and read the ids out of it.
    ///
    /// Which is what the game does itself: <c>MonsterModel.GetIntents</c> calls
    /// <c>GenerateMoveStateMachine()</c> on a bare model, with no combat in
    /// progress, so this needs no run. Reached by reflection because the method
    /// is protected — the alternative, scraping the decompiled source for string
    /// literals, would find the same names and be wrong in ways nobody would
    /// notice the moment a monster builds an id rather than writing one.
    ///
    /// Kept per monster rather than flattened, because the same id is expected
    /// to recur across monsters and the pairing is what makes it unambiguous.
    /// Only states that are moves: <c>NextMove</c> is always a
    /// <see cref="MoveState"/>, so non-move states can never be an intent.
    /// </remarks>
    private static SortedDictionary<string, object?> MonsterIntents(
        SortedDictionary<string, Type> monsterTypes)
    {
        var intents = new SortedDictionary<string, object?>(StringComparer.Ordinal);
        var problems = new List<string>();

        foreach (KeyValuePair<string, Type> pair in monsterTypes)
        {
            try
            {
                MonsterModel? canonical =
                    ModelDb.GetByIdOrNull<MonsterModel>(ModelDb.GetId(pair.Value));
                if (canonical is null)
                {
                    problems.Add($"{pair.Key}: no registered instance");
                    continue;
                }

                // A mutable copy, because ten monsters — the Decimillipede
                // segments among them — throw CanonicalModelException when their
                // state machine is built on the registered instance. The game
                // itself only ever builds these on mutable copies.
                MonsterModel monster = canonical.ToMutable();

                MethodInfo? generate = pair.Value.GetMethod(
                    "GenerateMoveStateMachine",
                    BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public
                        | BindingFlags.FlattenHierarchy);
                if (generate is null)
                {
                    problems.Add($"{pair.Key}: has no GenerateMoveStateMachine");
                    continue;
                }

                if (generate.Invoke(monster, null) is not StateMachine machine)
                {
                    problems.Add($"{pair.Key}: GenerateMoveStateMachine returned nothing");
                    continue;
                }

                var ids = new List<string>();
                foreach (MonsterState state in machine.States.Values)
                {
                    if (state.IsMove && state is MoveState move)
                    {
                        ids.Add(move.StateId);
                    }
                }
                ids.Sort(StringComparer.Ordinal);
                // The HP roll range, declared abstract on MonsterModel so every
                // monster has one. Dumped alongside the moves because it is the
                // same instantiation, and because it is the only place the
                // game says how large these numbers actually get.
                intents[pair.Key] = new Dictionary<string, object?>
                {
                    ["moves"] = ids,
                    ["minHp"] = monster.MinInitialHp,
                    ["maxHp"] = monster.MaxInitialHp,
                };
            }
            catch (Exception e)
            {
                // The innermost exception, since reflection wraps whatever the
                // monster's own constructor threw.
                Exception cause = e is TargetInvocationException { InnerException: { } inner }
                    ? inner
                    : e;
                problems.Add($"{pair.Key}: {cause.GetType().Name}: {cause.Message}");
            }
        }

        if (problems.Count > 0)
        {
            throw new InvalidOperationException(
                $"could not read moves for {problems.Count} of {monsterTypes.Count} monsters:\n  "
                + string.Join("\n  ", problems));
        }

        Log.Info($"[oracle] read moves and hp for {intents.Count} monsters");
        return intents;
    }

    /// <summary>
    /// Writes out the members of every C# enum a dumped field can hold.
    /// </summary>
    /// <remarks>
    /// Declaration order, which is also value order, so a generated enum can
    /// mirror the discriminants rather than only the names.
    ///
    /// Reflected rather than inferred from the values the dump happens to
    /// contain. No card currently has <c>CardRarity.None</c>, so a set built by
    /// observation would omit it, and the generated client would then fail to
    /// decode a value the game can legitimately produce. The whole point of
    /// asking the game is to stop guessing at closed domains, and a domain
    /// derived from a sample is still a guess.
    /// </remarks>
    private static SortedDictionary<string, List<string>> Enums()
    {
        var wanted = new[]
        {
            typeof(CardType),
            typeof(CardRarity),
            typeof(CardKeyword),
            typeof(TargetType),
        };

        var enums = new SortedDictionary<string, List<string>>(StringComparer.Ordinal);
        foreach (Type type in wanted)
        {
            enums[type.Name] = new List<string>(Enum.GetNames(type));
        }
        return enums;
    }

    /// <summary>
    /// Reads every card's printed stats off the game's own registered instances.
    /// </summary>
    /// <remarks>
    /// These are the properties a card subclass overrides with a constant —
    /// <c>Type</c>, <c>Rarity</c>, <c>TargetType</c>, <c>EnergyCost.Canonical</c>.
    /// They are data wearing the shape of code, which is why they can be
    /// tabulated at all. A card's play effect cannot be, and nothing here
    /// pretends otherwise.
    ///
    /// Read from the instance rather than from the decompiled source for the
    /// same reason the id list is: what a type declares and what the game
    /// answers need not agree, and only one of the two is the oracle.
    ///
    /// Failures are collected and thrown together at the end. A dump that
    /// silently omitted the cards it could not read would generate a client
    /// that is simply missing them, which is the failure this whole exercise
    /// exists to prevent.
    /// </remarks>
    private static SortedDictionary<string, object?> CardData(
        SortedDictionary<string, Type> cardTypes)
    {
        var cards = new SortedDictionary<string, object?>(StringComparer.Ordinal);
        var problems = new List<string>();

        foreach (KeyValuePair<string, Type> pair in cardTypes)
        {
            string entry = pair.Key;
            Type type = pair.Value;

            try
            {
                CardModel? card = ModelDb.GetByIdOrNull<CardModel>(ModelDb.GetId(type));
                if (card is null)
                {
                    // Every CARD id came from the same reflection walk that
                    // ModelDb registers instances from, so a missing instance
                    // means those two disagree — worth knowing, not skipping.
                    problems.Add($"{entry} ({type.FullName}): no registered instance");
                    continue;
                }

                var keywords = new List<string>();
                foreach (CardKeyword keyword in card.CanonicalKeywords)
                {
                    keywords.Add(keyword.ToString());
                }
                keywords.Sort(StringComparer.Ordinal);

                cards[entry] = new Dictionary<string, object?>
                {
                    ["type"] = card.Type.ToString(),
                    ["rarity"] = card.Rarity.ToString(),
                    ["target"] = card.TargetType.ToString(),
                    // Canonical, not current: the printed cost, before relics,
                    // powers or upgrades move it.
                    ["cost"] = card.EnergyCost.Canonical,
                    ["costsX"] = card.EnergyCost.CostsX,
                    ["starCost"] = card.CanonicalStarCost,
                    ["keywords"] = keywords,
                    ["pool"] = ModelDb.GetEntry(card.Pool.GetType()),
                };
            }
            catch (Exception e)
            {
                problems.Add($"{entry} ({type.FullName}): {e.GetType().Name}: {e.Message}");
            }
        }

        if (problems.Count > 0)
        {
            throw new InvalidOperationException(
                $"could not read {problems.Count} of {cardTypes.Count} cards:\n  "
                + string.Join("\n  ", problems));
        }

        Log.Info($"[oracle] read stats for {cards.Count} cards");
        return cards;
    }
}
