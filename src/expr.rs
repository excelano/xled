//! The compute layer: evaluate an `Expr` against one row to a typed `Value`.
//!
//! Three types (string/number/bool), no auto-coercion — casts are explicit (`num`, `bool`),
//! the property that keeps leading zeros safe. A cast failure is non-halting: it surfaces as
//! `EvalErr::Cast`, which the caller turns into "leave the cell, tally a warning" (rule 6).
//! Comparisons are string-wise unless both sides are numbers (the A3 footgun, by design).

use crate::ast::{BinOp, CmpOp, Expr};
use crate::errors::XledError;
use crate::model::Buffer;
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
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
    pub fn into_string(self) -> String {
        match self {
            Value::Str(s) => s,
            Value::Num(n) => format!("{n}"),
            Value::Bool(b) => b.to_string(),
        }
    }

    fn as_string(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Num(n) => format!("{n}"),
            Value::Bool(b) => b.to_string(),
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
    // Numeric order only when both are already numbers; otherwise lexical (string-wise).
    let ord = match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y),
        _ => Some(a.as_string().cmp(&b.as_string())),
    };
    match ord {
        Some(Ordering::Less) => matches!(op, CmpOp::Lt | CmpOp::Le | CmpOp::Ne),
        Some(Ordering::Equal) => matches!(op, CmpOp::Eq | CmpOp::Le | CmpOp::Ge),
        Some(Ordering::Greater) => matches!(op, CmpOp::Gt | CmpOp::Ge | CmpOp::Ne),
        None => matches!(op, CmpOp::Ne), // NaN: only != holds
    }
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
        other => Err(EvalErr::Hard(XledError::Correction(format!(
            "unknown function {other}()"
        )))),
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
fn cast_num(v: &Value) -> Result<f64, EvalErr> {
    match v {
        Value::Num(n) => Ok(*n),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Str(s) => s.trim().parse::<f64>().map_err(|_| EvalErr::Cast),
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
    }
}
