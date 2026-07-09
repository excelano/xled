# xled reference

Complete reference for the `xled` CLI. Load this when `SKILL.md` isn't specific
enough — full address grammar, per-command scope contracts, every expression
function with its signature, and the input-encoding rules.

## Invocation and flags

```
xled [FLAGS] '<script>' [FILE]     # one-shot (FILE omitted → read stdin)
xled [FLAGS] FILE                  # REPL (script omitted, stdin is a terminal)
```

| Flag | Meaning |
|---|---|
| `-d`, `--delim <CHAR>` | field delimiter; default `,`, or tab when the file ends in `.tsv` |
| `-f`, `--file <SCRIPT>` | read the script from a file (like `sed -f`); the lone positional is then the input file |
| `-i`, `--in-place[=SUFFIX]` | edit the file in place; attach a suffix (`-i.bak`) to keep the original. Refuses an inspect-only script (nothing to write) and refuses piped stdin |
| `--raw` | value-only output: no header row, no CSV quoting, one addressed value per line. Applies to inspect scripts (a bare address or `show`); ignored on a mutation |
| `--number` | prefix each output row with its logical 1-based row number (stays correct across cells that contain embedded newlines, which line-based tools miscount). Inspect scripts only |
| `--no-header` | treat row 1 as data, not a header — use when the real header sits under a title block, so row numbers match the file and you can `crop` then `header N` |
| `-V`, `--version` / `-h`, `--help` | standard |

A script is one or more statements, one per line. A statement is `address command`;
either part may stand alone. An address alone shows those cells; a command alone acts
on the whole table.

## Address grammar

Positional addresses are bare, names are bracketed.

| Form | Selects |
|---|---|
| `A`, `Z`, `AA`, `BC` | column by letter (spreadsheet-style, past Z) |
| `[name]` | column by header name — **exact, case-sensitive** (`[userId]` ≠ `[userid]`) |
| `[name with spaces]`, `[price (USD)]` | brackets quote any name with spaces, slashes, parens, digits |
| `5` | row 5 (1-based, over the logical header-aware row space) |
| `2:4` | rows 2–4 inclusive |
| `B2:C3` | rectangle between two corners |
| `C:AF` | column span |
| `/re/` | every cell matching the regex (add `i` for case-insensitive: `/re/i`) |
| `[col]~/re/` | cells in one column matching the regex |
| `/re/ [col]` | intersect a regex row-select with a column (space = intersect) |

**Combinators** (precedence low→high): `,` union < ` ` (space) intersect < `:` range
< `!` negate; parentheses override. There is intentionally **no `and` / `or`** — a
multi-predicate row filter is out of scope (use SQL). Disambiguation notes: the column
*named* `B` is `[B]` while the column *at* letter B is `B`; the header `2024` is
`[2024]` while row 2024 is `2024`.

## Commands and their scope contracts

| Command | Contract |
|---|---|
| `s/re/repl/flags` | substitute over the addressed cells. Flags: `g` (all occurrences), `i` (case-insensitive), a digit (the Nth occurrence). Replacement dialect: `\1`–`\9` backrefs, `&` whole match, `\U \L` (upper/lower until `\E`), `\u \l` (next char). `^`/`$` anchor within the cell |
| `= expr` | compute into **exactly one** column; creates it (appended at the current width) if the name is new. Refuses a multi-column address |
| `del` | delete **whole rows xor whole columns** — never a partial rectangle |
| `crop` | reduce the buffer to the single addressed rectangle (carve a table out of surrounding junk) |
| `header N` | promote row N to be the column-name header |
| `rename <newname>` | rename the addressed header in place; takes the rest of the line literally (no quoting). One row or one column |
| `fill` / `fill down` | fill blank addressed cells from the value above (merged-cell artifacts) |
| `drop blanks [rows\|cols]` | trim empty edge rows and/or columns |
| `describe` | advisory-only region report (preamble, blank edges, suspected header/total rows); never mutates |
| `show` | print the addressed cells; the default command when one is omitted |

When a command and its address disagree on shape, xled refuses with a correction that
names the right form — it does not guess.

## Expression language (`= expr`)

