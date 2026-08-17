//! A set of yes-or-no facts, where some of them are worked out at the time.
//!
//! Powers and relics both have one. They arrived independently and came out the
//! same, which is the point: the shape is forced rather than chosen. A domain
//! has a run of boolean columns, some of the underlying members are `virtual`
//! and a few models override them, and a flag therefore has three answers and
//! not two.
//!
//! # Why two words
//!
//! A single bitset would have to spell "computed" as either true or false, and
//! there is no honest choice between them. So the set carries the answers and
//! the computed-ness separately, and the generated `get` hands back a `Field<bool>` —
//! which means there is no way to ask the question that cannot be answered.
//! There is deliberately no `contains`: it would be the lie this exists to
//! avoid.
//!
//! An encounter's `FlagSet` is one word and does have `contains`, because
//! nothing computes those three. That is a different type for a real reason,
//! not a copy that drifted.
//!
//! # What a domain still owns
//!
//! The member names and their docs, and the prose on the two types. Those say
//! what the flags *are*, which is the part that differs; everything below says
//! how a set of them is spelled, which is the part that does not.

use std::fmt::Write as _;

use super::DERIVES;

/// One flag set to emit.
pub struct Set<'a> {
    /// The member enum's name, e.g. `PowerFlag`.
    pub flag: &'a str,
    /// The set's name, e.g. `PowerFlags`.
    pub set: &'a str,
    /// What the member enum's doc says, beyond the first line.
    pub flag_doc: &'a str,
    /// What the set's doc says.
    pub set_doc: &'a str,
    /// The members in bit order, each with its own doc.
    pub members: &'a [(&'a str, &'a str)],
}

impl Set<'_> {
    /// The narrowest word that holds every bit.
    ///
    /// The same decision `repr` makes and for the same reason: the set sits
    /// inside a data struct twice over, so a `u16` that only ever needs eight
    /// bits costs two bytes in every row of the table.
    fn word(&self) -> Result<&'static str, String> {
        match self.members.len() {
            n if n <= 8 => Ok("u8"),
            n if n <= 16 => Ok("u16"),
            n if n <= 32 => Ok("u32"),
            n => Err(format!("{n} flags is more than a u32 can hold")),
        }
    }
}

/// Emits the member enum and the set beside it.
pub fn emit(out: &mut String, set: &Set<'_>) -> Result<(), String> {
    let word = set.word()?;
    let (flag, name) = (set.flag, set.set);

    writeln!(out, "/// A yes-or-no fact about a {}.", article(name)).unwrap();
    for line in set.flag_doc.lines() {
        writeln!(out, "///{}", pad(line)).unwrap();
    }
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum {flag} {{").unwrap();
    for (bit, (member, doc)) in set.members.iter().enumerate() {
        for line in doc.lines() {
            writeln!(out, "    ///{}", pad(line.trim())).unwrap();
        }
        writeln!(out, "    {member} = {bit},").unwrap();
    }
    writeln!(out, "}}\n").unwrap();

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
