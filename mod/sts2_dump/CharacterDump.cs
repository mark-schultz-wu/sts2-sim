using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Characters;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Every character: what a run starts with, and what it draws from.
/// </summary>
/// <remarks>
/// The smallest dump and the one that ties the others together. A character is
/// three pool references, a starting deck, a starting relic and four numbers —
/// so nearly all of it is a pointer at content another dump already printed,
/// and the Rust side generates those as typed references rather than ids.
///
/// Eight classes, not the five <c>ModelDb.AllCharacters</c> lists. The other
/// three are <c>IsPlayable =&gt; false</c> — the deprecated placeholder, the
/// Deprived, and the "random" pseudo-character the select screen offers — and
/// they are dumped with that flag rather than left out, on the same reasoning
/// the monster dump reflects instead of walking encounter tables: a roster
/// gathered from what uses it is a roster with holes in it.
///
/// <c>StartingPotions</c> is not a column. It is <c>virtual</c> with an empty
/// default and no character overrides it, so the column would be empty for
/// every row in the build — and the dump would be asserting that a character
/// cannot start with a potion, which is not what the game says.
///
/// Presentation is deliberately not dumped: name and dialogue colours, speech
/// bubbles, map markers, icon paths, animation delays and select sfx.
/// </remarks>
public static class CharacterDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("character"));
        output.Append("# character <id> <playable> <gender> <startingHp> <startingGold> ");
        output.Append("<maxEnergy> <orbSlots> <alwaysShowsStarCounter> ");
        output.Append("<cardPool> <relicPool> <potionPool> <unlocksAfterRunAs>\n");
        output.Append("# deck <character> <index> <card> <pool>\n");
        output.Append("# startingrelic <character> <index> <relic>\n");
        output.Append("# scalestart <character> <ascension> <startingHp> <startingGold>\n");
        output.Append("# rows above are ascension 0; a scale* row is the level at which a value changes\n");

        List<CharacterModel> characters = AllCharacters();

        // Starting HP is the number ascension is most likely to touch, so it is
        // swept rather than read once. See `Ascension`.
        var start = new (int Hp, int Gold)[Ascension.Levels.Length][];
        foreach (int level in Ascension.Levels)
        {
            Ascension.SetLevel(level);
            start[level] = characters.Select(c => (c.StartingHp, c.StartingGold)).ToArray();
        }

        Ascension.SetLevel(0);

        var scaling = new StringBuilder();
        for (int i = 0; i < characters.Count; i++)
        {
            CharacterModel character = characters[i];
            string id = character.Id.Entry;

            output.Append(string.Join(" ", new[]
            {
                "character",
                id,
                character.IsPlayable ? "1" : "0",
                character.Gender.ToString(),
                start[0][i].Hp.ToString(CultureInfo.InvariantCulture),
                start[0][i].Gold.ToString(CultureInfo.InvariantCulture),
                character.MaxEnergy.ToString(CultureInfo.InvariantCulture),
                character.BaseOrbSlotCount.ToString(CultureInfo.InvariantCulture),
                character.ShouldAlwaysShowStarCounter ? "1" : "0",
                character.CardPool.Id.Entry,
                character.RelicPool.Id.Entry,
                character.PotionPool.Id.Entry,
                Unlocks(character),
            }));
            output.Append('\n');

            // Order is kept because a starting deck is a list and not a set:
            // the same card appears several times, and the order it is added in
            // is the order the first shuffle sees.
            int index = 0;
            foreach (CardModel card in character.StartingDeck)
            {
                // The pool too, because a card's identity in the table is
                // pool-scoped: `Strike` exists in two pools, so a starting deck
                // that only named the card would not say which one.
                output.Append($"deck {id} {index} {card.Id.Entry} {card.Pool.Id.Entry}\n");
                index++;
            }

            for (int r = 0; r < character.StartingRelics.Count; r++)
            {
                output.Append($"startingrelic {id} {r} {character.StartingRelics[r].Id.Entry}\n");
            }

            for (int level = 1; level < start.Length; level++)
            {
                if (start[level][i] != start[level - 1][i])
                {
                    scaling.Append(
                        $"scalestart {id} {level} {start[level][i].Hp} {start[level][i].Gold}\n");
                }
            }
        }

        output.Append(scaling);
        output.Append($"# {characters.Count} characters\n");

        foreach (string name in Enum.GetNames(typeof(CharacterGender)))
        {
            long value = Convert.ToInt64(Enum.Parse(typeof(CharacterGender), name));
            output.Append($"enum CharacterGender {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
        }

        Log.Info("[characterdump] begin\n" + output);
        Log.Info("[characterdump] end");
    }

    /// <summary>
    /// The character a run has to be finished as before this one is offered.
    /// </summary>
    /// <remarks>
    /// <c>UnlocksAfterRunAs</c> is <c>protected</c>, so this reads it by
    /// reflection rather than adding a reason for the game to expose it. Null
    /// means available from the start.
    /// </remarks>
    private static string Unlocks(CharacterModel character)
    {
        object? after = character
            .GetType()
            .GetProperty(
                "UnlocksAfterRunAs",
                BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)
            ?.GetValue(character);

        return after is CharacterModel c ? c.Id.Entry : "-";
    }

    /// <summary>
    /// Every character in the build, not just the playable five.
    /// </summary>
    private static List<CharacterModel> AllCharacters()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var characters = new List<CharacterModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(CharacterModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                characters.Add((CharacterModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return characters.OrderBy(c => c.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
