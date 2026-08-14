# Expr — the compute layer

Scope: the RHS of `= expr` assignment, and the operands of address-comparison atoms. The address/composition grammar lives in `composition-grammar.md`; text rewriting lives in `semantics.md` under `s///`. This file is everything that computes a *value*: arithmetic, concatenation, comparison-as-bool, and a small function library.

Why it earns its own spec: rendering `proving-ground.md` Part B against the locked grammar showed the "thin compute layer" is not thin. It carries B9 (compute/derive), B10 (conditional/blank), the join half of B8, and every comparison scope in Part A. The library below is **derived from the battery's actual operations, not invented** — each function cites the item that forces it.

## The layered split (load-bearing)

xled has exactly two transform layers, and the line between them is sharp:

- **`s///` rewrites text by pattern** — substitution, capture/rearrange, case (`\U \L \u \l \E`), trimming, whitespace, char-class stripping. Anything that edits the characters of one cell in place. Sed muscle memory, sed dialect (see `semantics.md`).
- **`= expr` computes a value** — arithmetic, concatenation, comparison, measuring (`len`), slicing (`left`/`mid`), rounding, defaulting, conditional. Anything that derives a new value, possibly from several columns. awk/Excel muscle memory.

The test: *rewriting the characters of a cell by pattern* → `s///`; *producing a value (number, bool, date, or a picked / measured / computed string)* → `expr`.

Case-folding and trimming were first held out of expr on the argument that they are pattern rewrites of text and belong in `s///`. The compose case decided otherwise: `[full] = upper([first]) & " " & [last]` cannot be written that way without a second pass, and a layer boundary that forces two commands to do one thing is drawn in the wrong place. So `upper`/`lower`/`proper` and the `trim` family live in expr as well, guaranteed to fold identically to `s///`'s `\U`/`\L` so a script can reach for either and never see them disagree. The boundary that actually holds is the one above: `s///` matches a *pattern*; expr does not. There is no regex in the function library, and there is no substitution in it either.

## Value model

Four types: string, number, bool, date. The buffer is all strings; expr lifts a cell to a typed value, computes, and serializes back to a string on write. **No auto-coercion** — casts are explicit (`num()`, `bool()`, `date()`), the same property that keeps leading zeros and long IDs safe. A cast failure is non-halting: it leaves the cell unchanged and increments the warning tally (`semantics.md` rule 6, lenient).

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

- Arithmetic `+ - * /` on numbers; a non-numeric operand is a cast failure (lenient, tallied). `+` and `-` also carry date arithmetic (see Dates).
- `&` concatenates strings — Excel's concat operator: `[first] & " " & [last]`.
- Comparison `== != < > <= >=` — string-wise unless `num()`-cast; yields a bool. This is the *same* operator set and operand grammar as the address-comparison atom in `composition-grammar.md`: in address position it selects rows, in RHS position it produces a bool value.

**No boolean `and`/`or`/`not` operators in expr** — that is the slippery slope out of xled's lane. Multi-condition logic nests through `if()`/`coalesce()`; genuine multi-predicate filtering is xql's job. This keeps expr consistent with refusing combinators in *address* position, which is the same boundary drawn in `composition-grammar.md`.

**`in(x, …)` is not the exception that reopens it.** `in(x, a, b)` is `x == a or x == b`, so it owes this refusal an answer rather than a pass. The answer is that **`in`'s arguments are values, not predicates** — one subject against a closed list of members. It combines nothing; the `or` is internal to a single test, which is why SQL gives `IN` its own primitive instead of making people write the chain. `coalesce` is already the variadic first-one-that-answers shape on the value side, and `in` is its sibling on the predicate side.

What the refusal has always meant is that no *operator* chains conditions: there is no production for one, so there is no rule for a parser to enforce (`ebnf.md`, the combinator wall). It has never meant two conditions cannot be combined at all — `if(cond, true, other)` nests them today and is documented above as the way to. `in(true, [a] > 5, [b] < 3)` arrives at the same place through the same door: neither newly possible nor recommended, because a real multi-predicate filter is a query and reads better as one. The wall is against growing the grammar toward xql's job, not against a determined nesting.

## Function library

Excel-faithful names where the user's Excel half reads them on sight; awk where that memory is stronger. Locked David 2026-06-21.

