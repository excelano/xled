---
name: xled
description: >-
  Clean and compute over CSV/DSV cell values with `xled`, "sed and awk for tabular data":
  strip currency, trim, fix case, restore leading zeros, convert DD/MM dates to ISO, fill
  merged-cell blanks, derive a column, crop a table out of a title block and promote its
  header, edit in place with a backup and a report of what changed. Not for reshaping the
  grid (xshape) or for the row set: join, group, sort, dedupe, filter (xql).
---

# xled

`xled` names part of a table with spreadsheet addresses (a column, a row span, a
rectangle, a regex-matched set of cells) and rewrites those cells with sed-style `s///`
or an awk-style computed expression. `xled --help` is authoritative for flags and exit
codes; `reference.md` in this directory has the full grammar and function library.

## When it is the right tool

xled rewrites cells inside the table's existing shape. It never coerces a value, so
leading zeros, 20-digit IDs and `1E5`-looking strings come back as they went in. Python's
`csv` module keeps strings too, so fidelity alone is not the reason to choose xled. The
reasons are that a cleanup (crop, header, trim, case, currency, dates, fill-down)
is one short declarative script rather than a loop with quoting inside a bash string,
that `-i.bak` edits in place and reports on stderr how many cells changed in which
columns, and that it refuses to guess an ambiguous date layout.

Hand off when the task changes something other than cell values. Unpivot, pivot,
transpose, split a column, explode a delimited cell or merge columns is **xshape**. Join,
group, aggregate, sort, dedupe or filtering rows out of the result is **xql** (or
sqlite3/DuckDB). On an unfamiliar file, run **xray** first; it reports a buried header
and names the step to take.

## Buried headers need `--no-header`

Without `--no-header`, xled takes row 1 as the header. If a title block sits above the
real header, the title line becomes the header, every row address is off by one from
what you see, and a `crop` meant to include the header drops it, at exit 0. xled warns on
stderr when row 1 is a lone title above a wider table, but a title block it does not
recognize gets no warning. Pass `--no-header`, crop to the table, then promote its first
row:

```sh
xled --no-header '4:$ crop
1 header' report.csv
```

`header` takes its row as an address, so it is `1 header`, not `header 1`. `describe`
suggests the header row and the matching `N header` command.

## Recipes

Names are bracketed and case-sensitive (`[Annual Cost]`); bare letters and numbers are
positions. Put each command on its own line.

```sh
xled -i.bak '[annual_cost] s/[$,]//g' portfolio.csv          # strip currency, keep a backup
xled '[total] = round(num([price]) * 1.0825, 2)' products.csv # arithmetic needs num()
xled '[zip] = lpad([zip], 5, "0")' zips.csv                   # restore stripped zeros
xled '[name] = proper(trim([name]))' contacts.csv
xled '[hired] = date([hired], "DD/MM/YYYY")' staff.csv        # writes ISO
xled '[days] = today() - date([hired])' staff.csv
xled '[Vendor] fill down' merged.csv                          # merged-cell blanks
xled '[first name] rename first_name' f.csv                   # rest of line, no quoting
xled '/active/i [status] = "approved"' portfolio.csv          # only matching rows
xled 'in([org], "APP", "CAM") del' f.csv                      # delete rows in a set
owner=$(xled --raw '[owner] 3' portfolio.csv)                 # one value, unquoted
xled --count '/retire/' portfolio.csv                         # rows matched, not wc -l
```

## Rules that fail quietly

**No coercion.** `[a] * [b]` does not multiply; every row is skipped as uncomputable and
the column comes out empty at exit 0, with a tally on stderr. Write `num([a]) * num([b])`.
Comparisons are string-wise unless both sides are cast, so `"9" > "10"`. Do not `num()`
an ID or zip you want to keep padded. Numbers are written at full precision, so wrap money
in `round(…, 2)`.

**Dates.** A bare `date([col])` reads ISO only; a non-ISO value halts and names the
layouts that would read it. Spell the layout out: `date([col], "DD/MM/YYYY")`.

**An address that matches nothing still succeeds.** Under `-i` the run exits 0 and
rewrites the file. The stderr line is how you tell: `wrote f.csv — 12 cells changed in
[price]` against `f.csv rewritten unchanged — the address matched no cells`. On
`unchanged`, check the header spelling before deciding the data was already clean.

**Set membership is `in()`.** `regexmatch([org], "^(?:APP|R+D)$")` compiles the members
as regex, so `R+D` never matches itself. `in([org], "APP", "R+D")` compares literals.

**Deriving one column from another by pattern is `regexreplace`.** `s///` writes back into
the cells it read; `[b] = regexreplace([a], "^Dept\. ", "")` reads `[a]` and writes `[b]`.

An "unknown function" error means the installed xled predates that function; upgrade it
with the package manager it came from.
