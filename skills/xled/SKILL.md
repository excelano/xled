---
name: xled
description: >-
  Edit, clean, and compute over CSV/DSV cell values in place with `xled` — "sed and awk for
  tabular data." Fires on the complaint as readily as the diagnosis: "Excel ate the leading
  zeros", "the IDs came out as 1.23E+15", "the amounts are text and won't sum", "there's a
  title block above the real header", "the merged cells left blanks down the column",
  "everything has trailing spaces", "the dates are DD/MM and need to be ISO". Use it to strip
  currency and format characters, normalize casing, restore or preserve leading zeros and
  long IDs, compute a derived column, fill merged-cell blanks, promote a buried header, or
  carve a real table out of surrounding junk. Better than an `awk -F,`/`sed` one-liner or a
  throwaway pandas script: the Python is six to eight lines inside a bash string with two
  levels of quoting, and `read_csv`/`to_csv` round-trips every column through type inference,
  silently reinstating the damage you were asked to repair. Profile with xray first on an
  unfamiliar file. Not for reshaping the grid — unpivot, pivot, transpose, split a column,
  explode a delimited cell, merge columns (xshape) — nor for the row set: join, group,
  aggregate, sort, dedupe, filter rows out (xql).
---

# xled — sed and awk for tabular data

`xled` applies **awk's field model**, **sed's `s///` substitution**, and **ed's live
in-memory buffer** to Excel-style ranges over CSV and DSV files. You name part of
the table (a column, a row span, a rectangle, a regex-selected set of cells), give a
command, and it rewrites those cells — or previews the result before writing.

