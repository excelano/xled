//! The sed-faithful replacement interpreter — the heart of `s///`.
//!
//! No crate does this: the `regex` crate exposes `$1` expansion, but sed's dialect is
//! `\1`–`\9` backrefs, `&` for the whole match, and the `\U \L \u \l \E` case-folding
//! escapes. So xled parses the replacement once into tokens and expands them over the
//! crate's captures, carrying the case-fold state itself.

use regex::{Captures, Regex};

#[derive(Debug, Clone, Copy)]
enum Fold {
    None,
    Upper,
    Lower,
}

#[derive(Debug, Clone)]
enum RepToken {
    Lit(String),
    Group(usize),
    /// `&` — the whole match.
    Whole,
    /// `\U` `\L` `\E` — set the persistent case-fold (None = `\E`).
    SetFold(Fold),
    /// `\u` `\l` — fold just the next emitted character.
    OneFold(Fold),
}

/// A parsed replacement template, ready to expand against any match's captures.
pub struct Replacement {
    tokens: Vec<RepToken>,
}

impl Replacement {
    pub fn parse(s: &str) -> Replacement {
        let mut tokens = Vec::new();
        let mut lit = String::new();
        let mut chars = s.chars().peekable();

        macro_rules! flush {
            () => {
                if !lit.is_empty() {
                    tokens.push(RepToken::Lit(std::mem::take(&mut lit)));
                }
            };
        }

        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some(d @ '1'..='9') => {
                        flush!();
                        tokens.push(RepToken::Group(d as usize - '0' as usize));
                    }
                    Some('U') => {
                        flush!();
                        tokens.push(RepToken::SetFold(Fold::Upper));
                    }
                    Some('L') => {
                        flush!();
                        tokens.push(RepToken::SetFold(Fold::Lower));
                    }
                    Some('E') => {
                        flush!();
                        tokens.push(RepToken::SetFold(Fold::None));
                    }
                    Some('u') => {
                        flush!();
                        tokens.push(RepToken::OneFold(Fold::Upper));
                    }
                    Some('l') => {
                        flush!();
                        tokens.push(RepToken::OneFold(Fold::Lower));
                    }
                    Some('n') => lit.push('\n'),
                    Some('t') => lit.push('\t'),
                    Some(other) => lit.push(other), // \& \\ \/ and any other → literal
                    None => lit.push('\\'),
                },
                '&' => {
                    flush!();
                    tokens.push(RepToken::Whole);
                }
                other => lit.push(other),
            }
        }
        flush!();
        Replacement { tokens }
    }

    fn expand(&self, caps: &Captures) -> String {
        let mut out = String::new();
        let mut persist = Fold::None;
        let mut one: Option<Fold> = None;
        for t in &self.tokens {
            match t {
                RepToken::Lit(s) => push_folded(&mut out, s, persist, &mut one),
                RepToken::Group(n) => {
                    let s = caps.get(*n).map_or("", |m| m.as_str());
                    push_folded(&mut out, s, persist, &mut one);
                }
                RepToken::Whole => {
                    let s = caps.get(0).unwrap().as_str();
                    push_folded(&mut out, s, persist, &mut one);
                }
                RepToken::SetFold(f) => persist = *f,
                RepToken::OneFold(f) => one = Some(*f),
            }
        }
        out
    }
}

fn push_folded(out: &mut String, s: &str, persist: Fold, one: &mut Option<Fold>) {
    for ch in s.chars() {
        let f = one.take().unwrap_or(persist);
        match f {
            Fold::Upper => out.extend(ch.to_uppercase()),
            Fold::Lower => out.extend(ch.to_lowercase()),
            Fold::None => out.push(ch),
        }
    }
}

/// Apply a substitution to one cell. `global` replaces every match, `nth` replaces the
/// Nth match (and, with `global`, from the Nth onward) — sed's count semantics. Custom
/// because the crate's `replacen` is "first N", not "the Nth".
pub fn substitute(
    re: &Regex,
    rep: &Replacement,
    cell: &str,
    global: bool,
    nth: Option<usize>,
) -> String {
    let mut out = String::new();
    let mut last = 0;
    let mut count = 0usize;

    for caps in re.captures_iter(cell) {
        let m = caps.get(0).unwrap();
        count += 1;
        let replace = match (nth, global) {
            (Some(n), true) => count >= n,
            (Some(n), false) => count == n,
            (None, true) => true,
            (None, false) => count == 1,
        };
        if replace {
            out.push_str(&cell[last..m.start()]);
            out.push_str(&rep.expand(&caps));
            last = m.end();
        }
    }
    out.push_str(&cell[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(pat: &str, rep: &str, cell: &str, global: bool, nth: Option<usize>) -> String {
        let re = Regex::new(pat).unwrap();
        substitute(&re, &Replacement::parse(rep), cell, global, nth)
    }

    #[test]
    fn backrefs_and_whole_match() {
        assert_eq!(sub(r"(\d+)", r"[\1]", "abc 42 xyz", false, None), "abc [42] xyz");
        assert_eq!(sub(r"\d+", r"<&>", "x9", false, None), "x<9>");
        // M/D/Y → Y-M-D
        assert_eq!(
            sub(r"(..)/(..)/(....)", r"\3-\1-\2", "01/15/2024", false, None),
            "2024-01-15"
        );
    }

    #[test]
    fn case_folding() {
        assert_eq!(sub(r".*", r"\U&", "abc", false, None), "ABC");
        assert_eq!(sub(r".*", r"\L&", "ABC", false, None), "abc");
        // \u uppercases only the next char, then reverts
        assert_eq!(sub(r"\b(.)", r"\U\1", "dave park", true, None), "Dave Park");
        // \E ends a run
        assert_eq!(sub(r"(.)(.*)", r"\U\1\E\2", "hello", false, None), "Hello");
    }

    #[test]
    fn global_vs_first_vs_nth() {
        assert_eq!(sub("a", "X", "banana", false, None), "bXnana");
        assert_eq!(sub("a", "X", "banana", true, None), "bXnXnX");
        assert_eq!(sub("a", "X", "banana", false, Some(2)), "banXna");
        // nth + global = from nth onward
        assert_eq!(sub("a", "X", "banana", true, Some(2)), "banXnX");
    }

    #[test]
    fn empty_match_fills_blank() {
        assert_eq!(sub("^$", "N/A", "", false, None), "N/A");
        assert_eq!(sub("^$", "N/A", "x", false, None), "x");
    }
}
