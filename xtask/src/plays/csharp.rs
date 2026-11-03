//! Just enough of C# to split a decompiled method into statements.
//!
//! The decompiler writes one consistent dialect: every block braced, and every
//! statement ended by a semicolon or a block. So a statement is read as its
//! tokens and nothing finer. What a statement means is decided by matching
//! those tokens, and anything this reads wrongly fails to match.

/// One token of source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// A name or a keyword.
    Ident(String),
    /// A number as written, suffix included, such as `3m` or `0.2f`.
    Num(String),
    /// A string literal's contents, escapes left as written.
    Str(String),
    /// An operator or a bracket.
    Punct(&'static str),
}

impl Tok {
    pub fn is(&self, text: &str) -> bool {
        match self {
            Tok::Ident(name) => name == text,
            Tok::Punct(punct) => *punct == text,
            Tok::Num(_) | Tok::Str(_) => false,
        }
    }

    pub fn ident(&self) -> Option<&str> {
        match self {
            Tok::Ident(name) => Some(name),
            _ => None,
        }
    }
}

/// Operators, longest first, so `?.` is never read as `?` then `.`.
const PUNCTS: [&str; 34] = [
    "??=", "<<=", ">>=", "?.", "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "++", "--", "+=",
    "-=", "*=", "/=", "|=", "&=", "(", ")", "{", "}", "[", "]", "<", ">", ".", ",", ";", "=", "?",
    ":",
];

/// Operators a single character spells, which `PUNCTS` leaves out.
const SINGLE: [&str; 9] = ["!", "+", "-", "*", "/", "%", "&", "|", "^"];

/// What the reader does not follow in a class's source.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("unexpected character '{0}'")]
    Character(char),
    #[error("{0} opens nothing")]
    OpensNothing(String),
    #[error("an unclosed '{0}'")]
    Unclosed(&'static str),
    #[error("a branch without braces")]
    NoBraces,
    #[error("a statement that closes more than it opens")]
    OverClosed,
    #[error("a statement with no end: {0}")]
    NoEnd(String),
    #[error("{0} has no block body")]
    NoBlockBody(String),
}

/// The tokens of `source`, or the character it stopped at.
pub fn tokens(source: &str) -> Result<Vec<Tok>, Error> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
            continue;
        }
        if c == '/' && chars.get(at + 1) == Some(&'/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            continue;
        }
        if c == '"' || (matches!(c, '@' | '$') && chars.get(at + 1) == Some(&'"')) {
            let start = if c == '"' { at + 1 } else { at + 2 };
            let mut end = start;
            while end < chars.len() && chars[end] != '"' {
                if chars[end] == '\\' {
                    end += 1;
                }
                end += 1;
            }
            out.push(Tok::Str(
                chars[start..end.min(chars.len())].iter().collect(),
            ));
            at = end + 1;
            continue;
        }
        if c == '\'' {
            let mut end = at + 1;
            while end < chars.len() && chars[end] != '\'' {
                if chars[end] == '\\' {
                    end += 1;
                }
                end += 1;
            }
            out.push(Tok::Str(
                chars[at + 1..end.min(chars.len())].iter().collect(),
            ));
            at = end + 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '.') {
                // A member access on a number never occurs, so a dot is a
                // decimal point.
                at += 1;
            }
            out.push(Tok::Num(chars[start..at].iter().collect()));
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '@' {
            let start = at;
            at += 1;
            while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            out.push(Tok::Ident(chars[start..at].iter().collect()));
            continue;
        }
        let rest: String = chars[at..(at + 3).min(chars.len())].iter().collect();
        match PUNCTS
            .iter()
            .chain(SINGLE.iter())
            .find(|punct| rest.starts_with(**punct))
        {
            Some(punct) => {
                out.push(Tok::Punct(punct));
                at += punct.len();
            }
            None => return Err(Error::Character(c)),
        }
    }
    Ok(out)
}

/// One statement of a method body.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// Everything up to and including its semicolon.
    Simple(Vec<Tok>),
    /// `if`, with the other branch when there is one. An `else if` is an
    /// other branch holding one `if`.
    If {
        cond: Vec<Tok>,
        then: Vec<Stmt>,
        other: Vec<Stmt>,
    },
    /// `foreach`, with what its parentheses hold.
    Foreach { head: Vec<Tok>, body: Vec<Stmt> },
    /// Any other statement that owns a block, kept whole.
    Other(Vec<Tok>),
}

