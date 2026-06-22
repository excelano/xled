# Expr — the compute layer

Scope: the RHS of `= expr` assignment, and the operands of address-comparison atoms. The address/composition grammar lives in `composition-grammar.md`; text rewriting lives in `semantics.md` under `s///`. This file is everything that computes a *value*: arithmetic, concatenation, comparison-as-bool, and a small function library.

Why it earns its own spec: slice 3 (rendering `proving-ground.md` Part B against the locked grammar) showed the "thin compute layer" is not thin. It carries B9 (compute/derive), B10 (conditional/blank), the join half of B8, and every comparison scope in Part A. The library below is **derived from the battery's actual operations, not invented** — each function cites the item that forces it.

## The layered split (load-bearing)

xled has exactly two transform layers, and the line between them is sharp:

- **`s///` rewrites text by pattern** — substitution, capture/rearrange, case (`\U \L \u \l \E`), trimming, whitespace, char-class stripping. Anything that edits the characters of one cell in place. Sed muscle memory, sed dialect (see `semantics.md`).
- **`= expr` computes a value** — arithmetic, concatenation, comparison, measuring (`len`), slicing (`left`/`mid`), rounding, defaulting, conditional. Anything that derives a new value, possibly from several columns. awk/Excel muscle memory.

The test: *rewriting the characters of a cell by pattern* → `s///`; *producing a value (number, bool, or a picked / measured / computed string)* → `expr`. Case-folding and trimming live in `s///` even though they *feel* like functions, because they are pattern rewrites of text — keeping them there stops the two layers from overlapping. This is a conscious v1 omission: there is no `upper()`/`lower()`/`trim()` in expr. Do it in `s///` then compute, or compute then `s///`. Revisit only if compose-cases (`[full] = upper([first]) & …`) prove common in the battery.

## Value model

Three types: string, number, bool. The buffer is all strings; expr lifts a cell to a typed value, computes, and serializes back to a string on write. **No auto-coercion** — casts are explicit (`num()`, `bool()`), the same property that keeps leading zeros and long IDs safe. A cast failure is non-halting: it leaves the cell unchanged and increments the warning tally (`semantics.md` rule 6, lenient).

**Comparisons are string-wise unless cast.** `[qty] < [reorder]` compares the literal strings — `"9" > "10"` lexically, which is *not* numeric order. For numeric order, cast both sides: `num([qty]) < num([reorder])`. This is the price of no-coercion and it is deliberate: auto-numifying (awk's behavior) reintroduces exactly the silent surprises the stringly model exists to prevent. David confirmed 2026-06-21. The proving ground's A3 example is corrected to the cast form.

## Atoms

| Atom | Means |
|---|---|
| `[name]`, `[C]` | this row's value in that column — **always bracketed in expr** (a bare identifier is a function name) |
| `"text"` | string literal (double-quoted; `\"` escapes a quote) |
| `42`, `-45.20`, `1.03` | number literal |
| `true`, `false` | bool literal |
| `fn(args)` | function call (see library) |

A literal `]` inside a bracketed name is doubled (`]]`), matching the address grammar's bracket-escape: the header `notes [draft]` is written `[notes [draft]]]`. Only `]` is ambiguous (it can close the bracket); `[` inside needs no escape.

## Operators & precedence

Highest → lowest: `fn()` / atom  >  unary `-`  >  `* /`  >  `+ -`  >  `&` (concat)  >  comparison (`== != < > <= >=`). Parentheses override. Comparison sits lowest so `num([qty]) < num([reorder])` groups as intended; a comparison yields a bool, serialized `true`/`false` when written to a cell (B9's boolean column).

- Arithmetic `+ - * /` on numbers; a non-numeric operand is a cast failure (lenient, tallied).
- `&` concatenates strings — Excel's concat operator: `[first] & " " & [last]`.
- Comparison `== != < > <= >=` — string-wise unless `num()`-cast; yields a bool. This is the *same* operator set and operand grammar as the address-comparison atom in `composition-grammar.md`: in address position it selects rows, in RHS position it produces a bool value.

**No boolean `and`/`or`/`not` operators in expr** (locked, David 2026-06-21 — "the slippery slope out of our lane"). Multi-condition logic nests through `if()`/`coalesce()`; genuine multi-predicate filtering is xql's job. This keeps expr consistent with refusing combinators in *address* position (the slice-2 boundary).

## Function library (v1 — proposed, derived from Part B)

Excel-faithful names where the user's Excel half reads them on sight; awk where that memory is stronger. Locked David 2026-06-21.

| Function | Does | Forced by |
|---|---|---|
| `num(x)` | cast to number; failure → leave cell + tally | B9, every comparison |
| `bool(x)` | cast to bool | B9 |
| `len(x)` | character length → number | B9 length |
| `left(x, n)` / `right(x, n)` | first / last `n` characters | B9 substring |
| `mid(x, start, n)` | `n` chars from 1-based `start` (Excel MID) | B9 substring |
| `substr(x, start [, len])` | awk substring, 1-based; the **2-arg form is "from `start` to end"** — the reason it earns a slot beside `mid` (the 3-arg form is `mid`, kept as the awk-memory door) | B9 substring |
| `round(x, d)` | round a number to `d` decimals | B9 round |
| `default(x, fb)` | `x` unless it is empty, then `fb` | B10 default-blank |
| `coalesce(a, b, …)` | first non-empty argument | B10 coalesce |
| `if(cond, a, b)` | `a` when `cond` is true, else `b` — a pure expression, **not** control flow | B10 conditional |

`if()` draws the no-control-flow line precisely: a conditional *expression* (a function returning a value) is in; statement-level branching and loops are out. Chosen over awk's `?:` because `:` is already the range operator and `if()` reuses the function-call machinery with zero new syntax — David confirmed 2026-06-21. It is also Excel's exact spelling for a half-Excel user.

Deliberately omitted from v1: case and trim (→ `s///`, layer separation); boolean `and`/`or` operators (nest `if`, or it is an xql query); a regex-extract function (→ `s///` in place — revisit only if split-into-columns earns a home, see proving-ground B8).

**Numbers serialize at full precision — `round()` is mandatory for money.** A number is a binary `f64`, and it writes back as the shortest decimal that round-trips to that exact float. So `= num([price]) * 1.1` on `19.99` writes `21.989` here but can write `21.989000000000004` elsewhere, because the product isn't representable in binary. This is not a bug to fix by rounding on write: silent rounding would betray the stringly model the same way auto-coercion would — the layer never invents precision the user didn't ask for. The rule is therefore explicit: any computed column that is currency or fixed-decimal must be wrapped in `round(…, d)`, e.g. `[total] = round(num([price]) * [qty], 2)`. Integral results (`2.0`) already print clean (`2`); it is only fractional float arithmetic that leaks artifacts, and `round()` is the one place precision is pinned.

## What this layer does not do

Join, aggregate, group, sort, multi-condition query → xql/DuckDB. Reshape — unpivot, split one cell into N columns, collapse a multi-row header → out. The expr layer computes one new value per row into one column; it never changes the table's shape beyond appending a column, which assignment already covers.
