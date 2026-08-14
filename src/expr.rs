//! The compute layer: evaluate an `Expr` against one row to a typed `Value`.
//!
//! Four types (string/number/bool/date), no auto-coercion — casts are explicit (`num`, `bool`,
//! `date`), the property that keeps leading zeros safe. A cast failure is non-halting: it
//! surfaces as `EvalErr::Cast`, which the caller turns into "leave the cell, tally a warning"
//! (rule 6). Comparisons are string-wise unless both sides are numbers or both are dates (the
//! A3 footgun, by design).

use crate::ast::{BinOp, CmpOp, Expr};
use crate::date;
use crate::errors::XledError;
use crate::model::Buffer;
use crate::subst::{self, Replacement};
use chrono::{Datelike, NaiveDate};
use regex::Regex;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
    Date(NaiveDate),
}

/// A non-halting cast failure (skip the cell, tally) vs a halting program error.
#[derive(Debug)]
pub enum EvalErr {
    Cast,
    Hard(XledError),
}

impl Value {
    /// Serialize back to a cell string. Integral numbers print without a decimal point.
    ///
    /// Numbers use `f64`'s shortest round-tripping form, so fractional arithmetic can leak
    /// representation artifacts (`… * 1.1` → `…000004`). This is deliberate: rounding on
    /// write would invent precision the user didn't ask for, betraying the stringly model.
    /// Money/fixed-decimal columns must wrap the value in `round(…, d)` (see expr-grammar.md).
    ///
    /// Dates write as ISO 8601. That is what makes normalizing a column fall out of the cast
    /// alone — `[hired] = date([hired], "DD/MM/YYYY")` needs no formatting call — and it is
    /// also the one serialization that sorts correctly as plain text afterwards.
    pub fn into_string(self) -> String {
        match self {
            Value::Str(s) => s,
            Value::Num(n) => format!("{n}"),
            Value::Bool(b) => b.to_string(),
            Value::Date(d) => d.format("%Y-%m-%d").to_string(),
        }
    }

    fn as_string(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Num(n) => format!("{n}"),
            Value::Bool(b) => b.to_string(),
            Value::Date(d) => d.format("%Y-%m-%d").to_string(),
        }
    }
}

pub fn eval(buf: &Buffer, row: usize, e: &Expr) -> Result<Value, EvalErr> {
    match e {
        Expr::Num(n) => Ok(Value::Num(*n)),
        Expr::Str(s) => Ok(Value::Str(s.clone())),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Col(name) => {
            let c = buf.name_to_col(name).ok_or_else(|| {
                EvalErr::Hard(XledError::Correction(format!("no column named [{name}]")))
            })?;
            Ok(Value::Str(buf.cell(row, c).to_string()))
        }
        Expr::Neg(inner) => {
            let n = require_num(&eval(buf, row, inner)?)?;
            Ok(Value::Num(-n))
        }
        Expr::Bin(op, a, b) => {
            let a = eval(buf, row, a)?;
            let b = eval(buf, row, b)?;
            eval_bin(*op, a, b)
        }
        Expr::Cmp(op, a, b) => {
            let a = eval(buf, row, a)?;
            let b = eval(buf, row, b)?;
            Ok(Value::Bool(eval_cmp(*op, &a, &b)))
        }
        Expr::Call(name, args) => eval_call(buf, row, name, args),
    }
}

fn eval_bin(op: BinOp, a: Value, b: Value) -> Result<Value, EvalErr> {
    if let BinOp::Concat = op {
        return Ok(Value::Str(format!("{}{}", a.as_string(), b.as_string())));
    }
    // Date arithmetic is matched ahead of the numeric requirement below, because a date is
    // not a number: subtracting two of them yields a day count, and offsetting one by a
    // number moves it by days. Anything else involving a date (date + date, number − date)
    // falls through and fails the require_num check, which is the right answer — it has no
    // meaning to reach for.
    match (op, &a, &b) {
        (BinOp::Sub, Value::Date(x), Value::Date(y)) => {
            return Ok(Value::Num(x.signed_duration_since(*y).num_days() as f64));
        }
        (BinOp::Add, Value::Date(d), Value::Num(n))
        | (BinOp::Add, Value::Num(n), Value::Date(d)) => {
            return offset_days(*d, *n);
        }
        (BinOp::Sub, Value::Date(d), Value::Num(n)) => return offset_days(*d, -*n),
        _ => {}
    }
    // Arithmetic requires numbers already — no auto-coercion of strings (use num()).
    let x = require_num(&a)?;
    let y = require_num(&b)?;
    let r = match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div => {
            if y == 0.0 {
                return Err(EvalErr::Cast); // #DIV/0! — leave the cell
            }
            x / y
        }
        BinOp::Concat => unreachable!(),
    };
    Ok(Value::Num(r))
}

