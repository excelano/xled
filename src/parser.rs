//! Recursive-descent parser, productions mirroring ebnf.md.
//!
//! The address algebra is parsed by precedence: union (`,`) → intersect (`SP`) → negate (`!`)
//! → primary → range → positional. The reference/command boundary uses maximal munch: the
//! parser consumes the longest leading reference, then one command. Commands are lexically
//! distinct — `=`, `s<delim>`, or a lowercase reserved word — and columns are uppercase, so
//! the two never collide (Disambiguation note 1).

use crate::ast::*;
use crate::errors::{parse, Result, XledError};

/// Reserved command words. Lowercase, but that alone does not separate them from an address:
/// `xaddr` upcases letter runs, so every lowercase word is also legal column letters. What
/// keeps them apart is that a command is matched here by name, and anything else has to
/// resolve against the table (see `resolver::check_columns_in_range`).
const RESERVED: [&str; 8] = [
    "del", "show", "crop", "header", "rename", "fill", "drop", "describe",
];

/// Words that name a capability xled does not have, matched at command position so they
/// reach their catalogued refusal instead of being read as an address. Without this they
/// parse as column letters, resolve past the table, and get the generic out-of-range error —
/// true, but silent about where the capability actually lives.
///
/// Only words the `errors.md` catalog names get an entry; anything else falls through to
/// the out-of-range message. Keep this list and the catalog in step.
const REFUSED: [&str; 12] = [
    "sort",
    "and",
    "or",
    "not",
    "aggregate",
    "group",
    "sum",
    "count",
    "join",
    "split",
    "unpivot",
    "append",
];

/// The catalogued refusal for a word in [`REFUSED`], verbatim from `errors.md`.
fn refusal(word: &str) -> Option<XledError> {
    let e = match word {
        "sort" => XledError::NotInScope(
            "sort is not in xled's scope: xled edits cells in place and never reorders rows. \
             Sort upstream and pipe in — sort -t, -k3 file.csv | xled '…' — or let the \
             query engine do it: duckdb -c \"FROM file.csv ORDER BY price\"."
                .into(),
        ),
        "and" | "or" | "not" => XledError::NotInScope(
            "combining conditions with and/or is not in xled's scope: an address selects rows \
             to edit, it is not a query. For one more condition, run a second xled command on \
             the result; for a real predicate, query first — xql 'SELECT * WHERE qty < reorder \
             AND status = \"active\"' file.csv | xled '…'."
                .into(),
        ),
        "aggregate" | "group" | "sum" | "count" => XledError::NotInScope(
            "aggregation (sum / count / group) is not in xled's scope: expr produces one value \
             per row, never a value across rows. Aggregate in the query engine — \
             duckdb -c \"FROM file.csv SELECT dept, sum(cost) GROUP BY dept\"."
                .into(),
        ),
        "join" => XledError::NotInScope(
            "joining tables is not in xled's scope: xled holds one buffer and never matches \
             rows across files. Join upstream, then scrub the result here — \
             duckdb -c \"FROM a.csv JOIN b.csv USING (id)\" | xled '…'."
                .into(),
        ),
        "split" => XledError::NotSupported(
            "splitting one cell into several columns is not supported: assignment writes one \
             column and never widens the table. For the common two-part case, rearrange in \
             place with s/// — [name] s/(.*), (.*)/\\2 \\1/ — or split upstream: \
             duckdb -c \"FROM file.csv SELECT split_part(name, ', ', 1) AS last, …\"."
                .into(),
        ),
        "unpivot" => XledError::NotSupported(
            "unpivoting (wide → long) is not supported: it changes the table's shape, which \
             xled never does. Reshape in the query engine — duckdb -c \"UNPIVOT file.csv …\"."
                .into(),
        ),
        "append" => XledError::NotAvailableYet(
            "appending a row is not available yet: xled edits existing rows; row generation is \
             still being designed against real cases. For now append upstream — \
             printf 'a,b,c\\n' >> file.csv — then reopen."
                .into(),
        ),
        _ => return None,
    };
    Some(e)
}

