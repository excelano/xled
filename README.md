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

It is deliberately not a query engine. xled rewrites cells and reshapes nothing — it never adds or removes rows behind your back, never reorders columns, never coerces a value you didn't ask it to. Group, aggregate, and multi-predicate query belong to [xql](https://github.com/excelano/xql), and a join, which needs a second table, to a full SQL engine; xled hands those off rather than growing into them.

## Install

Every install line below ends with `xled --install-skill`. That installs the [Claude Code skill](#use-it-from-claude-code) alongside the binary. Drop it if you do not use Claude Code — the CLI itself does not need it.

### Debian and Ubuntu

Add the [Excelano apt repository](https://excelano.com/apt/) once:

```sh
curl -fsSL https://excelano.com/apt/setup.sh | sudo sh
```

Then install it, so `apt upgrade` keeps it current:

```sh
sudo apt install xled && xled --install-skill
```

Both amd64 and arm64 packages ship with every release.

### Windows

With [WinGet](https://learn.microsoft.com/windows/package-manager/), so `winget upgrade` keeps it current:

```powershell
winget install Excelano.xled
xled --install-skill
```

### Cargo

If you have a Rust toolchain, install the published crate from [crates.io](https://crates.io/crates/xled). This compiles from source, so it is slower than the packages above but needs nothing else, and it is the install path on macOS:

```sh
cargo install xled && xled --install-skill
```

### Build from source

xled requires only a Rust toolchain. Its dependencies are pure Rust; there are no C dependencies and no runtime.

```sh
cd xled
cargo build --release
```

The binary is at `target/release/xled`.

## Use it from Claude Code

xled was built for AI coding agents as much as for people, so the repo ships an official [Claude Code](https://docs.claude.com/en/docs/claude-code) skill under [`skills/xled/`](skills/xled/). It teaches an agent xled's addressing model, commands, the no-coercion type rules, and the hard boundary (cell values only; reshaping, queries and joins go elsewhere), so it uses xled correctly rather than routing around it to a tool it already knows. The binary installs it:

```sh
xled --install-skill
```

That writes `~/.claude/skills/xled/` and stamps in the skill's revision, which `xled -V` also prints, so a later run reports whether the skill has fallen behind the binary rather than leaving you to notice. It is safe to re-run: an unchanged skill reports `already current` and nothing is written. `xled --uninstall-skill` removes it. Restart Claude Code afterwards, since skills are discovered at session start.

The skill is compiled into the binary, so this works the same however you installed xled — apt, cargo, or a build from source.

## Running it

```sh
xled '<script>' file.csv     # one-shot: run the script, print the result to stdout
… | xled '<script>'          # one-shot over piped stdin
xled file.csv                # open the interactive REPL on a file
```

In one-shot mode the data goes to stdout (clean, ready to pipe) and any advisory notices go to stderr, so `xled … file.csv > out.csv` is always safe. The REPL previews edits, keeps an undo stack, and writes only when you tell it to.

Two flags follow sed and awk directly. `-i` (`--in-place`) edits the file where it sits instead of printing to stdout, and an attached suffix keeps the original as a backup, as `sed -i.bak` does:

```sh
xled -i '[price] s/[$,]//g' file.csv       # rewrite file.csv in place
xled -i.bak '[price] s/[$,]//g' file.csv   # …and save the original as file.csv.bak
```

An in-place run prints nothing to stdout, so it reports what it did on stderr instead:

```
xled: wrote products.csv — 12 cells changed in [price]
xled: products.csv rewritten unchanged — the address matched no cells, or the script wrote the values already there
```

A script whose address matched nothing still exits 0 and still rewrites the file, so a silent success would look like a successful edit; `unchanged` is what tells you the header name was misspelled rather than the data being already clean.

`--diff` shows which cells a script changes, one line per cell with its address, old value and new value. On its own it is a preview that leaves the file alone; with `-i` the file is written and the lines record what changed:

```
$ xled --diff '[price] s/[$,]//g' products.csv
xled: 2 cells changed in [price]
C1 [price]: "$1,200" → "1200"
C2 [price]: "$30" → "30"
```

`-f` (`--file`) reads the script from a file rather than the command line, which avoids the `"$(cat …)"` dance when a batch of edits grows past a comfortable one-liner:

```sh
xled -f batch.xled file.csv                # run the script in batch.xled
xled -i -f batch.xled file.csv             # …and apply it in place
```

In-place editing is for scripts that change cells; run an inspect-only script (a bare address, or `show`) without `-i` so its output prints rather than overwriting the file.

A statement is `address command`, one per line. The address picks the cells; the command acts on them. Either part can stand alone: an address by itself shows those cells, and a command with no address acts on the whole table.

## The language

Positional addresses are bare and names are bracketed: `C` is the column at letter C, `[price]` the column named price, `2:4` a row span, `B2:C3` a rectangle, `/active/` the rows where any cell matches, and `num([qty]) < num([reorder])` the rows where one comparison holds. A space intersects two addresses and a comma unions them, as in Excel. The commands are sed's `s///`, an awk-style `= expr` that computes one column, and a few structural words for carving a table out of a messy export: `crop`, `header`, `drop blanks`, `fill`, `del`, `rename`, `describe` and `show`.

```sh
[total]  = round(num([price]) * [qty], 2)        # arithmetic, money-rounded
[name]   = proper(trim([name]))                  # tidy casing and stray spaces
[zip]    = lpad([zip], 5, "0")                   # restore a stripped leading zero
[hired]  = date([hired], "DD/MM/YYYY")           # normalize a date column to ISO
```

Two rules shape the expression language. There is no automatic coercion: arithmetic needs `num()`, and comparisons are string-wise unless both sides are cast, which is what keeps leading zeros and long IDs intact. And xled never guesses DD/MM versus MM/DD: a bare `date()` reads ISO only, and a value that another layout could read stops the run with the readings named.

The full address grammar, each command's scope contract, the function library, and the `--raw`, `--number` and `--count` flags for reading values are in the [reference](skills/xled/reference.md).

## Input encoding

xled expects UTF-8. An Excel "Save as CSV UTF-8" BOM at the start of the file is kept out of the first column header and written back on output. If the file looks like UTF-7 (the `+ACI-` escape that Scoutbook exports emit) or carries a UTF-16 BOM, xled prints a warning at startup with the `iconv` command needed to convert it to UTF-8 first. UTF-16 fails the underlying read; the warning lets you fix the file instead of staring at a "stream did not contain valid UTF-8" error.

## What xled does not do

Query, aggregate, group, and sort are out of scope; that is [xql](https://github.com/excelano/xql) territory, and a join belongs to a SQL engine such as DuckDB. Reshaping is also out, and has its own tool: splitting one cell into several columns, unpivoting a wide export, pivoting, exploding a delimited cell, transposing a sideways table, merging columns — all of that is [xshape](https://github.com/excelano/xshape). When a script asks for one of these, xled refuses and names a command that does it; several of those refusals give a DuckDB query. xled carves *a* rectangle and rewrites cells within the table's existing shape; it is not a splitter and not a spreadsheet.

## Implementation

xled is a hand-written recursive-descent parser over a stringly-typed buffer (`Vec<Vec<String>>` with a promotable header overlay), feeding a resolver that turns any address into a set of `(row, column)` coordinates, and an executor that applies each command under its scope contract. The `csv` crate handles the genuinely hard parsing — embedded commas, escaped quotes, embedded newlines — and rows a script does not change are written back byte-for-byte, line endings and quoting included, so leading zeros and quoted fields survive untouched. The `regex` crate powers selection and the `s///` engine, whose sed-faithful replacement dialect (backreferences, `&`, and case-folding) is implemented directly over its captures.

## License

MIT. See [LICENSE](LICENSE).