fn eval_cmp(op: CmpOp, a: &Value, b: &Value) -> bool {
    // Numeric order only when both are already numbers, chronological only when both are
    // already dates; otherwise lexical (string-wise). A date compared against a string still
    // orders correctly whenever that string is ISO, which is the point of serializing to ISO.
    let ord = match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y),
        (Value::Date(x), Value::Date(y)) => Some(x.cmp(y)),
        _ => Some(a.as_string().cmp(&b.as_string())),
    };
    match ord {
        Some(Ordering::Less) => matches!(op, CmpOp::Lt | CmpOp::Le | CmpOp::Ne),
        Some(Ordering::Equal) => matches!(op, CmpOp::Eq | CmpOp::Le | CmpOp::Ge),
        Some(Ordering::Greater) => matches!(op, CmpOp::Gt | CmpOp::Ge | CmpOp::Ne),
        None => matches!(op, CmpOp::Ne), // NaN: only != holds
    }
}

thread_local! {
    /// Compiled patterns, keyed by their source text. An expression runs once per row, and a
    /// pattern is nearly always a literal, so compiling per row would pay the whole cost of
    /// the regex engine on every line of the file. Keyed by string rather than cached on the
    /// AST node because the pattern is an ordinary argument — it may be a column, and then it
    /// genuinely varies per row, which a per-node cache would get wrong.
    static REGEX_CACHE: RefCell<HashMap<String, Regex>> = RefCell::new(HashMap::new());
}

/// Compile `pattern`, reusing an earlier compilation of the same text.
///
/// A bad pattern halts rather than tallying a skip. A cast failure is per-cell and says
/// something about that row's data; an unparsable regex is wrong on every row, so stopping and
/// showing the engine's own message is the useful answer (rule 6's line).
fn compiled(pattern: &str) -> Result<Regex, EvalErr> {
    REGEX_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(re) = c.get(pattern) {
            return Ok(re.clone());
        }
        let re = Regex::new(pattern).map_err(|e| {
            EvalErr::Hard(XledError::Correction(format!("bad regex {pattern:?}: {e}")))
        })?;
        c.insert(pattern.to_string(), re.clone());
        Ok(re)
    })
}