Three value types: **string**, **number** (`f64`), **bool**. **No implicit
coercion.** Operators: `+ - * /` (numbers only — cast first), `&` (string concat),
comparisons `== != < <= > >=`. Comparisons are **string-wise** unless both operands
are `num()`-cast, so `"9" > "10"` is `true` lexically; cast to compare numerically.
A failed cast is **non-halting**: the offending cell is left unchanged and a tally is
printed to stderr.

### Function library

Text:

| Signature | Returns |
|---|---|
| `len(s)` | length of `s` in characters |
| `left(s, n)` | first `n` characters |
| `right(s, n)` | last `n` characters |
| `mid(s, start, n)` | `n` chars from 1-based `start` (all three args required) |
| `substr(s, start, [n])` | from 1-based `start`; with `n`, that many chars, else to the end of the string |
| `trim(s)` | strip leading and trailing whitespace (interior preserved) |
| `ltrim(s)` / `rtrim(s)` | strip only the left / right side |
| `lpad(s, width, [fill])` | pad on the left to `width`; `fill` defaults to a space. **Never truncates** — an already-wide value returns unchanged; empty `fill` is a no-op |
| `rpad(s, width, [fill])` | as `lpad`, padding on the right |

Case (identical Unicode fold to `s///`'s `\U` / `\L`, non-ASCII included):

| Signature | Returns |
|---|---|
| `upper(s)` / `lower(s)` | Unicode upper / lower case |
| `proper(s)` | title-case each word; a non-letter resets the run, so `McDonald` → `Mcdonald` and `O'Brien` → `O'Brien` (matches Excel's `PROPER`) |

Numbers:

| Signature | Returns |
|---|---|
| `round(x, d)` | round `x` to `d` decimals (use for money — xled never rounds on write) |
| `abs(x)` / `floor(x)` / `ceil(x)` | absolute value / floor / ceiling |
| `mod(a, b)` | remainder taking the **dividend's sign** (`mod(-3, 5)` = `-3`, awk not Excel); `b == 0` leaves the cell and tallies |
| `min(x, …)` / `max(x, …)` | variadic, numeric-only; at least one argument |

Casts and logic:

| Signature | Returns |
|---|---|
| `num(s)` | parse to number; failure is the non-halting skip |
| `bool(s)` | parse to bool |
| `default(v, fallback)` | `fallback` when `v` is empty, else `v` |
| `coalesce(a, b, …)` | first non-empty argument |
| `if(cond, then, else)` | branch on a bool `cond` |

There is **no `row()`** — a computed cell cannot read its own row index (that would
break value-in/value-out). Use the `--number` flag to emit logical row numbers.

## REPL words

`xled FILE` opens a live, in-memory buffer; nothing reaches disk until `write`.

| Word | Effect |
|---|---|
| `address command` | apply an edit to the buffer (pushes an undo snapshot) |
| `preview <cmd>` | show what a command *would* do, without applying it |
| `undo` | revert the last mutation |
| `write [path]` | serialize the buffer to disk (to `path`, else the source file) — the only write |
| `help` | command help |
| `quit` / `q` | exit; warns on unsaved changes |
| `quit!` / `q!` | exit, discarding unsaved changes |

## Input encoding

xled expects **UTF-8**. A leading BOM (Excel "Save as CSV UTF-8") is stripped so the
first header isn't polluted. If the file looks like UTF-7 (the `+ACI-` escape some
exporters emit) or carries a UTF-16 BOM, xled warns at startup with the exact `iconv`
command to convert it first, rather than failing with a raw "invalid UTF-8" error.
Unchanged cells round-trip **byte-for-byte** (the leading-zero / quoted-field
guarantee); the `csv` crate handles embedded commas, escaped quotes, and embedded
newlines correctly.

## Error vocabulary

Refusals come in four voices, each with a correction rather than a bare failure:
**not-in-scope** (e.g. a multi-predicate filter → use SQL), **not-supported**,
**not-recoverable**, and **not-available-yet**. A scope-contract violation (a partial
`del`, a multi-column `=`) and the combinator wall (`and`/`or`) route here with the
right form named.