The set is **derived, not invented**: the original library came out of rendering `proving-ground.md` Part B against the grammar, and each addition since has come from a case the battery or real use produced. Nothing is here because a spreadsheet has it.

**Casts and logic**

| Function | Does |
|---|---|
| `num(x)` | cast to number; failure → leave cell + tally |
| `bool(x)` | cast to bool |
| `date(x)` / `date(x, fmt)` | cast to date — see Dates below |
| `default(x, fb)` | `x` unless it is empty, then `fb` |
| `coalesce(a, b, …)` | first non-empty argument |
| `if(cond, a, b)` | `a` when `cond` is true, else `b` — a pure expression, **not** control flow |
| `in(x, a, …)` | → bool, whether `x` equals any member. Members are **literals compared**, never a pattern |

**Text**

| Function | Does |
|---|---|
| `len(x)` | character length → number |
| `left(x, n)` / `right(x, n)` | first / last `n` characters |
| `mid(x, start, n)` | `n` chars from 1-based `start` (Excel MID) |
| `substr(x, start [, len])` | awk substring, 1-based; the **2-arg form is "from `start` to end"** — the reason it earns a slot beside `mid` (the 3-arg form is `mid`, kept as the awk-memory door) |
| `upper(x)` / `lower(x)` / `proper(x)` | case-fold, Unicode, identical to `s///`'s `\U`/`\L`; `proper` carries Excel PROPER's `mcdonald` → `Mcdonald` quirk rather than out-guessing names |
| `trim(x)` / `ltrim(x)` / `rtrim(x)` | strip whitespace both sides / left / right, Unicode so Excel's non-breaking space goes too |
| `lpad(x, w [, fill])` / `rpad(x, w [, fill])` | pad to width `w`; **never truncates**, because dropping characters is the same betrayal as coercion |

**Regex**

| Function | Does |
|---|---|
| `regexreplace(x, pat, rep)` | every match of `pat` in `x` replaced by `rep` |
| `regexmatch(x, pat)` | → bool, whether `pat` matches anywhere in `x` |

These are the reason expr can do what `s///` cannot. `s///` rewrites the cells it addresses, so it can only ever write back into the column it read; `regexreplace` reads one column and the assignment writes another, which is what a derived column needs.

`rep` is **xled's own replacement dialect**, not the regex crate's `$1` — `\1`–`\9`, `&` for the whole match, and the `\U \L \u \l \E` case-folds, expanded by the same parser `s///` uses so the two cannot drift. Inside a string literal these need no extra escaping: expr's strings only treat `\"` specially and pass every other backslash through, so `"\U\1"` arrives as written.

`regexreplace` replaces **every** match, where `s///` without `g` replaces the first. That difference is deliberate rather than an oversight: `regexreplace` is the spreadsheet family's function and keeps their contract, `s///` is sed's command and keeps sed's. Case-insensitivity has no flag argument because the regex dialect already carries one — write `(?i)` at the front of the pattern, the same inline form `s///i` expands to internally.

A pattern that will not compile **halts** rather than tallying a skipped cell. A failed cast says something about one row's data; an unparsable regex is wrong on every row, so the run stops and shows the engine's own message.

A pattern is an ordinary argument, so it may be a column and vary from row to row. Compiled patterns are cached by their text, which makes the common literal case compile once for the whole file without making the varying case wrong.

**Numbers**

| Function | Does |
|---|---|
| `round(x, d)` | round to `d` decimals |
| `abs(x)` / `floor(x)` / `ceil(x)` | the usual three |
| `mod(a, b)` | remainder taking the **dividend's** sign (awk/C, not Excel's divisor-sign MOD) — this being sed *and awk*; divide-by-zero leaves the cell |
| `min(a, …)` / `max(a, …)` | variadic, numeric |

**Dates** — `date`, `text`, `year`, `month`, `day`, `weekday`, `today`. Their own section below.

`if()` draws the no-control-flow line precisely: a conditional *expression* (a function returning a value) is in; statement-level branching and loops are out. Chosen over awk's `?:` because `:` is already the range operator and `if()` reuses the function-call machinery with zero new syntax — David confirmed 2026-06-21. It is also Excel's exact spelling for a half-Excel user.