impl Stmt {
    /// What the statement is, named and not quoted: its keyword, or the
    /// first method it calls, such as `call PowerCmd.Apply`. A dump carries
    /// this in place of the statement, so it holds none of the game's code.
    pub fn head(&self) -> String {
        let toks = match self {
            Stmt::If { .. } => return "if".to_owned(),
            Stmt::Foreach { .. } => return "foreach".to_owned(),
            Stmt::Other(toks) => match toks.first() {
                Some(Tok::Ident(keyword)) => return keyword.clone(),
                _ => return "block".to_owned(),
            },
            Stmt::Simple(toks) => toks,
        };
        // A dotted name, its type arguments passed over, right before `(`.
        let mut chain: Vec<&str> = Vec::new();
        let mut at = 0;
        while at < toks.len() {
            match &toks[at] {
                Tok::Ident(name)
                    if !chain.is_empty() && (toks[at - 1].is(".") || toks[at - 1].is("?.")) =>
                {
                    chain.push(name);
                }
                Tok::Ident(name) => chain = vec![name],
                Tok::Punct("." | "?.") if !chain.is_empty() => {}
                Tok::Punct("<") if !chain.is_empty() => {
                    let mut depth = 0;
                    while at < toks.len() {
                        if toks[at].is("<") {
                            depth += 1;
                        } else if toks[at].is(">") {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        at += 1;
                    }
                }
                Tok::Punct("(") if !chain.is_empty() => {
                    let named: Vec<&str> = chain
                        .iter()
                        .copied()
                        .skip_while(|part| matches!(*part, "base" | "this"))
                        .collect();
                    return format!("call {}", named.join("."));
                }
                _ => chain.clear(),
            }
            at += 1;
        }
        "statement".to_owned()
    }

    /// The statement as its tokens, spaced, for a person to read.
    pub fn text(&self) -> String {
        match self {
            Stmt::Simple(toks) | Stmt::Other(toks) => spell(toks),
            Stmt::If { cond, .. } => format!("if ({}) {{ ... }}", spell(cond)),
            Stmt::Foreach { head, .. } => format!("foreach ({}) {{ ... }}", spell(head)),
        }
    }
}

/// Tokens as source text, spaced only between two words and around a binary
/// operator.
pub fn spell(toks: &[Tok]) -> String {
    let mut out = String::new();
    let mut last_word = false;
    for tok in toks {
        let (text, word) = match tok {
            Tok::Ident(name) => (name.clone(), true),
            Tok::Num(num) => (num.clone(), true),
            Tok::Str(text) => (format!("\"{text}\""), true),
            Tok::Punct(punct) => (punct.to_string(), false),
        };
        let binary = matches!(
            tok,
            Tok::Punct("=" | "=>" | "==" | "!=" | "&&" | "||" | "+=" | "-=" | "??")
        );
        if (last_word && word) || binary {
            out.push(' ');
        }
        out.push_str(&text);
        if binary || tok.is(",") {
            out.push(' ');
        }
        last_word = word;
    }
    out
}

/// Where the bracket opening at `open` closes.
fn closing(toks: &[Tok], open: usize) -> Result<usize, Error> {
    let (up, down) = match &toks[open] {
        Tok::Punct("(") => ("(", ")"),
        Tok::Punct("{") => ("{", "}"),
        Tok::Punct("[") => ("[", "]"),
        tok => return Err(Error::OpensNothing(format!("{tok:?}"))),
    };
    let mut depth = 0;
    for (at, tok) in toks.iter().enumerate().skip(open) {
        if tok.is(up) {
            depth += 1;
        } else if tok.is(down) {
            depth -= 1;
            if depth == 0 {
                return Ok(at);
            }
        }
    }
    Err(Error::Unclosed(up))
}

/// The statements of a block's contents.
pub fn statements(toks: &[Tok]) -> Result<Vec<Stmt>, Error> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < toks.len() {
        let (stmt, next) = statement(toks, at)?;
        out.push(stmt);
        at = next;
    }
    Ok(out)
}

/// A block's statements, from the brace at `open`, and where it ends.
fn block(toks: &[Tok], open: usize) -> Result<(Vec<Stmt>, usize), Error> {
    if !toks.get(open).is_some_and(|tok| tok.is("{")) {
        return Err(Error::NoBraces);
    }
    let close = closing(toks, open)?;
    Ok((statements(&toks[open + 1..close])?, close + 1))
}

