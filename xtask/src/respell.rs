//! The shared behaviour, as one build spells its content.
//!
//! The rules in `sts2_content::behaviour` are generic over the build, so
//! they name content through associated consts, `V::Power::WEAK_POWER`, and
//! match it with guards, `__c0 if __c0 == V::Card::BASH =>`. `coverage`
//! reads content names and match arms out of the source. This turns the
//! shared source back into the build's own spelling, `Power::WeakPower` and
//! `IroncladCard::Bash =>`, by way of the build's impls in `ids.rs`.

use std::collections::BTreeMap;

/// Each content const's spelling in one build, by the associated type it is
/// named through and its name: `("Power", "WEAK_POWER")` is
/// `Power::WeakPower`.
pub(crate) struct Spellings(BTreeMap<(String, String), String>);

impl Spellings {
    /// The spellings of the build whose module is `module`, such as
    /// `v0_107_1`, read out of `ids.rs`.
    pub(crate) fn of(ids: &str, module: &str) -> Spellings {
        let head = format!(" for crate::{module}::");
        let path = format!("crate::{module}::");
        let mut spelt = BTreeMap::new();
        for block in ids.split("\nimpl ").skip(1) {
            let Some(at) = block.find(&head) else {
                continue;
            };
            let rest = &block[at + head.len()..];
            let Some(ty) = rest
                .split_once(" {")
                .and_then(|(ty, _)| ty.rsplit("::").next())
            else {
                continue;
            };
            let assoc = if ty == "Cards" { "Card" } else { ty };
            // A const rustfmt wrapped spans lines, so the block is read as
            // one line of `;`-ended items.
            let body = block.split_once('{').map_or("", |(_, body)| body);
            let joined: String = body.lines().map(str::trim).collect::<Vec<_>>().join(" ");
            for item in joined.split(';') {
                let Some(item) = item.trim().strip_prefix("const ") else {
                    continue;
                };
                let Some((name, value)) = item.split_once(": Self = ") else {
                    continue;
                };
                let value = value.trim().replace(&path, "");
                let value = strip_modules(&value);
                let value = match (assoc, value.split_once('(')) {
                    // A card is named by its pool's own enum.
                    ("Card", Some((_, inner))) => inner.trim_end_matches(')').to_owned(),
                    _ => value.replacen("Self::", &format!("{ty}::"), 1),
                };
                spelt.insert((assoc.to_owned(), name.trim().to_owned()), value);
            }
        }
        Spellings(spelt)
    }

    /// `source`, with every content const and guard arm spelled as the
    /// build spells it.
    pub(crate) fn respell(&self, source: &str) -> String {
        unlet(&unguard(&self.names(source)))
    }

    /// Every `V::Kind::CONST`, and every `<V::Kind as crate::ids::KindIds>::
    /// CONST`, as the build's own path.
    fn names(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len());
        let mut rest = source;
        loop {
            let plain = rest.find("V::");
            let qualified = rest.find("<V::");
            let at = match (plain, qualified) {
                (Some(p), Some(q)) if q < p => q,
                (Some(p), _) => p,
                (None, Some(q)) => q,
                (None, None) => break,
            };
            // `V` alone, not the tail of a longer name.
            let alone = at == 0
                || !rest[..at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            if let Some((len, spelt)) = alone.then(|| self.name_at(&rest[at..])).flatten() {
                out.push_str(&rest[..at]);
                out.push_str(spelt);
                rest = &rest[at + len..];
            } else {
                let step = at + 1;
                out.push_str(&rest[..step]);
                rest = &rest[step..];
            }
        }
        out.push_str(rest);
        out
    }

    /// The const named at the start of `text`, as how long its name runs
    /// and how the build spells it.
    fn name_at<'a>(&'a self, text: &str) -> Option<(usize, &'a str)> {
        let (kind, after) = if let Some(rest) = text.strip_prefix("<V::") {
            let (kind, rest) = rest.split_once(" as ")?;
            let (_, rest) = rest.split_once(">::")?;
            (kind, rest)
        } else {
            let rest = text.strip_prefix("V::")?;
            rest.split_once("::")?
        };
        let end = after
            .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
            .unwrap_or(after.len());
        let name = &after[..end];
        let spelt = self.0.get(&(kind.to_owned(), name.to_owned()))?;
        let len = text.len() - after.len() + end;
        Some((len, spelt))
    }
}