`in(x, a, …)` is the set-membership test, and it exists because the alternation that could stand in for it is wrong twice over. Unanchored, `^(?:APP|CAM)$` without its anchors matches inside `APPLE` and `SCAM`. Anchored, every member is still regex *source*, so a value carrying a metacharacter is compiled rather than compared — against `^(?:R+D)$`, the value `R+D` does not match and `RRD` does, which is the failure landing on the one row that matters and saying nothing. `in` compares literals, so neither is available to it. Comparison is the layer's own — numeric only when both sides are already numbers, chronological only when both are dates, string-wise otherwise — so `in(num([qty]), 1, 2)` is numeric and `in([code], "007")` is not. Case is exact, like `[name]` addressing and for the same reason; `in(upper([org]), "APP")` is the folded form, one visible call rather than a hidden policy. An empty member is an ordinary test that a blank cell passes, since `default`/`coalesce` already own the blank-handling vocabulary. A subject with no members (`in(x)`) is a correction, not a constant false. Reading the set from a column or a file would be a join, and that is xql's.

Deliberately absent: boolean `and`/`or` **operators** (nest `if`, or it is an xql query); a regex-extract function (→ `s///` in place — revisit only if split-into-columns earns a home, see proving-ground B8); `row()`, which would let a computed cell read its own position and break the value-in/value-out model — `--number` emits logical row numbers instead, and the error says so rather than reporting an unknown function.

**Numbers serialize at full precision — `round()` is mandatory for money.** A number is a binary `f64`, and it writes back as the shortest decimal that round-trips to that exact float. So `= num([price]) * 1.1` on `19.99` writes `21.989` here but can write `21.989000000000004` elsewhere, because the product isn't representable in binary. This is not a bug to fix by rounding on write: silent rounding would betray the stringly model the same way auto-coercion would — the layer never invents precision the user didn't ask for. The rule is therefore explicit: any computed column that is currency or fixed-decimal must be wrapped in `round(…, d)`, e.g. `[total] = round(num([price]) * [qty], 2)`. Integral results (`2.0`) already print clean (`2`); it is only fractional float arithmetic that leaks artifacts, and `round()` is the one place precision is pinned.

## Dates

A date is a type, not a string convention. `intake-taxonomy.md` names date auto-conversion as a headline damage category, and repairing it — normalizing a column, pulling a year out for grouping, computing a duration — is row-local scalar work, squarely in this layer's lane. Internally a naive calendar date: no time of day, no timezone, no offset.

**Dates serialize as ISO 8601 (`YYYY-MM-DD`).** This is load-bearing rather than a default, because it makes the most common repair fall out of the cast alone, with no formatting call at all:

```
[hired] = date([hired], "DD/MM/YYYY")      # a British-formatted column, normalized in place
```

ISO is also lexically sortable, so a serialized date column still orders correctly when something downstream compares it as plain text. For this one type, the string-wise comparison default stops being a footgun.

### The refusal to guess

`03/04/2024` is the third of April in one hemisphere and the fourth of March in the other, and a tool that picks one silently corrupts a column in a way nobody notices until quarter-end. **xled never guesses DD/MM versus MM/DD.** This is the same principle as refusing to coerce `02134` into `2134`, and it drives the shape of the cast.

`date(x)` reads ISO 8601 and nothing else — the extended form `2024-03-04`, the basic form `20240304`, and either followed by a time, which is truncated to its date. Truncating a timestamp is allowed precisely because the cast is explicit: the user asked for a date, and `2024-03-04T14:03:00Z` contains exactly one. That is a different act from choosing between two readings of the same characters.

`date(x, fmt)` reads that layout and only that one.

A bare `date(x)` therefore has three outcomes, and the split between the last two is the whole safety property:

| The value | Outcome | Because |
|---|---|---|
| reads as ISO | a date | the good path |
| reads under no known layout | leave the cell, tally (rule 6) | a hole in the **data** — per row, non-halting |
| reads under some other layout | halt, name the layouts | a hole in the **program** — identical on every row |

The third row departs from rule 6 deliberately. A value that some layout reads means the user meant a date here and has not said which layout, and that is wrong for all rows equally; tallying one warning per row would bury the single thing they need to read. So it halts, once:

```
03/04/2024 is ambiguous: both DD/MM/YYYY and MM/DD/YYYY parse it.
Say which one: date([col], "DD/MM/YYYY")
```