fn eval_call(buf: &Buffer, row: usize, name: &str, args: &[Expr]) -> Result<Value, EvalErr> {
    let argc = args.len();
    let want = |n: usize| -> Result<(), EvalErr> {
        if argc == n {
            Ok(())
        } else {
            Err(EvalErr::Hard(XledError::Correction(format!(
                "{name}() takes {n} argument(s), got {argc}"
            ))))
        }
    };

    match name {
        "num" => {
            want(1)?;
            Ok(Value::Num(cast_num(&eval(buf, row, &args[0])?)?))
        }
        "bool" => {
            want(1)?;
            Ok(Value::Bool(cast_bool(&eval(buf, row, &args[0])?)?))
        }
        "len" => {
            want(1)?;
            let s = eval(buf, row, &args[0])?.as_string();
            Ok(Value::Num(s.chars().count() as f64))
        }
        "left" => {
            want(2)?;
            let s = eval(buf, row, &args[0])?.as_string();
            let n = arg_usize(buf, row, &args[1])?;
            Ok(Value::Str(s.chars().take(n).collect()))
        }
        "right" => {
            want(2)?;
            let s = eval(buf, row, &args[0])?.as_string();
            let n = arg_usize(buf, row, &args[1])?;
            let len = s.chars().count();
            Ok(Value::Str(s.chars().skip(len.saturating_sub(n)).collect()))
        }
        "mid" => {
            want(3)?;
            let s = eval(buf, row, &args[0])?.as_string();
            let start = arg_usize(buf, row, &args[1])?.max(1);
            let n = arg_usize(buf, row, &args[2])?;
            Ok(Value::Str(s.chars().skip(start - 1).take(n).collect()))
        }
        "substr" => {
            if argc != 2 && argc != 3 {
                return Err(EvalErr::Hard(XledError::Correction(
                    "substr() takes 2 or 3 arguments".into(),
                )));
            }
            let s = eval(buf, row, &args[0])?.as_string();
            let start = arg_usize(buf, row, &args[1])?.max(1);
            let chars = s.chars().skip(start - 1);
            let out: String = if argc == 3 {
                let n = arg_usize(buf, row, &args[2])?;
                chars.take(n).collect()
            } else {
                chars.collect() // 2-arg: to end
            };
            Ok(Value::Str(out))
        }
        "round" => {
            want(2)?;
            let x = cast_num(&eval(buf, row, &args[0])?)?;
            let d = arg_usize(buf, row, &args[1])?;
            let f = 10f64.powi(d as i32);
            Ok(Value::Num((x * f).round() / f))
        }
        "default" => {
            want(2)?;
            let x = eval(buf, row, &args[0])?;
            if x.as_string().is_empty() {
                eval(buf, row, &args[1])
            } else {
                Ok(x)
            }
        }
        "coalesce" => {
            if argc == 0 {
                return Err(EvalErr::Hard(XledError::Correction(
                    "coalesce() needs at least one argument".into(),
                )));
            }
            for a in args {
                let v = eval(buf, row, a)?;
                if !v.as_string().is_empty() {
                    return Ok(v);
                }
            }
            Ok(Value::Str(String::new()))
        }
        "if" => {
            want(3)?;
            let cond = cast_bool(&eval(buf, row, &args[0])?)?;
            eval(buf, row, if cond { &args[1] } else { &args[2] })
        }
        // Set membership. The subject is compared against literals, which is what separates
        // this from the alternation it replaces: `^(?:APP|CAM)$` matches substrings when the
        // anchors are forgotten and compiles its own members when one carries a metacharacter
        // (`R+D` is the one value such a pattern will not match). Neither failure is available
        // to a comparison.
        //
        // Comparison is `eval_cmp`'s, not a second opinion: numeric only when both sides are
        // already numbers, chronological only when both are dates, string-wise otherwise. So
        // `in(num([qty]), 1, 2)` is numeric and `in([code], "007")` is not, and case is exact
        // for the same reason `[name]` addressing is — fold it visibly with `upper()`.
        // Nothing special happens for an empty member: `in([x], "")` is an ordinary test that
        // a blank cell passes, and blank-handling vocabulary stays with default/coalesce.
        "in" => {
            if argc < 2 {
                return Err(EvalErr::Hard(XledError::Correction(format!(
                    "in() takes a value and at least one member — in([col], \"A\", \"B\") — \
                     got {argc} argument(s). A membership test against nothing is a typo, not \
                     a constant false."
                ))));
            }
            let subject = eval(buf, row, &args[0])?;
            for member in &args[1..] {
                if eval_cmp(CmpOp::Eq, &subject, &eval(buf, row, member)?) {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        // Case-folding: Unicode, matching `s///`'s `\U`/`\L` exactly (both use char::to_upper/
        // lowercase), so `upper([c])` and `[c] s/.*/\U&/` never diverge on non-ASCII.
        "upper" => {
            want(1)?;
            Ok(Value::Str(
                eval(buf, row, &args[0])?.as_string().to_uppercase(),
            ))
        }
        "lower" => {
            want(1)?;
            Ok(Value::Str(
                eval(buf, row, &args[0])?.as_string().to_lowercase(),
            ))
        }
        "proper" => {
            want(1)?;
            Ok(Value::Str(title_case(
                &eval(buf, row, &args[0])?.as_string(),
            )))
        }
        // Whitespace stripping: Unicode `char::is_whitespace`, so Excel's non-breaking space
        // (U+00A0) is trimmed too — a common exported-CSV artifact.
        "trim" => {
            want(1)?;
            Ok(Value::Str(
                eval(buf, row, &args[0])?.as_string().trim().to_string(),
            ))
        }
        "ltrim" => {
            want(1)?;
            Ok(Value::Str(
                eval(buf, row, &args[0])?
                    .as_string()
                    .trim_start()
                    .to_string(),
            ))
        }
        "rtrim" => {
            want(1)?;
            Ok(Value::Str(
                eval(buf, row, &args[0])?.as_string().trim_end().to_string(),
            ))
        }
        // Pad to a fixed width — the leading-zero restorer: `lpad([zip], 5, "0")`. Never
        // truncates (a wider value passes through), because silent data loss is the same
        // betrayal as coercion. `fill` defaults to a space.
        "lpad" | "rpad" => {
            if argc != 2 && argc != 3 {
                return Err(EvalErr::Hard(XledError::Correction(format!(
                    "{name}() takes 2 or 3 arguments (string, width, [fill])"
                ))));
            }
            let s = eval(buf, row, &args[0])?.as_string();
            let width = arg_usize(buf, row, &args[1])?;
            let fill = if argc == 3 {
                eval(buf, row, &args[2])?.as_string()
            } else {
                " ".to_string()
            };
            Ok(Value::Str(pad(&s, width, &fill, name == "lpad")))
        }
        // Numeric helpers rounding out `round`. All cast their args with num()-strength
        // coercion, so a non-number skips the cell and tallies, same as arithmetic.
        "abs" => {
            want(1)?;
            Ok(Value::Num(cast_num(&eval(buf, row, &args[0])?)?.abs()))
        }
        "floor" => {
            want(1)?;
            Ok(Value::Num(cast_num(&eval(buf, row, &args[0])?)?.floor()))
        }
        "ceil" => {
            want(1)?;
            Ok(Value::Num(cast_num(&eval(buf, row, &args[0])?)?.ceil()))
        }
        // Remainder with the dividend's sign (Rust/awk/C `%`, not Excel's divisor-sign MOD),
        // this being sed *and awk*. Divide-by-zero leaves the cell + tallies, like BinOp::Div.
        "mod" => {
            want(2)?;
            let a = cast_num(&eval(buf, row, &args[0])?)?;
            let b = cast_num(&eval(buf, row, &args[1])?)?;
            if b == 0.0 {
                return Err(EvalErr::Cast);
            }
            Ok(Value::Num(a % b))
        }
        // Variadic and numeric-only (the names imply numbers); ≥1 arg, coalesce's precedent.
        "min" | "max" => {
            if argc == 0 {
                return Err(EvalErr::Hard(XledError::Correction(format!(
                    "{name}() needs at least one argument"
                ))));
            }
            let mut acc = cast_num(&eval(buf, row, &args[0])?)?;
            for a in &args[1..] {
                let v = cast_num(&eval(buf, row, a)?)?;
                acc = if name == "min" {
                    acc.min(v)
                } else {
                    acc.max(v)
                };
            }
            Ok(Value::Num(acc))
        }
        // The date cast. One argument reads ISO 8601 and nothing else; two spell out the
        // layout. Casting a date again is a no-op, so date(date(x)) is harmless.
        "date" => {
            if argc != 1 && argc != 2 {
                return Err(EvalErr::Hard(XledError::Correction(
                    "date() takes 1 or 2 arguments (value, [format])".into(),
                )));
            }
            let v = eval(buf, row, &args[0])?;
            if let Value::Date(d) = v {
                return Ok(Value::Date(d));
            }
            let s = v.as_string();
            if argc == 2 {
                let fmt = eval(buf, row, &args[1])?.as_string();
                if !date::has_year(&fmt) {
                    return Err(EvalErr::Hard(XledError::Correction(format!(
                        "the format \"{fmt}\" names no year, so it can't read a date — add \
                         a YYYY (or YY) token to it."
                    ))));
                }
                return date::parse_with(&s, &fmt)
                    .map(Value::Date)
                    .ok_or(EvalErr::Cast);
            }
            if let Some(d) = date::parse_iso(&s) {
                return Ok(Value::Date(d));
            }
            // Two different failures live here, and the split is the whole safety property.
            // A value no known layout reads is a hole in the *data*: skip the cell, tally,
            // rule 6. A value that some layout does read is a hole in the *program* — the
            // user meant a date and hasn't said which layout — and that is wrong identically
            // on every row, so it halts once rather than burying the fix in a warning count.
            let hits = date::probe(&s);
            let Some((first, first_date)) = hits.first() else {
                return Err(EvalErr::Cast);
            };
            let disagreeing = hits.iter().find(|(_, d)| d != first_date);
            Err(EvalErr::Hard(XledError::Correction(match disagreeing {
                Some((other, _)) => format!(
                    "{s} is ambiguous: both {first} and {other} parse it.\n\
                     Say which one: date([col], \"{first}\")"
                ),
                None => format!(
                    "{s} is not ISO 8601, and date() does not guess a layout.\n\
                     Say it: date([col], \"{first}\")"
                ),
            })))
        }
        // Excel's TEXT, date half only. The name is reserved for the whole of it, so calling
        // it on a number says "not yet" and points at what does exist today.
        "text" => {
            want(2)?;
            let v = eval(buf, row, &args[0])?;
            let fmt = eval(buf, row, &args[1])?.as_string();
            match v {
                Value::Date(d) => Ok(Value::Str(date::render(d, &fmt))),
                Value::Num(_) => Err(EvalErr::Hard(XledError::NotAvailableYet(
                    "text() formats dates. Number formatting — thousands separators, \
                     currency, fixed decimals — is a later xled. For decimal places now: \
                     round([col], 2)."
                        .into(),
                ))),
                _ => Err(EvalErr::Hard(XledError::Correction(
                    "text() needs a date, and xled does not coerce one — cast first: \
                     text(date([col]), \"DD/MM/YYYY\")"
                        .into(),
                ))),
            }
        }
        // Components, for grouping and filtering. `weekday` is ISO (1 = Monday), not Excel's
        // 1 = Sunday: the serialization is already ISO, and a tool whose dates are ISO should
        // not carry a US-convention off-by-one. Documented rather than silently different.
        "year" | "month" | "day" | "weekday" => {
            want(1)?;
            let d = require_date(&eval(buf, row, &args[0])?, name)?;
            Ok(Value::Num(match name {
                "year" => d.year() as f64,
                "month" => d.month() as f64,
                "day" => d.day() as f64,
                _ => d.weekday().number_from_monday() as f64,
            }))
        }
        "today" => {
            want(0)?;
            Ok(Value::Date(today()))
        }
        // A row-index function is deliberately absent: a computed cell sees only values, and
        // reading its own position would break that value-in/value-out model. Point to the
        // flag that does emit logical row numbers instead of leaving a bare "unknown function".
        // Regex as a *value*, not a command. `s///` rewrites the cells it addresses, so it can
        // only ever write back into the column it read; these read one column and can be
        // assigned into another, which is the whole gap they close.
        "regexreplace" => {
            want(3)?;
            let text = eval(buf, row, &args[0])?.as_string();
            let pattern = eval(buf, row, &args[1])?.as_string();
            let rep = eval(buf, row, &args[2])?.as_string();
            let re = compiled(&pattern)?;
            // `global = true`: this is the spreadsheet family's REGEXREPLACE, which replaces
            // every match. `s///` replaces the first without `g` because it is sed's command
            // and keeps sed's contract; the two conventions each stay faithful to their own
            // lineage rather than one bending to the other. Stated in expr-grammar.md.
            Ok(Value::Str(subst::substitute(
                &re,
                &Replacement::parse(&rep),
                &text,
                true,
                None,
            )))
        }
        "regexmatch" => {
            want(2)?;
            let text = eval(buf, row, &args[0])?.as_string();
            let pattern = eval(buf, row, &args[1])?.as_string();
            Ok(Value::Bool(compiled(&pattern)?.is_match(&text)))
        }
        "row" => Err(EvalErr::Hard(XledError::Correction(
            "there is no row() — a computed column can't read its own row index. To emit \
             logical row numbers, use the --number flag: `xled --number '[col]' file`."
                .into(),
        ))),
        other => Err(EvalErr::Hard(XledError::Correction(format!(
            "unknown function {other}()"
        )))),
    }
}

/// Title-case (`proper`): the first letter of each run of letters is uppercased, the rest
/// lowercased, non-letters pass through untouched. Any non-letter resets the run, so this
/// carries Excel PROPER's known quirk — `mcdonald` → `Mcdonald`, `o'brien` → `O'Brien` —
/// rather than trying to out-guess names.
fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_word_start = true;
    for ch in s.chars() {
        if ch.is_alphabetic() {
            if at_word_start {
                out.extend(ch.to_uppercase());
            } else {
                out.extend(ch.to_lowercase());
            }
            at_word_start = false;
        } else {
            out.push(ch);
            at_word_start = true;
        }
    }
    out
}

/// Pad `s` to `width` characters (counted as Unicode scalars, like `len`) with `fill`, on the
/// left (right-aligning `s`, for `lpad`) or the right (`rpad`). Never truncates: a value already
/// at least `width` wide returns unchanged. `fill` repeats and is cut to the exact deficit; an
/// empty `fill` can't pad, so the value passes through.
fn pad(s: &str, width: usize, fill: &str, left: bool) -> String {
    let len = s.chars().count();
    if len >= width || fill.is_empty() {
        return s.to_string();
    }
    let padding: String = fill.chars().cycle().take(width - len).collect();
    if left {
        format!("{padding}{s}")
    } else {
        format!("{s}{padding}")
    }
}

/// A function-arg count: evaluate and require a non-negative number.
fn arg_usize(buf: &Buffer, row: usize, e: &Expr) -> Result<usize, EvalErr> {
    let n = cast_num(&eval(buf, row, e)?)?;
    if n < 0.0 {
        return Err(EvalErr::Cast);
    }
    Ok(n as usize)
}

/// Operand of arithmetic: must already be a number (no coercion).
fn require_num(v: &Value) -> Result<f64, EvalErr> {
    match v {
        Value::Num(n) => Ok(*n),
        _ => Err(EvalErr::Cast),
    }
}

/// Explicit `num()` cast: parse strings, bools → 1/0.
///
/// A date has no number to cast to. Excel would hand back a serial number, but serials are
/// the damage xled exists to repair (`intake-taxonomy.md`), not a value to hand out — so this
/// halts and names the four things the caller actually wanted.
fn cast_num(v: &Value) -> Result<f64, EvalErr> {
    match v {
        Value::Num(n) => Ok(*n),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Str(s) => s.trim().parse::<f64>().map_err(|_| EvalErr::Cast),
        Value::Date(_) => Err(EvalErr::Hard(XledError::Correction(
            "num() on a date has no meaning — xled has no serial numbers. Use year(), \
             month(), day(), or subtract two dates for a count of days."
                .into(),
        ))),
    }
}

/// Explicit `bool()` cast.
fn cast_bool(v: &Value) -> Result<bool, EvalErr> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Num(n) => Ok(*n != 0.0),
        Value::Str(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(EvalErr::Cast),
        },
        Value::Date(_) => Err(EvalErr::Hard(XledError::Correction(
            "bool() on a date has no meaning — compare it instead: \
             date([col]) >= date(\"2024-01-01\")"
                .into(),
        ))),
    }
}