/// `value` without its module segments, the lowercase ones, so
/// `Self::Ironclad(cards::IroncladCard::Bash)` is
/// `Self::Ironclad(IroncladCard::Bash)`.
fn strip_modules(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find("::") {
        let start = rest[..at]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(0, |i| i + 1);
        let module = rest[start..at].starts_with(|c: char| c.is_ascii_lowercase());
        out.push_str(&rest[..if module { start } else { at + 2 }]);
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// Every guard arm that only compares its bindings to content, as the
/// pattern it stands for: `__c0 if __c0 == A || __c0 == B =>` is `A | B =>`,
/// and `(__c0, __c1) if (__c0 == A && __c1 == B) =>` is `(A, B) =>`.
fn unguard(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(at) = rest.find(" if ") {
        let (before, after) = rest.split_at(at);
        let Some(arrow) = after.find("=>") else {
            break;
        };
        let mut line_start = before.rfind('\n').map_or(0, |n| n + 1);
        // rustfmt puts a long guard on a line of its own, under its pattern.
        if before[line_start..].trim().is_empty() && line_start > 0 {
            line_start = before[..line_start - 1].rfind('\n').map_or(0, |n| n + 1);
        }
        let pattern = before[line_start..].trim();
        let condition = &after[" if ".len()..arrow];
        match pattern_for(pattern, condition) {
            Some(spelt) => {
                let indent =
                    &before[line_start..before.len() - before[line_start..].trim_start().len()];
                out.push_str(&before[..line_start]);
                out.push_str(indent);
                out.push_str(&spelt);
                out.push(' ');
                rest = &after[arrow..];
            }
            None => {
                out.push_str(&rest[..at + " if ".len()]);
                rest = &rest[at + " if ".len()..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Every let-chain that binds and only compares the bindings to content, as
/// the pattern it stands for: `if let (__c0, __c1) = (a, b) && __c0 == A &&
/// __c1 == B {` is `if let (A, B) = (a, b) {`.
fn unlet(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(at) = rest.find("if let ") {
        let start = at + "if let ".len();
        let after = &rest[start..];
        let spelt = after.split_once(" = ").and_then(|(pattern, tail)| {
            let brace = tail.find("\n{").or_else(|| tail.find(" {"))?;
            let (scrutinee, condition) = tail[..brace].split_once("&&")?;
            let spelt = pattern_for(pattern.trim(), condition)?;
            let len = pattern.len() + " = ".len() + brace;
            Some((len, format!("{spelt} = {} ", scrutinee.trim())))
        });
        match spelt {
            Some((len, spelt)) => {
                out.push_str(&rest[..start]);
                out.push_str(&spelt);
                rest = &rest[start + len..];
                rest = rest.trim_start_matches(['\n', ' ']);
            }
            None => {
                out.push_str(&rest[..start]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The pattern a guard arm stands for, when its pattern is bindings and its
/// guard only compares them to content.
fn pattern_for(pattern: &str, condition: &str) -> Option<String> {
    let bindings: Vec<&str> = match pattern.strip_prefix('(') {
        Some(tuple) => tuple.strip_suffix(')')?.split(',').map(str::trim).collect(),
        None => vec![pattern],
    };
    if !bindings.iter().any(|b| b.starts_with("__c")) {
        return None;
    }
    let mut alternatives: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for conjunct in split_top(strip_parens(condition.trim()), "&&") {
        for alternative in split_top(strip_parens(conjunct.trim()), "||") {
            let (binding, value) = alternative.trim().split_once(" == ")?;
            let value: String = value.split_whitespace().collect();
            if !binding.starts_with("__c") || value.contains(' ') {
                return None;
            }
            alternatives.entry(binding).or_default().push(value);
        }
    }
    let items: Vec<String> = bindings
        .iter()
        .map(|binding| match alternatives.get(binding) {
            Some(values) => values.join(" | "),
            None => (*binding).to_owned(),
        })
        .collect();
    Some(match items.as_slice() {
        [one] if bindings.len() == 1 && !pattern.starts_with('(') => one.clone(),
        _ => format!("({})", items.join(", ")),
    })
}

/// `text` without parentheses that wrap the whole of it.
fn strip_parens(text: &str) -> &str {
    let mut text = text;
    while let Some(inner) = text.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        // Only when the first parenthesis closes at the end.
        let mut depth = 0;
        let wraps = inner.chars().all(|c| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            depth >= 0
        });
        if !wraps {
            break;
        }
        text = inner.trim();
    }
    text
}

/// `text` split at each `op` outside parentheses.
fn split_top<'a>(text: &'a str, op: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ if depth == 0 && text[at..].starts_with(op) => {
                parts.push(&text[start..at]);
                at += op.len();
                start = at;
                continue;
            }
            _ => {}
        }
        at += 1;
    }
    parts.push(&text[start..]);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: &str = "
impl PowerIds for crate::v0_107_1::powers::Power {
    const WEAK_POWER: Self = Self::WeakPower;
}

impl CardIds for crate::v0_107_1::cards::Cards {
    const BASH: Self = Self::Ironclad(crate::v0_107_1::cards::IroncladCard::Bash);
    const BLOOD_WALL: Self =
        Self::Ironclad(crate::v0_107_1::cards::IroncladCard::BloodWall);
}

impl MonsterIds for crate::v0_107_1::monsters::Monster {
    const NIBBIT: Self = Self::Nibbit;
}

impl MoveIds for crate::v0_107_1::monsters::MoveId {
    const BUTT: Self = Self::Butt;
    const SLICE: Self = Self::Slice;
}
";

    #[test]
    fn a_const_is_the_build_s_own_path() {
        let spelt = Spellings::of(IDS, "v0_107_1");
        assert_eq!(
            spelt.respell(
                "apply(V::Power::WEAK_POWER); <V::Card as crate::ids::CardIds>::BLOOD_WALL"
            ),
            "apply(Power::WeakPower); IroncladCard::BloodWall"
        );
    }

    #[test]
    fn a_guard_arm_is_its_pattern() {
        let spelt = Spellings::of(IDS, "v0_107_1");
        let source = "    match card {\n        __c0 if __c0 == V::Card::BASH || __c0 == V::Card::BLOOD_WALL => x,\n";
        assert_eq!(
            spelt.respell(source),
            "    match card {\n        IroncladCard::Bash | IroncladCard::BloodWall => x,\n"
        );
        let pair = "        (__c0, __c1)\n            if (__c0 == V::Monster::NIBBIT\n                && (__c1 == V::MoveId::BUTT || __c1 == V::MoveId::SLICE)) =>\n";
        assert_eq!(
            spelt.respell(pair),
            "        (Monster::Nibbit, MoveId::Butt | MoveId::Slice) =>\n"
        );
    }

    #[test]
    fn a_let_chain_is_its_pattern() {
        let spelt = Spellings::of(IDS, "v0_107_1");
        let source = "    if let (__c0, __c1) = (monster, move_id)\n        && __c0 == V::Monster::NIBBIT\n        && (__c1 == V::MoveId::BUTT\n            || __c1 == V::MoveId::SLICE)\n    {\n        go();\n";
        assert_eq!(
            spelt.respell(source),
            "    if let (Monster::Nibbit, MoveId::Butt | MoveId::Slice) = (monster, move_id) {\n        go();\n"
        );
    }

    #[test]
    fn a_guard_with_more_than_content_stays() {
        let spelt = Spellings::of(IDS, "v0_107_1");
        let source = "        __c0 if __c0 == V::Card::BASH && ready => x,\n";
        assert_eq!(
            spelt.respell(source),
            "        __c0 if __c0 == IroncladCard::Bash && ready => x,\n"
        );
    }
}
