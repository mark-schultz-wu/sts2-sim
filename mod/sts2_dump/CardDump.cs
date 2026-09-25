using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Factories;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Unlocks;

namespace Sts2Dump;

/// <summary>
/// Prints every card's static data, as ground truth for the simulator's
/// generated tables.
/// </summary>
/// <remarks>
/// Static means: true of the card with no run in progress. Not what playing it
/// does — that needs the engine and comes later.
///
/// Two fields are not static for every card, and pretending otherwise is the
/// trap this dump exists to avoid. <c>Type</c> and <c>TargetType</c> are
/// virtual, and a handful of cards compute them from combat state: Shiv and
/// Sovereign Blade widen to <c>AllEnemies</c> when a power is out, Mad Science
/// becomes whatever card it stored. Reading those off a bare model gives
/// "whatever this is with no combat running", which would bake a lie into a
/// permanent table. So overrides are detected by reflection and the cell is
/// marked <c>dynamic</c> instead — the simulator has to ask, not look up.
///
/// Detection is deliberately conservative: any override at all is marked, even
/// a constant one. Marking a constant as dynamic costs one trait method;
/// marking a dynamic as constant costs a divergence nobody can see.
///
/// Keywords come from <c>KeywordSources.Local</c>, not <c>All</c>. <c>All</c>
/// folds in run-level modifiers, so dumping it would bake one run's state into
/// the table.
///
/// A card's variables are the numbers printed on its face: Strike's 6, Falling
/// Star's 8 damage and its two 1s. <c>BaseValue</c> and not
/// <c>PreviewValue</c> — the preview is what the numbers become once
/// Strength, Vulnerable and every other hook has had its say, which is a fact
/// about a fight rather than about a card. A <c>CalculatedVar</c> is marked,
/// because its printed number is a base plus a count taken off live combat
/// state, and a simulator has to run that rule rather than read this.
/// </remarks>
public static class CardDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        // Stamped, so regenerating against a different build fails visibly
        // rather than quietly replacing the table.
        output.Append(Stamp.Header("card"));
        output.Append("# card <id> <pool> <type> <rarity> <target> <cost> <costsX> <starCost> <starCostsX> <maxUpgrade> <keywords> <tags>\n");
        output.Append("# var <card> <name> <base> <calculated> <class>\n");
        output.Append("# pool <pool> <index> <card>\n");
        output.Append("# combatpool <pool> <index> <card>\n");
        output.Append("# rewardpool <character> <index> <card>\n");

        foreach (CardPoolModel pool in ModelDb.AllCardPools.OrderBy(p => p.Id.Entry, StringComparer.Ordinal))
        {
            int index = 0;
            foreach (CardModel card in pool.AllCards)
            {
                output.Append($"pool {pool.Id.Entry} {index} {card.Id.Entry}\n");
                index++;
            }
        }

        // What a combat generation effect -- Power Potion, Attack Potion,
        // Discovery -- can draw from, already narrowed the way
        // `CardFactory.FilterForCombat` narrows it and in the order the shuffle
        // it feeds will see. Dumped rather than derived, because deriving it
        // would mean a per-card `CanBeGeneratedInCombat` and
        // `MultiplayerConstraint` column and then re-running the filter here.
        //
        // `SingleplayerOnly` is what a one-player run's `RunState` reports, and
        // `FilterForPlayerCount` narrows the same way after it, so one pass
        // over `GetUnlockedCards` is the whole filter.
        //
        // The type filter each effect adds is not applied: it belongs to the
        // effect and not to the pool, and filtering preserves order, so
        // narrowing this list by type gives what the game's own LINQ gives.
        foreach (CardPoolModel pool in ModelDb.AllCardPools.OrderBy(p => p.Id.Entry, StringComparer.Ordinal))
        {
            int index = 0;
            IEnumerable<CardModel> unlocked = pool.GetUnlockedCards(
                UnlockState.all, CardMultiplayerConstraint.SingleplayerOnly);
            foreach (CardModel card in CardFactory.FilterForCombat(unlocked))
            {
                output.Append($"combatpool {pool.Id.Entry} {index} {card.Id.Entry}\n");
                index++;
            }
        }

        // What a merchant stocks a shelf from. `MerchantInventory` reads
        // `Character.CardPool` and `ColorlessCardPool` through
        // `GetUnlockedCards`, and `CreateForMerchant` then drops the basics and
        // runs `FilterForPlayerCount` over what is left.
        //
        // Dumped rather than derived for the same reason `combatpool` is: the
        // filter reads `MultiplayerConstraint`, which is on the model and in no
        // column here. The rarity and type filters a shelf adds are not
        // applied, because they belong to the shelf and filtering preserves
        // order.
        foreach (CardPoolModel pool in ModelDb.AllCardPools.OrderBy(p => p.Id.Entry, StringComparer.Ordinal))
        {
            int index = 0;
            IEnumerable<CardModel> unlocked = pool.GetUnlockedCards(
                UnlockState.all, CardMultiplayerConstraint.SingleplayerOnly);
            foreach (CardModel card in unlocked)
            {
                output.Append($"shoppool {pool.Id.Entry} {index} {card.Id.Entry}\n");
                index++;
            }
        }

        foreach (CharacterModel character in ModelDb.AllCharacters.Where(c => c.IsPlayable))
        {
            Player player = Player.CreateForNewRun(character, UnlockState.all, 1uL);
            CardCreationOptions options = CardCreationOptions.ForRoom(player, RoomType.Monster);
            int index = 0;
            foreach (CardModel card in options.GetPossibleCards(player))
            {
                output.Append($"rewardpool {character.Id.Entry} {index} {card.Id.Entry}\n");
                index++;
            }
        }

        List<CardModel> cards = ModelDb.AllCards
            .OrderBy(c => c.Id.Entry, StringComparer.Ordinal)
            .ToList();

        foreach (CardModel card in cards)
        {
            Type t = card.GetType();

            string type = Computed(t, nameof(CardModel.Type)) ? "dynamic" : card.Type.ToString();
            string target = Computed(t, nameof(CardModel.TargetType))
                ? "dynamic"
                : card.TargetType.ToString();

            IReadOnlySet<CardKeyword> keywords = card.GetKeywordsWithSources(KeywordSources.Local);
            string keywordList = keywords.Count == 0
                ? "-"
                : string.Join(",", keywords.Select(k => k.ToString()).OrderBy(k => k, StringComparer.Ordinal));

            // `Tags` is `CanonicalTags` on every card but a mock, so it is a
            // fact about the card rather than about a fight.
            List<string> tags = card.Tags.Select(t => t.ToString()).OrderBy(t => t, StringComparer.Ordinal).ToList();
            string tagList = tags.Count == 0 ? "-" : string.Join(",", tags);

            output.Append(string.Join(" ", new[]
            {
                "card",
                card.Id.Entry,
                card.Pool?.Id.Entry ?? "NONE",
                type,
                card.Rarity.ToString(),
                target,
                card.EnergyCost.Canonical.ToString(CultureInfo.InvariantCulture),
                card.EnergyCost.CostsX ? "1" : "0",
                card.CanonicalStarCost.ToString(CultureInfo.InvariantCulture),
                card.HasStarCostX ? "1" : "0",
                card.MaxUpgradeLevel.ToString(CultureInfo.InvariantCulture),
                keywordList,
                tagList,
            }));
            output.Append('\n');

            foreach (KeyValuePair<string, DynamicVar> entry in card.DynamicVars
                .OrderBy(v => v.Key, StringComparer.Ordinal))
            {
                output.Append(string.Join(" ", new[]
                {
                    "var",
                    card.Id.Entry,
                    entry.Key,
                    entry.Value.BaseValue.ToString(CultureInfo.InvariantCulture),
                    entry.Value is CalculatedVar ? "1" : "0",
                    VarClass(entry.Value),
                }));
                output.Append('\n');
            }
        }

        output.Append($"# {cards.Count} cards\n");

        // The enums the card fields draw from, in declaration order.
        //
        // Reflected rather than inferred from the values above: a member no
        // card happens to use is still a member, and a generated enum missing
        // one fails to decode the first time content grows into it. Order
        // matters because it is the discriminant.
        foreach (Type e in new[]
        {
            typeof(CardType), typeof(CardRarity), typeof(TargetType), typeof(CardKeyword), typeof(CardTag),
        })
        {
            foreach (string name in Enum.GetNames(e))
            {
                output.Append($"enum {e.Name} {Convert.ToInt64(Enum.Parse(e, name)).ToString(CultureInfo.InvariantCulture)} {name}\n");
            }
        }

        Log.Info("[carddump] begin\n" + output);
        Log.Info("[carddump] end");
    }

    /// <summary>
    /// The <c>DynamicVar</c> subclass this number is, by its own name.
    /// </summary>
    /// <remarks>
    /// The class is what decides the number's rules, and the key it is filed
    /// under says nothing: <c>SovereignBlade</c>'s block is a
    /// <c>CalculatedBlockVar</c> named <c>CalculatedBlock</c>, and its preview
    /// runs the block hooks exactly as a <c>BlockVar</c> named <c>Block</c>
    /// does.
    ///
    /// A generic var prints without the arity C# spells it with, so
    /// <c>PowerVar`1</c> is dumped as <c>PowerVar</c>.
    ///
    /// A <c>CalculatedDamageVar</c> built with <c>FromOsty</c> is dumped as
    /// <c>CalculatedDamageVar+FromOsty</c>. Its preview runs the damage hooks
    /// with the owner's Osty dealing, as an <c>OstyDamageVar</c>'s does.
    /// </remarks>
    private static string VarClass(DynamicVar var)
    {
        string name = var.GetType().Name;
        int arity = name.IndexOf('`');
        name = arity < 0 ? name : name.Substring(0, arity);
        return var is CalculatedDamageVar { IsFromOsty: true } ? name + "+FromOsty" : name;
    }

    /// <summary>
    /// Whether this card computes the property rather than inheriting
    /// <c>CardModel</c>'s own.
    /// </summary>
    private static bool Computed(Type card, string property)
    {
        MethodInfo? getter = card
            .GetProperty(property, BindingFlags.Public | BindingFlags.Instance)
            ?.GetGetMethod();

        return getter != null && getter.DeclaringType != typeof(CardModel);
    }
}
