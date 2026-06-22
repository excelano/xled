//! The parsed forms: `Statement { reference?, command? }`, the reference tree, and commands.
//!
//! Mirrors the EBNF productions in ebnf.md. Slice 1 populates the addressing tree and the
//! `show` command; later slices add subst/assign/word variants and the expr tree.

/// A single addressable position: a cell, a whole column, a row, a named column, or `$`.
#[derive(Debug, Clone)]
pub enum Positional {
    /// A1-style cell, e.g. `B2` — column index + 1-based row as written.
    Cell { col: usize, row: usize },
    /// A whole column by letter, e.g. `C` — 0-based index.
    Column(usize),
    /// A whole row by number, e.g. `3` — 1-based as written.
    Row(usize),
    /// A whole column by bracketed name, e.g. `[price]`.
    Name(String),
    /// The last data row, `$`.
    LastRow,
}

/// A range or a lone positional. The four `:` forms collapse here:
/// `pos` (is_range=false), `pos:pos`, `pos:` (end=None), `:pos` (start=None).
#[derive(Debug, Clone)]
pub struct RangeRef {
    pub start: Option<Positional>,
    pub end: Option<Positional>,
    pub is_range: bool,
}

/// The address tree. Precedence (low→high): `,` union < `SP` intersect < `:` range < `!` negate.
#[derive(Debug, Clone)]
pub enum Reference {
    Union(Vec<Reference>),
    Intersect(Vec<Reference>),
    Negate(Box<Reference>),
    Range(RangeRef),
    /// `/re/` — rows where any cell matches.
    RegexSel { body: String, ci: bool },
    /// `[col]~/re/` (or `!~`) — rows where a named column matches (or doesn't).
    ColRegexSel {
        col: String,
        neg: bool,
        body: String,
        ci: bool,
    },
    /// A single comparison as scope — rows where the (bool-valued) expr is true.
    Comparison(Expr),
}

/// The compute layer (RHS of `=`, operands of a comparison). Columns are always bracketed.
#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Str(String),
    Bool(bool),
    /// `[name]` — this row's value in that column (a string, until cast).
    Col(String),
    /// Unary minus.
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    /// A comparison yields a bool.
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    /// `&` — string concatenation.
    Concat,
}

#[derive(Debug, Clone, Copy)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// A command. Slices 1–2 implement `show` and `s///`; the rest land in later slices.
#[derive(Debug, Clone)]
pub enum Command {
    Show,
    /// `s DELIM re DELIM rep DELIM flags` — the headline substitute.
    Subst {
        re: String,
        rep: String,
        global: bool,
        ci: bool,
        /// Nth-occurrence flag (sed's numeric flag).
        nth: Option<usize>,
    },
    /// `= expr` — compute a value into exactly one column (creating it if new).
    Assign(Expr),
    /// `del` — drop whole rows or whole columns.
    Del,
    /// `crop` — make the scoped rectangle the working table.
    Crop,
    /// `header` — promote the scoped row to the name overlay.
    Header,
    /// `rename NAME` — rename the scoped column (rest-of-line is the new name).
    Rename(String),
    /// `fill` / `fill down` — forward-fill empty cells from the value above.
    Fill,
    /// `drop blanks [rows|cols]` — trim fully-empty edge rows and/or columns.
    DropBlanks(DropAxis),
    /// `describe` — advisory region report; never mutates.
    Describe,
}

#[derive(Debug, Clone, Copy)]
pub enum DropAxis {
    Both,
    Rows,
    Cols,
}

/// One line of xled: an optional address paired with an optional command.
/// Both omitted is impossible; reference-only ⇒ implicit show; command-only ⇒ whole-table scope.
#[derive(Debug, Clone)]
pub struct Statement {
    pub reference: Option<Reference>,
    pub command: Option<Command>,
}