The authoritative sources for xled's behavior are the binary itself (`xled --help`)
and the [README](https://github.com/excelano/xled/blob/main/README.md); if anything
here conflicts with them, they win. These recipes assume the complete expression-function
library — `upper` `lower` `proper`, `trim`/`ltrim`/`rtrim`, `lpad`/`rpad`, `abs` `floor`
`ceil` `mod` `min` `max`, and the date functions `date` `text` `year` `month` `day`
`weekday` `today`. **An "unknown function" error means the installed copy predates
one of them**; upgrade with `sudo apt install --only-upgrade xled` (Debian/Ubuntu),
`brew upgrade xled` (macOS), or by re-running the install one-liner from the README.

## The one rule that decides whether xled is the right tool

xled **rewrites cells within the table's existing shape**. It never adds or removes
rows, never reorders or splits columns, never coerces a value. The moment a task needs
*reshaping* — unpivot a wide export, pivot, transpose, split one cell into several
columns, explode a delimited cell, merge columns — that is
[xshape](https://github.com/excelano/xshape). The moment it needs *querying* — join,
group, aggregate, sort, dedupe, filter rows *out* of the output — that is SQL/DuckDB
([xql](https://github.com/excelano/xql)). xled's own errors point you to the sibling by
name. Use xled for **value-level cleanup and per-row computation**; use xshape for
**geometry** and SQL for **set-level questions**.

If you have not seen the file before, [xray](https://github.com/excelano/xray) profiles
it read-only first and tells you which of the three you need.

## Running it

```sh
xled '<script>' file.csv      # one-shot: run the script, print result to stdout
… | xled '<script>'           # one-shot over piped stdin
xled -i '<script>' file.csv   # edit the file in place (like sed -i)
xled -i.bak '<script>' f.csv  # …keeping the original as f.csv.bak
xled file.csv                 # open the interactive REPL on a file
```

One-shot sends data to **stdout** (clean, pipeable) and advisory notices to
**stderr**, so `xled … file.csv > out.csv` is always safe.

**`-i` reports what it did, on stderr.** It writes nothing to stdout, so read that line
instead of spending a `head` to check:

```
xled: wrote products.csv — 12 cells changed in [price]
xled: wrote report.csv — now 41 rows × 8 columns (was 44 × 8)
xled: products.csv rewritten unchanged — the address matched no cells, or the script
      wrote the values already there
```

The third one is the one to watch for. A script whose address matched nothing still exits
0 and still rewrites the file, so **success alone does not mean your address was right** —
`unchanged` is how you find out it wasn't. Re-read the header spelling (names are
case-sensitive) before assuming the data was already clean.

Useful flags: `-d/--delim <char>` (delimiter, `\t` for tab; defaults to `,`, or tab for `.tsv`),
`-f/--file <script>` (read the script from a file), `--raw` (print just the addressed
values, no header, no CSV quoting — for shell capture), `--number` (prefix each
output row with its logical row number), `--count` (print how many rows the address
selected instead of the cells), `--no-header` (treat row 1 as data, when the
real header is buried under a title block).

## Worked recipes

Start here. Most tasks are a small variation on one of these lines; the grammar
sections below are the reference for when none of them fits.

```sh
# strip the currency formatting from a column, in place
xled -i '[annual_cost] s/[$,]//g' app-portfolio.csv

# derive a tax-inclusive total, money-rounded (note the explicit num() casts)
xled '[total] = round(num([price]) * 1.0825, 2)' products.csv

# restore a stripped leading zero: pad the zip back to 5 digits
xled '[zip] = lpad([zip], 5, "0")' ids-zips.csv

# normalize a whole column's case (compute form; the s/// form is `s/.*/\L&/`)
xled '[email] = lower([email])' contacts.csv
xled '[name]  = proper(trim([name]))' contacts.csv    # tidy casing + stray spaces

# normalize a British-formatted date column to ISO (the cast alone does it)
xled '[hired] = date([hired], "DD/MM/YYYY")' staff.csv

# tenure in days, and a year column to group on
xled '[days] = today() - date([hired])
[year] = year(date([hired]))' staff.csv

# fill merged-cell blanks down from the value above
xled '[Vendor] fill down' fill-down.csv

# rename a header in place (rest of line, no quoting needed)
xled '[first name] rename first_name' tricky-headers.csv

# only touch cells that match — set a status where the row is active
xled '/active/i [status] = "approved"' app-portfolio.csv

# capture a single value into a shell variable (no header, no quoting)
owner=$(xled --raw '[owner] 3' app-portfolio.csv)
xled --count '/retire/' app-portfolio.csv        # how many rows match, not which
```

Use `--count` rather than piping a read into `wc -l`: a cell holding a newline makes a
line counter overcount, and `--count` counts logical rows. Nothing matched is `0` at
exit 0, so branch on the number, not on the status.

Two of these carry the rules that catch people out: arithmetic needs an explicit
`num()` cast, and a non-ISO date needs its layout spelled out. Both are in *Expressions
and the type model* below, which is worth reading before writing a `=` command of your
own.

## Addressing (which cells)

Positional addresses are bare; **names are bracketed**. That single rule resolves
every header ambiguity.

| Address | Selects |
|---|---|
| `C` | the column at letter C (past Z too: `AA`, `BC`) |
| `[price]` | the column named `price` — exact, **case-sensitive** |
| `3` / `2:4` | row 3 / rows 2 through 4 |
| `B2:C3` | the rectangle from B2 to C3 |
| `[price (USD)]` | a name with spaces/slashes/parens — brackets quote it |
| `/active/` | every cell matching the regex |
| `[status]~/active/` | cells in `[status]` matching the regex |
| `/active/i [status]` | scope a row-select to one column (space = intersect) |

Combine sets with `,` (union), a space (intersect), `!` (negate), and parens.
There is deliberately **no `and`/`or`** — a multi-predicate row filter is a SQL job.
An address with no command just **shows** those cells.

## Commands (what to do)

| Command | Does |
|---|---|
| `s/re/repl/flags` | sed substitution (`g`, `i`, an occurrence number, `\1`–`\9`, `&`, `\U \L \u \l \E`) |
| `= expr` | compute a value into **exactly one** column, creating it if new |
| `del` | delete whole rows **or** whole columns (never a partial rectangle) |
| `crop` | reduce the buffer to one rectangle (carve a table out of junk) |
| `header N` | promote row N to the column-name header |
| `rename newname` | rename a header in place (takes the rest of the line, no quoting) |
| `fill` / `fill down` | fill blank cells from the value above (merged-cell artifacts) |
| `drop blanks [rows\|cols]` | trim empty edge rows and columns |
| `describe` | advisory region report — never mutates |
| `show` | print the addressed cells (the default when no command is given) |

Each command enforces a **scope contract**; when the command and address disagree,
xled refuses with a correction naming the right form rather than guessing.

## Expressions and the type model (the part agents get wrong)

`= expr` computes one column. Values are **string, number, bool, or date**, and there
is **no automatic coercion**. This is the whole point — it is what keeps leading zeros
and long identifiers intact — so respect it:

- **Arithmetic and numeric comparison require an explicit `num()` cast.** `[a] * [b]`
  is an error; write `num([a]) * num([b])`. `[qty] < [reorder]` compares *as strings*
  (so `"9" > "10"` is true, lexically); cast both sides: `num([qty]) < num([reorder])`.
- **Money:** numbers serialize at full `f64` precision and xled never rounds on write,
  so wrap any currency/decimal result in `round(…, 2)`.
- **Leading zeros / IDs:** they survive because cells are strings. Do *not* `num()` a
  zip or account number you want to keep padded — that throws the zeros away.
- **`lpad`/`rpad` never truncate** (a value already at/over the width is returned as-is),
  and **`mod` takes the dividend's sign** (`mod(-3,5)` is `-3`, following awk not Excel).
- A **failed cast is non-halting**: that cell is left untouched and a tally goes to
  stderr. So a bad value in one row won't abort the whole run.
- **Dates need `date()` and never a guessed layout.** A bare `date([col])` reads ISO
  only; anything else must be spelled out — `date([col], "DD/MM/YYYY")`. A non-ISO
  value under a bare `date()` **halts** (the command is under-specified, and equally
  so on every row) rather than skipping; only a value no layout can read takes the
  normal skip. Dates always write back as ISO, so the cast alone normalizes a column.
- There is **no `row()`** function — a computed cell can't read its own position; use
  the `--number` flag if you need logical row numbers.

Function library (full signatures in `reference.md`): text — `len left right mid
substr trim ltrim rtrim lpad rpad`; case — `upper lower proper` (same Unicode fold as
`s///`'s `\U`/`\L`); regex — `regexreplace regexmatch`; numbers — `round abs floor ceil
mod min max`; dates — `date text year month day weekday today`; casts and logic — `num
bool default coalesce if in`. Concatenate with `&`.

**Set membership is `in()`, not an alternation.** `in([org], "APP", "CAM", "CES")`
compares literals; `regexmatch([org], "^(?:APP|CAM|CES)$")` compiles them, so a member
carrying a metacharacter (`R+D`, `C++`, `Dept.`) never matches while a value nobody
listed (`RRD`) does — silently, on the row that matters. It addresses rows directly:
`in([org], "APP", "CAM") del`.

**Deriving one column from another by pattern is `regexreplace`, not `s///`.** `s///`
rewrites the cells it addresses, so it can only write back into the column it read. To
read `[a]` and write `[b]`, use the expression:

```sh
xled '[responsible] = regexreplace([requestor], "^(?:(?:APP|CE|T&D)\s*[-/\\ ]\s*)+", "")' f.csv
```

A `+` on the alternation collapses stacked prefixes in one pass, so a repeated prefix needs
no loop. `regexmatch(x, pat)` returns a bool for branching inside `if()`. The replacement is
xled's own dialect (`\1`, `&`, `\U`), the same one `s///` expands, and `regexreplace`
replaces every match where `s///` without `g` replaces the first. Case-insensitivity is
`(?i)` at the front of the pattern rather than a flag argument.

## The interactive REPL

`xled file.csv` opens a live buffer. Ordinary `address command` lines edit it; word
commands manage the session: **`preview <cmd>`** (see an edit without applying it),
**`undo`** (revert the last mutation), **`write [path]`** (the *only* thing that
touches disk), **`help`**, **`quit`** / **`quit!`** (discard unsaved changes).
Nothing is written until `write`.

## When to stop and switch

Two destinations, and the split is *what changes*:

- **The geometry changes, the values don't** — unpivot a wide export to long, pivot it
  back, transpose a sideways table, split one column into several by a delimiter,
  explode a `;`-delimited cell into one row per value, merge columns → **xshape**.
- **The row set changes** — join, group, aggregate, sort, dedupe, filter rows out of the
  result → **SQL/DuckDB (xql)**.

xled carves *a* rectangle and rewrites cells inside the table's existing shape; it is not
a splitter, not a query engine, and not a spreadsheet.

See `reference.md` in this directory for the complete address grammar, every command's
scope contract, the full expression-function reference, and the input-encoding notes.
