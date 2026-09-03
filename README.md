# xled — sed and awk for tabular data

xled brings the muscle memory of `sed` and `awk` to CSV and DSV files. It borrows awk's field model, sed's `s///` substitution, and ed's live in-memory buffer, and points all three at Excel-style ranges: a column by letter or name, a row span, a rectangle, a regex-selected set of cells. You address part of the table, you give it a command, and it shows you the result before anything is written.

**Project page:** [https://excelano.com/xled/](https://excelano.com/xled/) · **Tutorial:** [an introduction](https://excelano.com/xled/tutorial/)

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

Every install line below ends with `xled --install-skill`. That installs the [Claude Code skill](#use-it-from-claude-code) alongside the binary, which is the one step people reliably skipped when it lived further down the page. Drop it if you do not use Claude Code — the CLI itself does not need it.

### Debian and Ubuntu

Add the [Excelano apt repository](https://excelano.com/apt/) once (one-time setup):

```sh
curl -fsSL https://excelano.com/apt/setup.sh | sudo sh
```

Then install it, so `apt upgrade` keeps it current:

```sh
sudo apt install xled && xled --install-skill
```

Both amd64 and arm64 packages ship with every release.

### Homebrew

On macOS or Linux, tap and trust the repository once — Homebrew gates third-party taps behind explicit trust (one-time setup):

```sh
brew tap excelano/tap
brew trust excelano/tap
```

Then install it, so `brew upgrade` keeps it current:

```sh
brew install xled && xled --install-skill
```

### Windows

With [WinGet](https://learn.microsoft.com/windows/package-manager/), so `winget upgrade` keeps it current:

```powershell
winget install Excelano.xled
xled --install-skill
```

Or run the standalone installer in PowerShell:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/excelano/xled/releases/latest/download/xled-installer.ps1 | iex"
```

### Prebuilt binary (Linux and macOS)

```sh
curl -fsSL https://raw.githubusercontent.com/excelano/xled/main/install.sh | sh
```

The installer downloads the right tarball for your platform from the GitHub release, verifies its checksum, and drops the binary into `~/.cargo/bin` (or the equivalent on Windows). If `xled` isn't found on your `PATH` afterward, ensure `~/.cargo/bin` is on it. Releases also ship raw tarballs (`xled-*.tar.xz` / `.zip`) for manual installation. To uninstall:

```sh
curl -fsSL https://raw.githubusercontent.com/excelano/xled/main/uninstall.sh | sh
```

That removes the binary from `~/.cargo/bin`; you can also just `rm ~/.cargo/bin/xled`.

### Cargo

If you have a Rust toolchain, install the published crate from [crates.io](https://crates.io/crates/xled). This compiles from source rather than fetching a prebuilt binary, so it is slower than the installer above but needs nothing else:

```sh
cargo install xled && xled --install-skill
```

### X-CMD

[x-cmd](https://www.x-cmd.com/) is a modern Shell toolkit that gives AI agents and developers powerful, portable, and composable command-line capabilities.

```bash
x eget use excelano/xled
```

### Build from source

xled requires only a Rust toolchain. Four pure-Rust crates carry the load (`regex`, `csv`, `clap`, `rustyline`); there are no C dependencies and no runtime.

```sh
cd xled
cargo build --release
```

The binary is at `target/release/xled`.

## Use it from Claude Code

xled was built for AI coding agents as much as for people, so the repo ships an official [Claude Code](https://docs.claude.com/en/docs/claude-code) skill under [`skills/xled/`](skills/xled/). It teaches an agent xled's addressing model, commands, the no-coercion type rules, and the hard boundary (never reshapes — reach for SQL/DuckDB instead), so it uses xled correctly rather than routing around it to a tool it already knows. The binary installs it:

```sh
xled --install-skill
```

That writes `~/.claude/skills/xled/` and stamps in the version it came from, so a later run reports whether the skill has fallen behind the binary rather than leaving you to notice. It is safe to re-run: an unchanged skill reports `already current` and nothing is written. `xled --uninstall-skill` removes it. Restart Claude Code afterwards, since skills are discovered at session start.

The skill is compiled into the binary, so this works the same however you installed xled — apt, Homebrew, cargo, the curl one-liner, or a build from source.

## Three ways to run it

```sh
xled '<script>' file.csv     # one-shot: run the script, print the result to stdout
… | xled '<script>'          # one-shot over piped stdin
xled file.csv                # open the interactive REPL on a file
```

In one-shot mode the data goes to stdout (clean, ready to pipe) and any advisory notices go to stderr, so `xled … file.csv > out.csv` is always safe. The REPL previews edits, keeps an undo stack, and writes only when you tell it to.

Two flags follow sed and awk directly. `-i` (`--in-place`) edits the file where it sits instead of printing to stdout, and an attached suffix keeps the original as a backup, exactly as `sed -i.bak` does:

```sh
xled -i '[price] s/[$,]//g' file.csv       # rewrite file.csv in place
xled -i.bak '[price] s/[$,]//g' file.csv   # …and save the original as file.csv.bak
```

An in-place run prints nothing to stdout, so it reports what it did on stderr instead:

```
xled: wrote products.csv — 12 cells changed in [price]
xled: products.csv rewritten unchanged — the address matched no cells, or the script wrote the values already there
```

That second line is worth reading. A script whose address matched nothing still exits 0 and still rewrites the file, so a silent success would look exactly like a successful edit; `unchanged` is what tells you the header name was misspelled rather than the data being already clean.

`-f` (`--file`) reads the script from a file rather than the command line, which avoids the `"$(cat …)"` dance when a batch of edits grows past a comfortable one-liner:

```sh
xled -f batch.xled file.csv                # run the script in batch.xled
xled -i -f batch.xled file.csv             # …and apply it in place
```

In-place editing is for scripts that change cells; run an inspect-only script (a bare address, or `show`) without `-i` so its output prints rather than overwriting the file.

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
| `num([qty]) < num([reorder])` | rows where a comparison holds |
| `regexmatch([sku], "^TL-")` | rows where a bool-valued function is true |

Brackets disambiguate the hard cases for free: the column *named* `B` is `[B]` while the column *at* letter B is `B`, and the header `2024` is `[2024]` while row 2024 is `2024`. Names match exactly — `[userId]` is not `[userid]` — because a header is data and silent case-folding is the same class of surprise as dropping a leading zero. Add the `i` flag to a regex for a case-insensitive match when you want one.

The last two rows are the same kind of atom: a test evaluated per row, selecting the ones it answers true for. A function that already returns a bool needs no `== true` after it. There is deliberately no `and` or `or` to chain two of them — one condition per address, and a real multi-predicate filter is a query, so run a second xled command on the result or reach for xql.

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

## Reading values

A read prints CSV by default, header and all, which is the right thing when the result is a table. Two flags reshape it for scripting. `--raw` drops the header and the CSV quoting and prints just the addressed values, one per line, so a single-cell lookup is the value and nothing else:

```sh
xled --raw '[status] 12' file.csv      # -> approved
owner=$(xled --raw '[owner] 3' file.csv)
```

`--number` prefixes each row with its logical row number — the number xled itself addresses by. That matters because a cell may hold an embedded newline: piping a column into `nl` counts physical lines and drifts out of sync for every row after the first multiline value, while `--number` stays keyed to the real row:

```sh
xled --number '[description]' file.csv    # a reliable row-number -> value map
xled --raw --number '[description]' file.csv
```

`--count` answers how many rows an address selected instead of printing them — `grep -c` for an address. It counts rows, de-duplicated, so a match spanning three columns of one row counts once:

```sh
xled --count '/active/' file.csv          # -> 42
[ "$(xled --count '/error/' log.csv)" -gt 0 ] && echo "found some"
```

Reach for it instead of piping to `wc -l`, which counts physical lines and so overcounts any table holding a cell with a newline in it — the same drift `--number` exists to correct. An address that matches nothing counts 0 and exits 0: the empty result is the answer, not a failure. Because there is nothing to format about a single integer, `--count` refuses to combine with `--raw` or `--number` rather than letting one of them quietly win.

All three shape the output of a read (a bare address or `show`); on a script that changes cells they have nothing to report on and are ignored. There is deliberately no `row()` function inside a compute — a computed cell sees values, not its own position — so `--number` is the one way to surface row numbers.

## Expressions

`= expr` is the compute layer. Values are one of four types — string, number, bool, date — and there is **no automatic coercion**: arithmetic requires numbers, and you cast explicitly with `num()`, `bool()`, or `date()`. That is what keeps leading zeros and long identifiers intact. A cast that fails is non-halting: the cell is left untouched and a tally tells you how many were skipped.

```sh
[total]  = round(num([price]) * [qty], 2)        # arithmetic, money-rounded
[full]   = [first] & " " & [last]                # concatenation
[name]   = proper(trim([name]))                  # tidy casing and stray spaces
[zip]    = lpad([zip], 5, "0")                   # restore a stripped leading zero
[hired]  = date([hired], "DD/MM/YYYY")           # normalize a date column to ISO
[days]   = today() - date([hired])               # tenure, in days
[low]    = num([qty]) < num([reorder])           # a boolean column
[owner]  = default([owner], "Unassigned")        # fill blanks
[flag]   = if(num([qty]) < num([reorder]), "REORDER", "ok")
```

The library groups into text handling — `len left right mid substr trim ltrim rtrim lpad rpad` for measuring, slicing, stripping whitespace, and padding to a fixed width — regex — `regexreplace regexmatch`, which is how a pattern derives *another* column, since `s///` can only write back into the column it reads — case-folding — `upper lower proper`, the same Unicode dialect as `s///`'s `\U`/`\L` so the two never disagree — numbers — `round abs floor ceil mod min max` — dates — `date text year month day weekday today` — and casts and logic — `num bool default coalesce if in`. `in([org], "APP", "CAM", "CES")` is set membership, and it compares its members as literals rather than compiling them: the alternation it replaces matches inside `APPLE` and `SCAM` when the anchors are forgotten, and treats a member like `R+D` as a pattern when they are not, so the one value you are testing for is the one that fails. Padding never truncates: `lpad` on a value already at or past the width returns it unchanged, because dropping characters is the same betrayal as coercion. `mod` takes the dividend's sign (`mod(-3, 5)` is `-3`), following awk rather than Excel, and a divide-by-zero leaves the cell untouched. Comparisons are string-wise unless both sides are cast with `num()` — `"9" > "10"` is true lexically, which is *not* numeric order — because auto-numifying would smuggle back exactly the surprises the stringly model exists to prevent.

Numbers serialize at full `f64` precision, so any currency or fixed-decimal column must be wrapped in `round(…, d)`; xled never rounds on write, because inventing precision the user didn't ask for is the same betrayal as silent coercion.

### Dates

Dates are a real type, and they always write as ISO 8601. That is what lets a single cast normalize a column — `[hired] = date([hired], "DD/MM/YYYY")` needs no formatting call — and it means the result still sorts correctly anywhere downstream that treats it as text. Format tokens are Excel's (`YYYY MMMM MMM MM M DDDD DDD DD D`), not strftime's, because that is the dialect the audience already knows.

The rule worth knowing before you start is that **xled never guesses DD/MM versus MM/DD**. A bare `date([col])` reads ISO and nothing else — including the basic form `20240304` and a timestamp truncated to its date. Anything else has to be spelled out, and if you don't, the error names the readings rather than picking one:

```
03/04/2024 is ambiguous: both DD/MM/YYYY and MM/DD/YYYY parse it.
Say which one: date([col], "DD/MM/YYYY")
```

That halt is deliberate and it is different from a skipped cell. A value no layout can read is bad data: the cell is left alone and tallied, like any other cast failure. A value that some layout *can* read means the command is under-specified, which is wrong on every row equally — so it stops once instead of burying the fix in a warning count.

Subtracting two dates gives a number of days; adding or subtracting a number gives a date. `year` `month` `day` pull components out for grouping, `weekday` returns the ISO day of week (1 = Monday, not Excel's 1 = Sunday), `text(d, fmt)` renders a date back out under any Excel format, and `today()` is fixed once for the whole run so a long job can't straddle midnight and stamp two different days into one column. Month arithmetic is deliberately absent: "January 31 plus one month" has no correct answer, only policies, and picking one quietly would be the same mistake as guessing the layout.

## Input encoding

xled expects UTF-8. An Excel "Save as CSV UTF-8" BOM at the start of the file is stripped so the first column header is not prefixed with it. If the file looks like UTF-7 (the `+ACI-` escape that Scoutbook exports emit) or carries a UTF-16 BOM, xled prints a warning at startup with the `iconv` command needed to convert it to UTF-8 first. UTF-16 fails the underlying read; the warning lets you fix the file instead of staring at a "stream did not contain valid UTF-8" error.

## What xled does not do

Query, join, aggregate, group, and sort are out of scope — that is [xql](https://github.com/excelano/xql) and DuckDB territory, and xled's error messages point you there by name. Reshaping is also out, and has its own tool: splitting one cell into several columns, unpivoting a wide export, pivoting, exploding a delimited cell, transposing a sideways table, merging columns — all of that is [xshape](https://github.com/excelano/xshape). xled carves *a* rectangle and rewrites cells within the table's existing shape; it is not a splitter and not a spreadsheet.

## Implementation

xled is a hand-written recursive-descent parser over a stringly-typed buffer (`Vec<Vec<String>>` with a promotable header overlay), feeding a resolver that turns any address into a set of `(row, column)` coordinates, and an executor that applies each command under its scope contract. The `csv` crate handles the genuinely hard parsing — embedded commas, escaped quotes, embedded newlines — and unchanged cells round-trip byte-for-byte, so leading zeros and quoted fields survive untouched. The `regex` crate powers selection and the `s///` engine, whose sed-faithful replacement dialect (backreferences, `&`, and case-folding) is implemented directly over its captures.

## License

MIT. See [LICENSE](LICENSE).
