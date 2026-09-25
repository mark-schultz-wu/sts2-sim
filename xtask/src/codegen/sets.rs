//! A bitset over a generated enum, in the two shapes the domains need.
//!
//! Every domain grows one sooner or later. A card's keywords, a monster's
//! intents, the families an encounter fields, the card types an enchantment can
//! be put on. They arrived independently and came out the same, which is the
//! point. The shape is forced rather than chosen.
//!
//! The xtask README says how the two shapes differ, why the computed one needs
//! two words, and what a domain still owns.

use std::fmt::Write as _;

use super::DERIVES;

/// A set whose members cannot share one word.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{set}: {first} and {second} are both bit {bit}")]
    SharedBit {
        set: String,
        first: String,
        second: String,
        bit: usize,
    },
    #[error("{set} needs bit {bit}, which is more than a u32 holds")]
    Wide { set: String, bit: usize },
}

/// A method a set gets only where a caller wants it.
///
/// `new`, `contains`, `iter` and `Debug` are the core, and every set has them:
/// build one, ask it, walk it, print it. These two are not. Handing them out
/// anyway gives four sets an `intersects` nothing calls.
#[derive(PartialEq, Eq)]
pub enum Extra {
    IsEmpty,
    Intersects,
}

/// One set to emit, in either shape.
pub struct Set<'a> {
    /// The member enum's name, e.g. `PowerFlag`.
    pub flag: &'a str,
    /// The set's name, e.g. `PowerFlags`.
    pub set: &'a str,
    /// What the member enum's doc says, beyond the first line.
    ///
    /// Empty when the enum is not emitted here. That is a set over an enum
    /// another domain already declares, like `CardTypeSet` over `CardType`.
    pub flag_doc: &'a str,
    /// What the set's doc says.
    pub set_doc: &'a str,
    /// The members, each at the bit it occupies.
    pub members: &'a [Member<'a>],
    /// Whether to emit the member enum, or only the set over it.
    pub declare: bool,
    /// Methods beyond the core, emitted only where something calls them.
    pub extras: &'a [Extra],
}

/// One member of a set, and the bit it occupies.
pub struct Member<'a> {
    pub name: &'a str,
    /// The bit, which is also the discriminant where the set declares the enum.
    ///
    /// Not the member's position. The two differ whenever an enum has a hole in
    /// it. `CardKeyword` drops its zero and runs 1..=7, so seven members reach
    /// bit 7 and the word has to hold eight bits. Numbering by position is
    /// right by luck there, and wrong the first time a domain drops a member in
    /// the middle.
    pub bit: usize,
    pub doc: &'a str,
}

/// Members at 0, 1, 2 …, for an enum this module declares.
///
/// A domain's flags are columns in its dump and not a game enum. There is no
/// numbering to inherit, and position is the only one available.
pub fn counted<'a>(members: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<Member<'a>> {
    members
        .into_iter()
        .enumerate()
        .map(|(bit, (name, doc))| Member { name, bit, doc })
        .collect()
}

impl Set<'_> {
    /// The highest bit any member occupies.
    fn bits(&self) -> usize {
        self.members.iter().map(|m| m.bit).max().unwrap_or(0)
    }

    /// The narrowest word that holds every bit, once the members are sound.
    ///
    /// The width is the same decision `repr` makes. A `u16` needing eight bits
    /// costs two bytes in every row of the table.
    ///
    /// The soundness is that no two members share a bit. Two flags on one bit
    /// answer for each other, and that is the mistake a domain numbering its
    /// own members can make.
    fn word(&self) -> Result<&'static str, Error> {
        for (i, m) in self.members.iter().enumerate() {
            if let Some(other) = self.members[..i].iter().find(|o| o.bit == m.bit) {
                return Err(Error::SharedBit {
                    set: self.set.to_owned(),
                    first: other.name.to_owned(),
                    second: m.name.to_owned(),
                    bit: m.bit,
                });
            }
        }

        match self.bits() {
            n if n < 8 => Ok("u8"),
            n if n < 16 => Ok("u16"),
            n if n < 32 => Ok("u32"),
            bit => Err(Error::Wide {
                set: self.set.to_owned(),
                bit,
            }),
        }
    }
}

/// Emits a set whose members can be computed: two words, and `get`.
pub fn computed(out: &mut String, set: &Set<'_>) -> Result<(), Error> {
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
            .map(|m| format!("{flag}::{}", m.name))
            .collect::<Vec<_>>()
            .join(", "),
    )
    .unwrap();

    Ok(())
}

/// Emits a set whose members are in it or not: one word, and `contains`.
///
/// The bits are the member's own discriminant, so a bit means the same thing
/// here as it does wherever the enum came from. That matters when the enum is
/// another domain's and the numbers are the game's.
pub fn plain(out: &mut String, set: &Set<'_>) -> Result<(), Error> {
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
{extras}\
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
         }}\n\
         \n\
         impl sts2_core::FlagSet for {name} {{\n\
         \x20   type Member = {flag};\n\
         \n\
         \x20   const MEMBERS: &'static [{flag}] = &Self::MEMBERS;\n\
         \n\
         \x20   fn from_members(members: &[{flag}]) -> Self {{\n\
         \x20       Self::new(members)\n\
         \x20   }}\n\
         \n\
         \x20   fn contains(self, member: {flag}) -> bool {{\n\
         \x20       Self::contains(self, member)\n\
         \x20   }}\n\
         }}\n",
        n = set.members.len(),
        members = set
            .members
            .iter()
            .map(|m| format!("{flag}::{}", m.name))
            .collect::<Vec<_>>()
            .join(", "),
        extras = extras(set, word, name),
    )
    .unwrap();

    Ok(())
}

