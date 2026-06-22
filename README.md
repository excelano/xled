# xled — sed and awk for tabular data

xled brings the muscle memory of `sed` and `awk` to CSV and DSV files. It borrows awk's field model, sed's `s///` substitution, and ed's live in-memory buffer, and points all three at Excel-style ranges: a column by letter or name, a row span, a rectangle, a regex-selected set of cells. You address part of the table, you give it a command, and it shows you the result before anything is written.

```sh
# strip the currency formatting from the price column, in place
xled '[price] s/[$,]//g' products.csv

# derive a tax-inclusive total, rounded like money
xled '[total] = round(num([price]) * 1.0825, 2)' products.csv
```

## Why

Spreadsheets that arrive as CSV are full of small, repetitive damage: a dollar sign glued to every number, a leading apostrophe, inconsistent casing, a column that should be split, a header buried under three title rows. The reach for these is usually a throwaway pandas script or a fragile `awk -F,` one-liner that mishandles the first quoted comma. xled is the tool in between: faithful CSV parsing, two-dimensional addressing that matches how you already think about a sheet, and a transform vocabulary small enough to keep in your head.

It is deliberately not a query engine. xled rewrites cells and reshapes nothing — it never adds or removes rows behind your back, never reorders columns, never coerces a value you didn't ask it to. Join, group, aggregate, and multi-predicate query belong to SQL; xled hands those off to [xql](https://github.com/excelano/xql) rather than growing into them.

## Install

The fastest path on Linux or macOS is the prebuilt-binary installer:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/excelano/xled/releases/latest/download/xled-installer.sh | sh
```

On Windows, in PowerShell:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/excelano/xled/releases/latest/download/xled-installer.ps1 | iex"
```

The installer downloads the right tarball for your platform from the GitHub release, verifies its checksum, and drops the binary into `~/.cargo/bin` (or the equivalent on Windows). If `xled` isn't found on your `PATH` afterward, ensure `~/.cargo/bin` is on it. Releases also ship raw tarballs (`xled-*.tar.xz` / `.zip`) for manual installation.

To uninstall, remove the binary: `rm ~/.cargo/bin/xled`.

### Debian and Ubuntu

Install from the [Excelano apt repository](https://excelano.com/apt/), so `apt upgrade` keeps it current:

```sh
curl -fsSL https://excelano.com/apt/setup.sh | sudo sh
sudo apt install xled
```

Both amd64 and arm64 packages ship with every release.

## Build from source

xled requires only a Rust toolchain. Four pure-Rust crates carry the load (`regex`, `csv`, `clap`, `rustyline`); there are no C dependencies and no runtime.

```sh
cd xled
cargo build --release
```

The binary is at `target/release/xled`.

## Three ways to run it

```sh
xled '<script>' file.csv     # one-shot: run the script, print the result to stdout
… | xled '<script>'          # one-shot over piped stdin
xled file.csv                # open the interactive REPL on a file
```

In one-shot mode the data goes to stdout (clean, ready to pipe) and any advisory notices go to stderr, so `xled … file.csv > out.csv` is always safe. The REPL previews edits, keeps an undo stack, and writes only when you tell it to.

A statement is `address command`, one per line. The address picks the cells; the command acts on them. Either part can stand alone: an address by itself shows those cells, and a command with no address acts on the whole table.

## Addresses

Positional addresses are bare; names are bracketed. That one rule resolves every ambiguity a real header throws at you.

| Address | Selects |
|---|---|
| `C` | the column at letter C (past Z too: `AA`, `BC`, `CQ`) |
| `[price]` | the column named `price` — exact, case-sensitive |
| `3` | row 3 |
| `2:4` | rows 2 through 4 |
| `B2:C3` | the rectangle from B2 to C3 |
| `[price (USD)]` | a name containing spaces, slashes, or parens — brackets quote it |
| `/active/` | every cell matching the regex |
| `[status]~/active/` | cells in `[status]` matching the regex |
| `/active/i [status]` | combine row-select and column to a scoped set |

Brackets disambiguate the hard cases for free: the column *named* `B` is `[B]` while the column *at* letter B is `B`, and the header `2024` is `[2024]` while row 2024 is `2024`. Names match exactly — `[userId]` is not `[userid]` — because a header is data and silent case-folding is the same class of surprise as dropping a leading zero. Add the `i` flag to a regex for a case-insensitive match when you want one.

## Commands

| Command | Does |
|---|---|
| `s/re/replacement/flags` | sed substitution over the addressed cells (`g`, `i`, an occurrence number, `\1`–`\9`, `&`, `\U \L \u \l \E`) |
| `= expr` | compute a value into one column, creating it if new |
| `del` | delete whole rows or whole columns |
| `crop` | reduce the buffer to one rectangle (carve a table out of junk) |
| `header N` | promote row N to the column-name header |
| `rename newname` | rename a header in place (takes the rest of the line, no quoting needed) |
| `fill` / `fill down` | fill blank cells from the value above (merged-cell artifacts) |
| `drop blanks [rows\|cols]` | trim empty edge rows and columns |
| `describe` | advisory region report — preamble, blank edges, suspected header and total rows; never mutates |
| `show` | print the addressed cells (the default when a command is omitted) |

Each command enforces a scope contract. `= expr` writes exactly one column; `del` takes whole rows xor whole columns, never a partial rectangle; `header` and `rename` take one row or one column. When a command and an address disagree, xled refuses with a correction that names the right form rather than guessing.

## Expressions

`= expr` is the compute layer. Values are one of three types — string, number, bool — and there is **no automatic coercion**: arithmetic requires numbers, and you cast explicitly with `num()` or `bool()`. That is what keeps leading zeros and long identifiers intact. A cast that fails is non-halting: the cell is left untouched and a tally tells you how many were skipped.

```sh
[total]  = round(num([price]) * [qty], 2)        # arithmetic, money-rounded
[full]   = [first] & " " & [last]                # concatenation
[low]    = num([qty]) < num([reorder])           # a boolean column
[owner]  = default([owner], "Unassigned")        # fill blanks
[flag]   = if(num([qty]) < num([reorder]), "REORDER", "ok")
```

The library is `num bool len left right mid substr round default coalesce if`. Comparisons are string-wise unless both sides are cast with `num()` — `"9" > "10"` is true lexically, which is *not* numeric order — because auto-numifying would smuggle back exactly the surprises the stringly model exists to prevent.

Numbers serialize at full `f64` precision, so any currency or fixed-decimal column must be wrapped in `round(…, d)`; xled never rounds on write, because inventing precision the user didn't ask for is the same betrayal as silent coercion.

## What xled does not do

Query, join, aggregate, group, and sort are out of scope — that is [xql](https://github.com/excelano/xql) and DuckDB territory, and xled's error messages point you there by name. Reshaping is also out: splitting one cell into several columns, collapsing a multi-row header, unpivoting, merging stacked tables. xled carves *a* rectangle and rewrites cells within the table's existing shape; it is not a splitter and not a spreadsheet.

## Implementation

xled is a hand-written recursive-descent parser over a stringly-typed buffer (`Vec<Vec<String>>` with a promotable header overlay), feeding a resolver that turns any address into a set of `(row, column)` coordinates, and an executor that applies each command under its scope contract. The `csv` crate handles the genuinely hard parsing — embedded commas, escaped quotes, embedded newlines — and unchanged cells round-trip byte-for-byte, so leading zeros and quoted fields survive untouched. The `regex` crate powers selection and the `s///` engine, whose sed-faithful replacement dialect (backreferences, `&`, and case-folding) is implemented directly over its captures.

## License

MIT. See [LICENSE](LICENSE).
