//! A bitset over a generated enum, in the two shapes the domains need.
//!
//! Every domain grows one sooner or later: a card's keywords, a monster's
//! intents, the families an encounter fields, the card types an enchantment can
//! be put on. They arrived independently and came out the same, which is the
//! point — the shape is forced rather than chosen.
//!
//! # Two shapes, and the difference is real
//!
//! [`plain`] is one word with a `contains`. It is right when the member either
//! is or is not in the set, which is most of them.
//!
//! [`computed`] is two words and answers with a `Field<bool>`. It is right when
//! some of the underlying members are `virtual` and a few models override them,
//! so a member has three answers and not two.
//!
//! # Why the second shape needs two words
//!
//! A single bitset would have to spell "computed" as either true or false, and
//! there is no honest choice between them. So the set carries the answers and
//! the computed-ness separately, and the generated `get` hands back a `Field<bool>` —
//! which means there is no way to ask the question that cannot be answered.
//! There is deliberately no `contains`: it would be the lie this exists to
//! avoid.
//!
//! # What a domain still owns
//!
//! The member names and their docs, and the prose on the types. Those say what
//! the members *are*, which is the part that differs; everything here says how
//! a set of them is spelled, which is the part that does not.
//!
//! A set is over the whole enum, not over the members one caller happens to
//! use. An enchantment can only be put on three of the six card types, and
//! `CardTypeSet` still spans all six — narrowing the type to its first caller
//! would make the second one wrong.

use std::fmt::Write as _;

use super::DERIVES;

/// One set to emit, in either shape.
pub struct Set<'a> {
    /// The member enum's name, e.g. `PowerFlag`.
    pub flag: &'a str,
    /// The set's name, e.g. `PowerFlags`.
    pub set: &'a str,
    /// What the member enum's doc says, beyond the first line.
    ///
    /// Empty when the enum is not emitted here — a set over an enum another
    /// domain already declares, like `CardTypeSet` over `CardType`.
    pub flag_doc: &'a str,
    /// What the set's doc says.
    pub set_doc: &'a str,
    /// The members in bit order, each with its own doc.
    pub members: &'a [(&'a str, &'a str)],
    /// Whether to emit the member enum, or only the set over it.
    pub declare: bool,
    /// The highest bit the set can use, which is the highest discriminant of
    /// any member — not the member count.
    ///
    /// The two differ whenever an enum has a hole in it. `CardKeyword` drops its
    /// zero and runs 1..=7, so seven members reach bit 7 and the word has to
    /// hold eight bits rather than seven. Sizing from the count would be right
    /// by luck there and wrong the first time a domain drops a member in the
    /// middle.
    pub bits: usize,
}

impl Set<'_> {
    /// The narrowest word that holds every bit.
    ///
    /// The same decision `repr` makes and for the same reason: the set sits
    /// inside a data struct, so a `u16` that only ever needs eight bits costs
    /// two bytes in every row of the table.
    fn word(&self) -> Result<&'static str, String> {
        match self.bits {
            n if n < 8 => Ok("u8"),
            n if n < 16 => Ok("u16"),
            n if n < 32 => Ok("u32"),
            n => Err(format!(
                "{} needs bit {n}, which is more than a u32 holds",
                self.set
            )),
        }
    }
}

/// Emits a set whose members may be computed: two words, and `get`.
pub fn computed(out: &mut String, set: &Set<'_>) -> Result<(), String> {
    let word = set.word()?;
    let (flag, name) = (set.flag, set.set);

    declare(out, set);

    for line in set.set_doc.lines() {
        writeln!(out, "///{}", pad(line)).unwrap();
    }
    writeln!(
        out,
        "#[derive(Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct {name} {{\n\
         \x20   /// The flags that are set. A computed flag is never in here.\n\
         \x20   yes: {word},\n\
         \x20   /// The flags the model decides at the time.\n\
         \x20   computed: {word},\n\
         }}\n\
         \n\
         impl {name} {{\n\
         \x20   /// Every flag, in bit order.\n\
         \x20   const MEMBERS: [{flag}; {n}] = [{members}];\n\
         \n\
         \x20   /// The flags a model sets, and the ones it computes.\n\
         \x20   ///\n\
         \x20   /// A flag in both lists is a contradiction, and this is a `const\n\
         \x20   /// fn` called from a `const`, so saying so fails the build rather\n\
         \x20   /// than the run.\n\
         \x20   pub const fn new(yes: &[{flag}], computed: &[{flag}]) -> Self {{\n\
         \x20       let mut set = 0{word};\n\
         \x20       let mut i = 0;\n\
         \x20       while i < yes.len() {{\n\
         \x20           set |= 1{word} << yes[i] as u8;\n\
         \x20           i += 1;\n\
         \x20       }}\n\
         \n\
         \x20       let mut asked = 0{word};\n\
         \x20       i = 0;\n\
         \x20       while i < computed.len() {{\n\
         \x20           asked |= 1{word} << computed[i] as u8;\n\
         \x20           i += 1;\n\
         \x20       }}\n\
         \n\
         \x20       assert!(set & asked == 0, \"a flag cannot be both set and computed\");\n\
         \x20       Self {{ yes: set, computed: asked }}\n\
         \x20   }}\n\
         \n\
         \x20   /// One flag: the answer, or the cue to ask for it.\n\
         \x20   ///\n\
         \x20   /// There is no `contains`. A computed flag has no answer until\n\
         \x20   /// something asks the model, and a `bool` would have to invent\n\
         \x20   /// one.\n\
         \x20   pub const fn get(self, flag: {flag}) -> Field<bool> {{\n\
         \x20       let bit = 1{word} << flag as u8;\n\
         \x20       if self.computed & bit != 0 {{\n\
         \x20           return Field::Computed;\n\
         \x20       }}\n\
         \x20       Field::Constant(self.yes & bit != 0)\n\
         \x20   }}\n\
         }}\n\
         \n\
         impl std::fmt::Debug for {name} {{\n\
         \x20   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n\
         \x20       let mut set = f.debug_set();\n\
         \x20       let mut i = 0;\n\
         \x20       while i < Self::MEMBERS.len() {{\n\
         \x20           let flag = Self::MEMBERS[i];\n\
         \x20           match self.get(flag) {{\n\
         \x20               Field::Constant(true) => {{\n\
         \x20                   set.entry(&flag);\n\
         \x20               }}\n\
         \x20               Field::Computed => {{\n\
         \x20                   set.entry(&format_args!(\"{{flag:?}}: computed\"));\n\
         \x20               }}\n\
         \x20               Field::Constant(false) => {{}}\n\
         \x20           }}\n\
         \x20           i += 1;\n\
         \x20       }}\n\
         \x20       set.finish()\n\
         \x20   }}\n\
         }}\n",
        n = set.members.len(),
        members = set
            .members
            .iter()
            .map(|(m, _)| format!("{flag}::{m}"))
            .collect::<Vec<_>>()
            .join(", "),
    )
    .unwrap();

    Ok(())
}