/// Parse a whole program: one statement per non-blank line (sequential semantics, rule 9).
pub fn parse_program(input: &str) -> Result<Vec<Statement>> {
    let mut out = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push(parse_statement(line)?);
    }
    Ok(out)
}

fn parse_statement(line: &str) -> Result<Statement> {
    let mut p = Parser::new(line);
    p.skip_spaces();
    let reference = if p.eof() {
        return Err(parse("empty statement"));
    } else if p.at_command_here() {
        None
    } else {
        Some(p.parse_union()?)
    };
    p.skip_spaces();
    let command = if p.eof() {
        None
    } else {
        Some(p.parse_command()?)
    };
    p.skip_spaces();
    if !p.eof() {
        return Err(parse(format!("unexpected trailing input: {:?}", p.rest())));
    }
    Ok(Statement { reference, command })
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(s: &str) -> Self {
        Parser {
            chars: s.chars().collect(),
            pos: 0,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, k: usize) -> Option<char> {
        self.chars.get(self.pos + k).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn rest(&self) -> String {
        self.chars[self.pos..].iter().collect()
    }

    /// Skip a run of spaces; return how many were skipped (a run between atoms = intersection).
    fn skip_spaces(&mut self) -> usize {
        let start = self.pos;
        while self.peek() == Some(' ') {
            self.pos += 1;
        }
        self.pos - start
    }

    // --- reference ---------------------------------------------------------

    fn parse_union(&mut self) -> Result<Reference> {
        let mut parts = vec![self.parse_intersect()?];
        loop {
            let save = self.pos;
            self.skip_spaces();
            if self.peek() == Some(',') {
                self.bump();
                self.skip_spaces();
                parts.push(self.parse_intersect()?);
            } else {
                self.pos = save;
                break;
            }
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Reference::Union(parts)
        })
    }

    fn parse_intersect(&mut self) -> Result<Reference> {
        let mut parts = vec![self.parse_negate()?];
        loop {
            let save = self.pos;
            let spaces = self.skip_spaces();
            // A run of spaces is intersection only if a reference atom follows. If a command
            // or `,` or end follows, the reference is done — restore and let the caller decide.
            if spaces > 0 && !self.at_command_here() && self.starts_ref_atom() {
                parts.push(self.parse_negate()?);
            } else {
                self.pos = save;
                break;
            }
        }
        Ok(if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Reference::Intersect(parts)
        })
    }

    fn parse_negate(&mut self) -> Result<Reference> {
        if self.peek() == Some('!') {
            self.bump();
            Ok(Reference::Negate(Box::new(self.parse_negate()?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Reference> {
        match self.peek() {
            Some('(') => {
                self.bump();
                let inner = self.parse_union()?;
                self.skip_spaces();
                if self.peek() != Some(')') {
                    return Err(parse("expected ')'"));
                }
                self.bump();
                Ok(inner)
            }
            Some('/') => {
                let (body, ci) = self.parse_slash_regex()?;
                Ok(Reference::RegexSel { body, ci })
            }
            _ => {
                // column ↔ comparison disambiguation: if a top-level cmpOp leads (before the
                // assignment `=` or end), this atom is a comparison row-set, not a range.
                if self.has_leading_comparison() {
                    return Ok(Reference::Comparison(self.parse_expr()?));
                }
                let rng = self.parse_range()?;
                // colRegexSel: a lone [name] followed by `~` or `!~` is a column-scoped match.
                if let Some(xaddr::ColRef::Name(name)) = crate::ast::lone_column(&rng) {
                    let neg = self.peek() == Some('!') && self.peek_at(1) == Some('~');
                    if self.peek() == Some('~') || neg {
                        let name = name.clone();
                        if neg {
                            self.bump(); // '!'
                        }
                        self.bump(); // '~'
                        let (body, ci) = self.parse_slash_regex()?;
                        return Ok(Reference::ColRegexSel {
                            col: name,
                            neg,
                            body,
                            ci,
                        });
                    }
                }
                Ok(Reference::Range(rng))
            }
        }
    }

    /// Parse `/body/` with an optional trailing `i`, cursor at the opening `/`.
    fn parse_slash_regex(&mut self) -> Result<(String, bool)> {
        if self.peek() != Some('/') {
            return Err(parse("expected /regex/"));
        }
        self.bump(); // opening '/'
        let mut body = String::new();
        loop {
            match self.peek() {
                None => return Err(parse("unterminated /regex/")),
                Some('\\') => {
                    body.push('\\');
                    self.bump();
                    if let Some(c) = self.bump() {
                        body.push(c);
                    }
                }
                Some('/') => {
                    self.bump();
                    break;
                }
                Some(c) => {
                    body.push(c);
                    self.bump();
                }
            }
        }
        let ci = if self.peek() == Some('i') {
            self.bump();
            true
        } else {
            false
        };
        Ok((body, ci))
    }

    /// Read one positional address, delegating the grammar to `xaddr`.
    ///
    /// An address here has no delimiter — it ends wherever the command that follows begins —
    /// so `parse_prefix` reports how far it got and the cursor advances by exactly that. The
    /// byte count it returns is converted to characters, since this parser indexes by `char`.
    fn parse_range(&mut self) -> Result<xaddr::Spec> {
        let rest = self.rest();
        let (spec, used) = xaddr::parse_prefix(&rest).map_err(|e| parse(e.message))?;
        self.pos += rest[..used].chars().count();
        Ok(spec)
    }

    fn parse_name(&mut self) -> Result<String> {
        self.bump(); // '['
        let mut name = String::new();
        loop {
            match self.peek() {
                None => return Err(parse("unterminated [name]")),
                Some(']') => {
                    if self.peek_at(1) == Some(']') {
                        name.push(']');
                        self.bump();
                        self.bump();
                    } else {
                        self.bump();
                        break;
                    }
                }
                Some(c) => {
                    name.push(c);
                    self.bump();
                }
            }
        }
        Ok(name)
    }

    // --- command -----------------------------------------------------------

    fn parse_command(&mut self) -> Result<Command> {
        if self.peek() == Some('=') {
            self.bump();
            return Ok(Command::Assign(self.parse_expr()?));
        }
        if self.peek() == Some('s') && is_subst_delim(self.peek_at(1)) {
            return self.parse_subst();
        }
        let w = self.read_lower_word();
        match w.as_str() {
            "show" => Ok(Command::Show),
            "del" => Ok(Command::Del),
            "crop" => Ok(Command::Crop),
            "header" => Ok(Command::Header),
            "describe" => Ok(Command::Describe),
            "rename" => {
                self.skip_spaces();
                let name = self.rest();
                self.pos = self.chars.len();
                if name.is_empty() {
                    return Err(parse("rename needs a new name"));
                }
                Ok(Command::Rename(name))
            }
            "fill" => {
                self.skip_spaces();
                match self.read_lower_word().as_str() {
                    "" | "down" => Ok(Command::Fill),
                    other => Err(parse(format!(
                        "fill takes an optional 'down', not {other:?}"
                    ))),
                }
            }
            "drop" => {
                self.skip_spaces();
                if self.read_lower_word() != "blanks" {
                    return Err(parse("drop expects 'blanks' — `drop blanks [rows|cols]`"));
                }
                self.skip_spaces();
                let axis = match self.read_lower_word().as_str() {
                    "" => DropAxis::Both,
                    "rows" => DropAxis::Rows,
                    "cols" => DropAxis::Cols,
                    other => {
                        return Err(parse(format!(
                            "drop blanks takes 'rows' or 'cols', not {other:?}"
                        )))
                    }
                };
                Ok(Command::DropBlanks(axis))
            }
            "" => Err(parse("expected a command")),
            // The capability boundary: sort/join/aggregate/and-or and the rest name a job that
            // lives in a sibling tool. The catalogued message says which one.
            other => match refusal(other) {
                Some(e) => Err(e),
                None => Err(parse(format!("unknown command '{other}'"))),
            },
        }
    }

    /// `s DELIM re DELIM rep DELIM flags` — delimiter is the char after `s`.
    fn parse_subst(&mut self) -> Result<Command> {
        self.bump(); // 's'
        let delim = self.bump().ok_or_else(|| parse("s/// missing delimiter"))?;
        let re = self.read_until_delim(delim)?;
        let rep = self.read_until_delim(delim)?;

        let mut global = false;
        let mut ci = false;
        let mut nth: Option<usize> = None;
        let mut digits = String::new();
        while let Some(c) = self.peek() {
            match c {
                'g' => {
                    global = true;
                    self.bump();
                }
                'i' => {
                    ci = true;
                    self.bump();
                }
                d if d.is_ascii_digit() => {
                    digits.push(d);
                    self.bump();
                }
                ' ' => break,
                other => return Err(parse(format!("unknown s/// flag {other:?}"))),
            }
        }
        if !digits.is_empty() {
            nth = Some(digits.parse().map_err(|_| parse("bad s/// count flag"))?);
        }
        Ok(Command::Subst {
            re,
            rep,
            global,
            ci,
            nth,
        })
    }

    /// Read a sed field up to the closing delimiter. `\<delim>` unescapes to a literal
    /// delimiter; every other `\x` is preserved (it is a regex/replacement escape).
    fn read_until_delim(&mut self, delim: char) -> Result<String> {
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(parse("unterminated s/// — missing closing delimiter")),
                Some('\\') => {
                    if self.peek_at(1) == Some(delim) {
                        out.push(delim);
                        self.bump();
                        self.bump();
                    } else {
                        out.push('\\');
                        self.bump();
                        if let Some(c) = self.bump() {
                            out.push(c);
                        }
                    }
                }
                Some(c) if c == delim => {
                    self.bump();
                    break;
                }
                Some(c) => {
                    out.push(c);
                    self.bump();
                }
            }
        }
        Ok(out)
    }

    fn read_lower_word(&mut self) -> String {
        let mut w = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_lowercase() {
                w.push(c);
                self.bump();
            } else {
                break;
            }
        }
        w
    }

    // --- expr (compute layer) ---------------------------------------------
    // Whitespace is insignificant inside an expr, so these skip spaces freely.

    fn parse_expr(&mut self) -> Result<Expr> {
        let left = self.parse_concat()?;
        // Probe for a comparison operator, but don't eat a trailing intersection space.
        let save = self.pos;
        self.skip_spaces();
        if let Some(op) = self.peek_cmp_op() {
            self.consume_cmp_op(op);
            let right = self.parse_concat()?;
            Ok(Expr::Cmp(op, Box::new(left), Box::new(right)))
        } else {
            self.pos = save;
            Ok(left)
        }
    }

    fn parse_concat(&mut self) -> Result<Expr> {
        let mut e = self.parse_addsub()?;
        loop {
            let save = self.pos;
            self.skip_spaces();
            if self.peek() == Some('&') {
                self.bump();
                let r = self.parse_addsub()?;
                e = Expr::Bin(BinOp::Concat, Box::new(e), Box::new(r));
            } else {
                self.pos = save;
                break;
            }
        }
        Ok(e)
    }

    fn parse_addsub(&mut self) -> Result<Expr> {
        let mut e = self.parse_muldiv()?;
        loop {
            let save = self.pos;
            self.skip_spaces();
            let op = match self.peek() {
                Some('+') => BinOp::Add,
                Some('-') => BinOp::Sub,
                _ => {
                    self.pos = save;
                    break;
                }
            };
            self.bump();
            let r = self.parse_muldiv()?;
            e = Expr::Bin(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_muldiv(&mut self) -> Result<Expr> {
        let mut e = self.parse_unary()?;
        loop {
            let save = self.pos;
            self.skip_spaces();
            let op = match self.peek() {
                Some('*') => BinOp::Mul,
                Some('/') => BinOp::Div,
                _ => {
                    self.pos = save;
                    break;
                }
            };
            self.bump();
            let r = self.parse_unary()?;
            e = Expr::Bin(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        self.skip_spaces();
        if self.peek() == Some('-') {
            self.bump();
            Ok(Expr::Neg(Box::new(self.parse_unary()?)))
        } else {
            self.parse_atom()
        }
    }

    fn parse_atom(&mut self) -> Result<Expr> {
        self.skip_spaces();
        match self.peek() {
            Some('(') => {
                self.bump();
                let e = self.parse_expr()?;
                self.skip_spaces();
                if self.peek() != Some(')') {
                    return Err(parse("expected ')' in expr"));
                }
                self.bump();
                Ok(e)
            }
            Some('"') => Ok(Expr::Str(self.parse_string()?)),
            Some('[') => Ok(Expr::Col(self.parse_name()?)),
            Some(c) if c.is_ascii_digit() => Ok(Expr::Num(self.read_float()?)),
            Some(c) if c.is_ascii_lowercase() => {
                let ident = self.read_lower_word();
                match ident.as_str() {
                    "true" => Ok(Expr::Bool(true)),
                    "false" => Ok(Expr::Bool(false)),
                    _ => {
                        if self.peek() != Some('(') {
                            return Err(parse(format!("expected '(' after function {ident}")));
                        }
                        self.parse_call(ident)
                    }
                }
            }
            other => Err(parse(format!("expected a value in expr, found {other:?}"))),
        }
    }

    fn parse_call(&mut self, name: String) -> Result<Expr> {
        self.bump(); // '('
        let mut args = Vec::new();
        self.skip_spaces();
        if self.peek() == Some(')') {
            self.bump();
            return Ok(Expr::Call(name, args));
        }
        loop {
            args.push(self.parse_expr()?);
            self.skip_spaces();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(')') => {
                    self.bump();
                    break;
                }
                other => {
                    return Err(parse(format!(
                        "expected ',' or ')' in {name}(), found {other:?}"
                    )))
                }
            }
        }
        Ok(Expr::Call(name, args))
    }

    fn parse_string(&mut self) -> Result<String> {
        self.bump(); // opening quote
        let mut s = String::new();
        loop {
            match self.peek() {
                None => return Err(parse("unterminated string literal")),
                Some('\\') if self.peek_at(1) == Some('"') => {
                    s.push('"');
                    self.bump();
                    self.bump();
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some(c) => {
                    s.push(c);
                    self.bump();
                }
            }
        }
        Ok(s)
    }

    fn read_float(&mut self) -> Result<f64> {
        let mut s = String::new();
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            s.push(self.bump().unwrap());
        }
        if self.peek() == Some('.') {
            s.push('.');
            self.bump();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                s.push(self.bump().unwrap());
            }
        }
        s.parse::<f64>()
            .map_err(|_| parse(format!("bad number {s:?}")))
    }

    /// Peek a comparison operator (not the single `=`, which is assignment).
    fn peek_cmp_op(&self) -> Option<CmpOp> {
        match (self.peek(), self.peek_at(1)) {
            (Some('='), Some('=')) => Some(CmpOp::Eq),
            (Some('!'), Some('=')) => Some(CmpOp::Ne),
            (Some('<'), Some('=')) => Some(CmpOp::Le),
            (Some('>'), Some('=')) => Some(CmpOp::Ge),
            (Some('<'), _) => Some(CmpOp::Lt),
            (Some('>'), _) => Some(CmpOp::Gt),
            _ => None,
        }
    }

    fn consume_cmp_op(&mut self, op: CmpOp) {
        let two = matches!(op, CmpOp::Eq | CmpOp::Ne | CmpOp::Le | CmpOp::Ge);
        self.bump();
        if two {
            self.bump();
        }
    }

    /// Scan ahead for a top-level comparison operator before the assignment `=`, a union
    /// `,`, a closing `)`, a command word, or end — the column↔comparison decision.
    fn has_leading_comparison(&self) -> bool {
        let mut i = self.pos;
        let mut depth = 0i32;
        let n = self.chars.len();
        while i < n {
            let c = self.chars[i];
            match c {
                '"' => {
                    // skip string literal
                    i += 1;
                    while i < n && self.chars[i] != '"' {
                        i += 1;
                    }
                }
                '/' if depth == 0 => {
                    // skip a /regex/ body so its contents don't trip the scan
                    i += 1;
                    while i < n && self.chars[i] != '/' {
                        if self.chars[i] == '\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                '[' | '(' => depth += 1,
                ']' | ')' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                _ if depth == 0 => {
                    let next = self.chars.get(i + 1).copied();
                    if matches!(c, '<' | '>') {
                        return true;
                    }
                    if c == '=' && next == Some('=') {
                        return true;
                    }
                    if c == '!' && next == Some('=') {
                        return true;
                    }
                    if c == '=' {
                        return false; // assignment boundary
                    }
                    if c == ',' {
                        return false;
                    }
                    if c == 's' && is_subst_delim(self.chars.get(i + 1).copied()) {
                        // a substitute command `s<delim>…` ends the reference: its pattern and
                        // replacement may contain `<`/`>`, which are not comparisons in scope.
                        return false;
                    }
                    if c.is_ascii_lowercase() {
                        // a reserved command word ends the reference; a function name (→ '(') does not
                        let mut j = i;
                        let mut w = String::new();
                        while j < n && self.chars[j].is_ascii_lowercase() {
                            w.push(self.chars[j]);
                            j += 1;
                        }
                        let boundary = j >= n || self.chars[j] == ' ';
                        if RESERVED.contains(&w.as_str()) && boundary {
                            return false;
                        }
                        i = j;
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        false
    }

    // --- lookahead helpers -------------------------------------------------

    /// True if the cursor (no skipping) sits at the start of a command.
    fn at_command_here(&self) -> bool {
        match self.peek() {
            None => false,
            Some('=') => true,
            Some('s') if is_subst_delim(self.peek_at(1)) => true,
            Some(c) if c.is_ascii_lowercase() => self.matches_reserved_word(),
            _ => false,
        }
    }

    /// True if a lowercase reserved word starts here and ends at a word boundary.
    fn matches_reserved_word(&self) -> bool {
        let mut i = self.pos;
        let mut w = String::new();
        while i < self.chars.len() && self.chars[i].is_ascii_lowercase() {
            w.push(self.chars[i]);
            i += 1;
        }
        if !RESERVED.contains(&w.as_str()) && !REFUSED.contains(&w.as_str()) {
            return false;
        }
        // boundary: a reserved word is followed by a space (then its args) or end of line.
        i >= self.chars.len() || self.chars[i] == ' '
    }

    /// True if the cursor begins a reference atom.
    fn starts_ref_atom(&self) -> bool {
        match self.peek() {
            Some(c) => {
                c.is_ascii_uppercase()
                    || c.is_ascii_digit()
                    || matches!(c, '[' | '/' | '$' | '(' | '!' | ':')
            }
            None => false,
        }
    }
}

/// A sed `s///` delimiter is the char right after `s`: any non-alphanumeric that is not a
/// reference operator. That's how column `S`/`SK` (followed by space/`,`/`:`/end) never
/// reads as substitute.
fn is_subst_delim(c: Option<char>) -> bool {
    match c {
        Some(c) => {
            !c.is_alphanumeric() && !c.is_whitespace() && !matches!(c, ',' | ':' | '(' | ')' | '!')
        }
        None => false,
    }
}