/// The date the run started, fixed once for the whole run.
///
/// Per-row evaluation would let a long run straddle midnight and stamp two different "today"s
/// into one column. That is a correctness property, not an optimization.
fn today() -> NaiveDate {
    static TODAY: OnceLock<NaiveDate> = OnceLock::new();
    *TODAY.get_or_init(|| chrono::Local::now().date_naive())
}

/// Move a date by a whole number of days. A fractional offset skips the cell rather than
/// truncating silently: there is no time of day here to carry the remainder into.
fn offset_days(d: NaiveDate, n: f64) -> Result<Value, EvalErr> {
    if !n.is_finite() || n.fract() != 0.0 {
        return Err(EvalErr::Cast);
    }
    chrono::TimeDelta::try_days(n as i64)
        .and_then(|delta| d.checked_add_signed(delta))
        .map(Value::Date)
        .ok_or(EvalErr::Cast)
}

/// Operand of a date function: must already be a date, same no-coercion rule as `require_num`.
/// A missing cast is wrong on every row equally, so it halts with the corrected form instead
/// of tallying one skip per row.
fn require_date(v: &Value, fname: &str) -> Result<NaiveDate, EvalErr> {
    match v {
        Value::Date(d) => Ok(*d),
        _ => Err(EvalErr::Hard(XledError::Correction(format!(
            "{fname}() needs a date, and xled does not coerce one — cast first: \
             {fname}(date([col]))"
        )))),
    }
}