/// Emits a set whose members are simply in it or not: one word, and `contains`.
///
/// The bits are the member's own discriminant, so a bit means the same thing
/// here as it does wherever the enum came from — which matters when the enum is
/// another domain's and the numbers are the game's.
pub fn plain(out: &mut String, set: &Set<'_>) -> Result<(), String> {
    let word = set.word()?;
    let (flag, name) = (set.flag, set.set);

    declare(out, set);

    for line in set.set_doc.lines() {
        writeln!(out, "///{}", pad(line)).unwrap();
    }
    writeln!(
        out,
        "#[derive(Clone, Copy, PartialEq, Eq, Hash)]\n\
         pub struct {name}({word});\n\
         \n\
         impl {name} {{\n\
         \x20   /// Every member, in the enum's own order.\n\
         \x20   const MEMBERS: [{flag}; {n}] = [{members}];\n\
         \n\
         \x20   pub const fn new(members: &[{flag}]) -> Self {{\n\
         \x20       let mut bits = 0{word};\n\
         \x20       let mut i = 0;\n\
         \x20       while i < members.len() {{\n\
         \x20           bits |= 1{word} << members[i] as u8;\n\
         \x20           i += 1;\n\
         \x20       }}\n\
         \x20       Self(bits)\n\
         \x20   }}\n\
         \n\
         \x20   pub const fn contains(self, member: {flag}) -> bool {{\n\
         \x20       self.0 & (1{word} << member as u8) != 0\n\
         \x20   }}\n\
         \n\
         \x20   pub const fn is_empty(self) -> bool {{\n\
         \x20       self.0 == 0\n\
         \x20   }}\n\
         \n\
         \x20   /// Whether the two sets share a member.\n\
         \x20   pub const fn intersects(self, other: {name}) -> bool {{\n\
         \x20       self.0 & other.0 != 0\n\
         \x20   }}\n\
         \n\
         \x20   /// The members, in the enum's own order.\n\
         \x20   pub fn iter(self) -> impl Iterator<Item = {flag}> {{\n\
         \x20       Self::MEMBERS.into_iter().filter(move |&m| self.contains(m))\n\
         \x20   }}\n\
         }}\n\
         \n\
         impl std::fmt::Debug for {name} {{\n\
         \x20   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n\
         \x20       f.debug_set().entries(self.iter()).finish()\n\
         \x20   }}\n\
         }}\n",
        n = set.members.len(),
        members = set
            .members
            .iter()
            .map(|(m, _)| format!("{flag}::{m}"))
            .collect::<Vec<_>>()
            .join(", "),
    )
    .unwrap();

    Ok(())
}

/// The member enum, where the set declares one.
///
/// A set over another domain's enum declares nothing: `CardTypeSet` is a set
/// over `cards::CardType`, and a second `CardType` beside it would be the
/// duplicate this module exists to avoid.
fn declare(out: &mut String, set: &Set<'_>) {
    if !set.declare {
        return;
    }

    writeln!(out, "/// A yes-or-no fact about a {}.", article(set.set)).unwrap();
    for line in set.flag_doc.lines() {
        writeln!(out, "///{}", pad(line)).unwrap();
    }
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum {} {{", set.flag).unwrap();
    for (bit, (member, doc)) in set.members.iter().enumerate() {
        for line in doc.lines() {
            writeln!(out, "    ///{}", pad(line.trim())).unwrap();
        }
        writeln!(out, "    {member} = {bit},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();
}

/// `PowerFlags` describes a power, `RelicFlags` a relic.
fn article(set: &str) -> String {
    set.trim_end_matches("Flags").to_lowercase()
}

/// A doc line, with the space only a non-empty line gets.
///
/// rustfmt strips a trailing space anyway, but the generated file reads better
/// without one and a blank doc line is `///` rather than `/// `.
fn pad(line: &str) -> String {
    if line.is_empty() {
        String::new()
    } else {
        format!(" {line}")
    }
}