/// The statement starting at `at`, and where the next one starts.
fn statement(toks: &[Tok], at: usize) -> Result<(Stmt, usize), Error> {
    let head = &toks[at];
    if head.is("if") {
        let close = closing(toks, at + 1)?;
        let cond = toks[at + 2..close].to_vec();
        let (then, mut next) = block(toks, close + 1)?;
        let mut other = Vec::new();
        if toks.get(next).is_some_and(|tok| tok.is("else")) {
            if toks.get(next + 1).is_some_and(|tok| tok.is("if")) {
                let (nested, after) = statement(toks, next + 1)?;
                other.push(nested);
                next = after;
            } else {
                let (body, after) = block(toks, next + 1)?;
                other = body;
                next = after;
            }
        }
        return Ok((Stmt::If { cond, then, other }, next));
    }
    if head.is("foreach") {
        let close = closing(toks, at + 1)?;
        let (body, next) = block(toks, close + 1)?;
        let head = toks[at + 2..close].to_vec();
        return Ok((Stmt::Foreach { head, body }, next));
    }
    if [
        "for", "while", "using", "lock", "switch", "try", "do", "checked", "unsafe",
    ]
    .iter()
    .any(|keyword| head.is(keyword))
        && !toks.get(at + 1).is_some_and(|tok| tok.is("var"))
    {
        // Parentheses, then a block, then whatever blocks hang off it.
        let mut next = at + 1;
        if toks.get(next).is_some_and(|tok| tok.is("(")) {
            next = closing(toks, next)? + 1;
        }
        if toks.get(next).is_some_and(|tok| tok.is("{")) {
            next = closing(toks, next)? + 1;
            while toks
                .get(next)
                .is_some_and(|tok| tok.is("catch") || tok.is("finally") || tok.is("while"))
            {
                next += 1;
                if toks.get(next).is_some_and(|tok| tok.is("(")) {
                    next = closing(toks, next)? + 1;
                }
                if toks.get(next).is_some_and(|tok| tok.is("{")) {
                    next = closing(toks, next)? + 1;
                }
            }
            if toks.get(next).is_some_and(|tok| tok.is(";")) {
                next += 1;
            }
            return Ok((Stmt::Other(toks[at..next].to_vec()), next));
        }
    }
    // A plain statement runs to the semicolon outside every bracket.
    let mut depth = 0i32;
    for (end, tok) in toks.iter().enumerate().skip(at) {
        match tok {
            Tok::Punct("(" | "{" | "[") => depth += 1,
            Tok::Punct(")" | "}" | "]") => depth -= 1,
            Tok::Punct(";") if depth == 0 => {
                return Ok((Stmt::Simple(toks[at..=end].to_vec()), end + 1));
            }
            _ => {}
        }
        if depth < 0 {
            return Err(Error::OverClosed);
        }
    }
    Err(Error::NoEnd(spell(&toks[at..])))
}

/// The body of the method `name` overrides in a class's source, as tokens.
///
/// `None` when the class does not override it.
pub fn method(class: &[Tok], name: &str) -> Result<Option<Vec<Tok>>, Error> {
    for at in 0..class.len() {
        if !(class[at].is("override") && class.get(at + 1).is_some()) {
            continue;
        }
        // `override [async] <type> <name> (`.
        let Some(open) = (at..class.len().min(at + 8)).find(|&i| class[i].is("(")) else {
            continue;
        };
        if !class[open - 1].is(name) {
            continue;
        }
        let close = closing(class, open)?;
        if !class.get(close + 1).is_some_and(|tok| tok.is("{")) {
            return Err(Error::NoBlockBody(name.to_owned()));
        }
        let end = closing(class, close + 1)?;
        return Ok(Some(class[close + 2..end].to_vec()));
    }
    Ok(None)
}