/// The optional methods, in a fixed order so two sets asking for both agree.
fn extras(set: &Set<'_>, word: &str, name: &str) -> String {
    let mut out = String::new();

    if set.extras.contains(&Extra::IsEmpty) {
        writeln!(out, "    pub const fn is_empty(self) -> bool {{").unwrap();
        writeln!(out, "        self.0 == 0{word}").unwrap();
        writeln!(out, "    }}\n").unwrap();
    }

    if set.extras.contains(&Extra::Intersects) {
        writeln!(out, "    /// Whether the two sets share a member.").unwrap();
        writeln!(
            out,
            "    pub const fn intersects(self, other: {name}) -> bool {{"
        )
        .unwrap();
        writeln!(out, "        self.0 & other.0 != 0").unwrap();
        writeln!(out, "    }}\n").unwrap();
    }

    out
}

/// The member enum, where the set declares one.
///
/// A set over another domain's enum declares nothing. `CardTypeSet` is a set
/// over `cards::CardType`, and a second `CardType` beside it is the duplicate
/// this module exists to avoid.
fn declare(out: &mut String, set: &Set<'_>) {
    if !set.declare {
        return;
    }

    let noun = noun(set.set);
    writeln!(out, "/// A yes-or-no fact about {} {noun}.", article(&noun)).unwrap();
    for line in set.flag_doc.lines() {
        writeln!(out, "///{}", pad(line)).unwrap();
    }
    writeln!(out, "{DERIVES}").unwrap();
    writeln!(out, "#[repr(u8)]").unwrap();
    writeln!(out, "pub enum {} {{", set.flag).unwrap();
    for member in set.members {
        for line in member.doc.lines() {
            writeln!(out, "    ///{}", pad(line.trim())).unwrap();
        }
        writeln!(out, "    {} = {},", member.name, member.bit).unwrap();
    }
    writeln!(out, "}}\n").unwrap();
}

/// `PowerFlags` describes a power, `RelicFlags` a relic.
fn noun(set: &str) -> String {
    set.trim_end_matches("Flags").to_lowercase()
}

/// A power, but an affliction and an enchantment.
///
/// Crude, because it reads the letter rather than the sound. But every noun
/// here is a kind of game content, and none of them is a "hour" or a "user".
fn article(noun: &str) -> &'static str {
    match noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        true => "an",
        false => "a",
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `members` members at distinct bits, the highest of which is `bits`.
    ///
    /// Distinct is not a choice here. Bits are what a set is made of, so a
    /// member count above `bits + 1` describes nothing that exists.
    fn set(members: usize, bits: usize) -> Vec<Member<'static>> {
        assert!(
            members <= bits + 1,
            "{members} members do not fit under bit {bits}"
        );
        let mut all = counted(std::iter::repeat_n(("M", ""), members));
        all.last_mut().unwrap().bit = bits;
        all
    }

    fn width(members: &[Member<'_>]) -> Result<&'static str, Error> {
        Set {
            flag: "Member",
            set: "MemberSet",
            flag_doc: "",
            set_doc: "",
            members,
            declare: false,
            extras: &[],
        }
        .word()
    }

    /// The width comes from the highest bit, not the member count.
    ///
    /// The two agree on every enum in the build, so the generated tree cannot
    /// tell the rules apart. That is why this is here. They come apart the
    /// first time a domain drops a member in the middle of an enum. Sizing from
    /// the count then silently truncates the set.
    #[test]
    fn the_word_holds_the_highest_bit_and_not_the_member_count() {
        // Seven members reaching bit 7 — `CardKeyword`, which drops its zero.
        // The count says six bits and the truth says eight.
        assert_eq!(width(&set(7, 7)).unwrap(), "u8");

        // The case that would break: few members, high discriminants. Two
        // members and a `u32`, where counting would say one byte.
        assert_eq!(width(&set(2, 9)).unwrap(), "u16");
        assert_eq!(width(&set(2, 20)).unwrap(), "u32");

        // The boundaries themselves, from either side.
        assert_eq!(width(&set(1, 0)).unwrap(), "u8");
        assert_eq!(width(&set(8, 7)).unwrap(), "u8");
        assert_eq!(width(&set(2, 8)).unwrap(), "u16");
        assert_eq!(width(&set(2, 15)).unwrap(), "u16");
        assert_eq!(width(&set(2, 16)).unwrap(), "u32");
    }

    /// Past a `u32` is an error rather than a truncation.
    #[test]
    fn a_set_too_wide_to_hold_fails_generation() {
        let err = width(&set(3, 32)).unwrap_err();
        assert!(
            matches!(&err, Error::Wide { set, bit: 32 } if set == "MemberSet"),
            "the error names the set and the bit it could not hold: {err}"
        );
    }

    /// Two members on one bit is the mistake explicit numbering allows.
    ///
    /// `counted` cannot make it, but a domain reading bits out of a dump can.
    /// The result is two flags that answer for each other.
    #[test]
    fn two_members_on_one_bit_fail_generation() {
        let members = [
            Member {
                name: "Early",
                bit: 3,
                doc: "",
            },
            Member {
                name: "Late",
                bit: 3,
                doc: "",
            },
        ];
        let err = width(&members).unwrap_err();
        assert!(
            matches!(&err, Error::SharedBit { first, second, bit: 3, .. } if first == "Early" && second == "Late"),
            "{err}"
        );
    }
}
