//! Content by the ids the game spells it with, as a runner's arguments and
//! a recorded run's files write it.

use sts2_core::RunModifier;

/// A value by the id the game spells it with.
pub fn read_id<T: serde::de::DeserializeOwned>(id: &str) -> Result<T, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(id.to_owned()))
}

/// A run modifier as text: its id as the game spells it. One set up with a
/// character adds `:` and the character's id, as `CHARACTER_CARDS:DEFECT`.
pub fn spell_modifier<V: sts2_core::ContentVersion>(entry: &RunModifier<V>) -> String {
    let modifier = spelled(&entry.modifier);
    match &entry.character {
        None => modifier,
        Some(character) => format!("{modifier}:{}", spelled(character)),
    }
}

/// [`spell_modifier`]'s text, read back.
pub fn read_modifier<V: sts2_core::ContentVersion>(
    text: &str,
) -> Result<RunModifier<V>, serde_json::Error> {
    let (modifier, character) = match text.split_once(':') {
        Some((modifier, character)) => (modifier, Some(character)),
        None => (text, None),
    };
    Ok(RunModifier {
        modifier: read_id(modifier)?,
        character: character.map(read_id).transpose()?,
    })
}

/// The id the game spells `value` with.
pub fn spelled<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(id)) => id,
        other => panic!("{other:?} is not an id"),
    }
}

#[cfg(test)]
mod tests {
    use super::{read_modifier, spell_modifier};
    use sts2_core::RunModifier;

    #[test]
    fn a_modifier_with_a_character_reads_back_as_it_was_spelled() {
        use sts2_content::v0_107_1::{V0_107_1, characters::Character, modifiers::Modifier};

        let entry = RunModifier::<V0_107_1> {
            modifier: Modifier::CharacterCards,
            character: Some(Character::Defect),
        };
        let text = spell_modifier(&entry);
        assert_eq!(text, "CHARACTER_CARDS:DEFECT");
        assert_eq!(read_modifier::<V0_107_1>(&text).ok(), Some(entry));
        let plain = RunModifier::<V0_107_1>::plain(Modifier::Flight);
        assert_eq!(spell_modifier(&plain), "FLIGHT");
    }
}