/// The body of the method `name` a class declares, whatever its modifiers,
/// as tokens. `None` when the class declares none by that name.
pub fn body(class: &[Tok], name: &str) -> Result<Option<Vec<Tok>>, Error> {
    for at in 1..class.len() {
        // `<type> name (`, which a call never is: a call follows `.`, `(`, an
        // operator or `await`.
        if !(class[at].is(name) && class.get(at + 1).is_some_and(|tok| tok.is("("))) {
            continue;
        }
        if !matches!(&class[at - 1], Tok::Ident(ty) if ty != "await" && ty != "return" && ty != "new")
            && !class[at - 1].is(">")
        {
            continue;
        }
        let close = closing(class, at + 1)?;
        if !class.get(close + 1).is_some_and(|tok| tok.is("{")) {
            continue;
        }
        let end = closing(class, close + 1)?;
        return Ok(Some(class[close + 2..end].to_vec()));
    }
    Ok(None)
}

/// Whether a class overrides the member `name`, method or property.
pub fn overrides(class: &[Tok], name: &str) -> bool {
    class.iter().enumerate().any(|(at, tok)| {
        tok.is("override")
            && class[at + 1..]
                .iter()
                .take_while(|tok| !(tok.is("(") || tok.is("=>") || tok.is("{") || tok.is(";")))
                .any(|tok| tok.is(name))
    })
}

/// The class a class declares it derives from, which is the first name after
/// the colon in its declaration.
pub fn base_class(class: &[Tok], name: &str) -> Option<String> {
    let at = class
        .windows(2)
        .position(|pair| pair[0].is("class") && pair[1].is(name))?;
    class[at + 2..]
        .iter()
        .take_while(|tok| !tok.is("{"))
        .skip_while(|tok| !tok.is(":"))
        .nth(1)
        .and_then(|tok| tok.ident())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_splits_into_its_statements() {
        let toks = tokens(
            "Guard.NotNull(play.Aim, \"play.Aim\");\n\
             Node n = Screen.Instance?.NodeFor(play.Aim);\n\
             if (n != null) { Sound.Play(\"x\"); } else if (a) { b(); } else { c(); }\n\
             foreach (Thing e in things) { Effect.PlayOn(e, \"v\"); }\n\
             await ExampleCmd.Hit(base.Vars.Amount.Value).From(this)\
             .At(play.Aim).Run(context);",
        )
        .unwrap();
        let stmts = statements(&toks).unwrap();
        assert_eq!(stmts.len(), 5);
        assert!(matches!(&stmts[2], Stmt::If { other, .. } if matches!(other[0], Stmt::If { .. })));
        assert!(matches!(&stmts[3], Stmt::Foreach { body, .. } if body.len() == 1));
        assert_eq!(
            stmts[4].text(),
            "await ExampleCmd.Hit(base.Vars.Amount.Value).From(this)\
             .At(play.Aim).Run(context);"
        );
    }

    #[test]
    fn a_statement_is_named_by_what_it_calls() {
        let toks = tokens(
            "Guard.NotNull(play.Aim, \"play.Aim\");\n\
             Node n = Screen.Instance?.NodeFor(play.Aim);\n\
             if (n != null) { Sound.Play(\"x\"); }\n\
             foreach (Thing e in things) { Effect.PlayOn(e, \"v\"); }\n\
             await ExampleCmd.Hit<Thing>(base.Vars.Amount.Value).From(this);\n\
             base.Vars.Amount.Raise(1m);\n\
             n = m;",
        )
        .unwrap();
        let heads: Vec<String> = statements(&toks).unwrap().iter().map(Stmt::head).collect();
        assert_eq!(
            heads,
            [
                "call Guard.NotNull",
                "call Screen.Instance.NodeFor",
                "if",
                "foreach",
                "call ExampleCmd.Hit",
                "call Vars.Amount.Raise",
                "statement",
            ]
        );
    }

    #[test]
    fn a_method_is_found_by_its_override() {
        let toks = tokens(
            "public sealed class MadeUpCard : CardModel { \
             protected override async Task OnPlay(Context context, Play play) \
             { await ExampleCmd.Gain(base.Vars.Amount.Value, base.Owner); } \
             protected override void OnUpgrade() { base.Vars.Amount.Raise(1m); } }",
        )
        .unwrap();
        assert_eq!(
            base_class(&toks, "MadeUpCard").as_deref(),
            Some("CardModel")
        );
        let upgrade = method(&toks, "OnUpgrade").unwrap().unwrap();
        assert_eq!(spell(&upgrade), "base.Vars.Amount.Raise(1m);");
        assert!(method(&toks, "OnTurnEndInHand").unwrap().is_none());
    }
}