A value only one layout reads (`15/07/2023` — 15 is no month) halts too, with "date() does not guess a layout" and that layout named. Accepting it would mean deciding the layout per row, which is the same guess arrived at more quietly.

### Format tokens

Excel's, not strftime's: `YYYY YY MMMM MMM MM M DDDD DDD DD D`, matched case-insensitively, with everything else passing through as a literal and `\` escaping a token letter back into one. The tiebreak this file already fixed decides it — Excel-faithful where the user's Excel half reads it on sight — and nobody's Excel memory holds `%Y`. Excel's own `mm`-means-minutes-after-an-hour wart cannot arise, because there are no times here for a month token to collide with.

Reading is more lenient than writing, where leniency costs nothing: either width of month or weekday name is accepted under either token, a weekday name is consumed and discarded (it carries nothing the date doesn't), and a whitespace literal absorbs a run of whitespace. A missing day or month defaults to the first, so `date([m], "MMM YYYY")` reads a month column. A missing *year* cannot be defaulted, so a format without one is rejected as the program error it is, before any row runs. Month and day names are English; a localized column is handled the way any other non-ISO layout is.

`YY` reads on Excel's pivot — `00`–`29` as the 2000s, `30`–`99` as the 1900s — which is a policy, and the reason to spell out `YYYY` wherever the data allows it.

### Arithmetic and comparison

`date − date` is a **number of days** (age, tenure, days outstanding); `date ± number` is a **date** (due dates, retention cutoffs). A fractional offset skips the cell rather than truncating silently — there is no time of day here to carry a remainder into. `date + date` has no meaning and skips, as does any other mixture. Two dates compare chronologically; a date against anything else falls back to string-wise, which is still correct against an ISO literal.

Casting a date *out* is refused rather than skipped, since it can only be a mistake in the program. `num()` on a date does not hand back a serial number — serials are the damage this tool repairs, not a value to give away — and points at `year()`, `month()`, `day()`, or subtraction instead. `bool()` on a date points at comparison.

### The rest of the library

`text(d, fmt)` renders a date back out under an Excel format. The name is reserved for the whole of Excel's TEXT, but only the date half ships: called on a number it says *not yet* and points at `round()`, because the error taxonomy exists so a real future capability is distinguishable from a typo. Number formatting — separators, currency, fixed decimals — has its own token language and is a later slice.

`year(d)`, `month(d)`, `day(d)` return numbers. `weekday(d)` returns the **ISO** day of week, 1 = Monday through 7 = Sunday. This differs from Excel, whose default 1 = Sunday is a US convention kept for backward compatibility and which Excel itself lets you override; a tool whose dates serialize as ISO should not carry an off-by-one weekday. Stated here rather than left to be discovered.

`today()` is the date the run started, fixed once for the whole run rather than read per row. That is a correctness property, not an optimization: a long run must not straddle midnight and stamp two different days into one column.

None of these coerce. `year([hired])` on a string column is a missing cast, wrong on every row, so it halts and shows `year(date([col]))` rather than tallying one skip per row.

### Out of scope

**Month arithmetic** (Excel's `EDATE`/`EOMONTH`). "January 31 plus one month" has no correct answer, only policies, and picking one silently is the same class of mistake as guessing DD/MM. Deferred until there is real evidence of the need, and then designed with the policy stated out loud.

**Times and timezones.** A `DateTime` would need offset rules, DST, and a formatting dialect in which Excel's `mm` ambiguity finally bites. Truncating a timestamp covers the common created/modified column without opening any of it.

**Aggregates over a date column** — earliest, latest, count by month — are xql's, unchanged. The two agree on what a date is: xql's loader reads the same ISO forms and likewise refuses to guess a layout, so a column normalized here groups correctly there. The one asymmetry to know when chaining is that **xled truncates a timestamp to its date and xql keeps the time.** Running a created/modified column through `date()` therefore discards the clock, which is the intended behavior of a tool with no time type but is worth knowing before piping the result on.

## What this layer does not do

Join, aggregate, group, sort, multi-condition query → xql/DuckDB. Reshape — unpivot, split one cell into N columns, collapse a multi-row header → out. The expr layer computes one new value per row into one column; it never changes the table's shape beyond appending a column, which assignment already covers.
