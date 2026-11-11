# Command set & execution semantics

The address grammar (Excel's reference algebra plus the `[name]`, `/re/`, comparison and call atoms) is in `ebnf.md`; this file is what follows the address, the commands, and how they execute over a buffer. Top-level shape is always `reference command`.

## Command set

Two kinds, lexically distinct from reference atoms so the space-intersection operator never collides with the address/command boundary.

### Sigil-led (recognized by the character that follows — never a column ref)

| Command | Does | Scope it expects |
|---|---|---|
| `s/re/rep/flags` | Substitute, per cell. Flags: `g` all-in-cell, `i` case-insensitive, `N` Nth occurrence. No flag = first occurrence per cell. | any cell scope |
| `= expr` | Assign / compute, evaluated per row, written to the column scope. Bare `=` is assignment only; equality in an address comparison is `==`. The expr language — operators, precedence, value types, and the function library — is specified in `expr-grammar.md`. | one column (write target) × a row scope |

`re` and `rep` may use any delimiter (`s#…#…#`) so slashes in data don't fight the syntax — standard sed. `=` is never a column letter, so it self-disambiguates.

**Replacement dialect is sed's, not the regex crate's.** xled writes its own replacement expander over the `regex` crate's captures rather than calling `.replace()`'s `$1` syntax: it supports `\1`–`\9` backrefs, `&` (whole match), and the case-folding escapes `\U \L \u \l \E`. No crate provides this (`sedregex` is a `$1` wrapper, unmaintained; `fancy-regex` adds *pattern* backrefs xled doesn't need and forfeits linear-time matching). The split between the two transform layers is in `expr-grammar.md`: `s///` rewrites characters by pattern, `= expr` computes a value, and case-folding and trimming are available in both, folding identically.

### Reserved words (≥3 letters; a column literally named one is reachable bracketed, e.g. `[fill]`)

| Command | Does | Scope it expects |
|---|---|---|
| `del` | Delete. Rows scope → drop those rows; columns scope → drop those columns. | whole rows **or** whole columns (a rectangle/cell is an error) |
| `show` | Print/inspect the scope. Also the default when a bare reference has no command. | any |
| `crop` | Make the scope the working table; discard everything outside for the session. | a rectangle/range |
| `header` | Promote the scoped row to the name overlay. | exactly one row |
| `rename` | `[old] rename newname` — rename a header in place. Takes the rest of the line as the new name, so real headers with spaces, slashes, and parens (`Owner / Contact (named)`) are nameable without quoting. | one column |
| `fill` | Forward-fill **down**: empty cells in scope take the value above (the merged-cell body artifact). Down only: filling across edges into reshaping, and a merged *header* is fixed by renaming its columns by letter. | a column or columns |
| `drop blanks` | Trim fully-empty rows and columns at the edges of the table; `rows` or `cols` restricts it to one axis. | the table's edges |
| `describe` | Report the likely table region (preamble rows, blank edges, a suspected header or totals row) and the commands that would carve it out. Advisory: it never changes the buffer. | any |

### Not separate commands (folded into the above)

- **Create a column** — assign to a new name or letter: `[markup] = num([price]) * 0.1`, or `F = …` on a narrower file. Assignment past the width appends.
- **Duplicate a column** — assignment from a column: `[price_copy] = [price]`.
- **Clear cells** — `… s/.*//` (no dedicated verb).

### Not commands

- `sort` is refused: xled never reorders rows. The refusal is in `errors.md`.
- `y/set/set/` (transliterate) does not exist; `s///` with alternation covers the common smart-punctuation case.
- **Row insert/append** is refused: generating rows changes the row set, which is the query side of the line. The refusal is in `errors.md`.

## Execution semantics

1. **Regex is per cell.** Every cell in scope is evaluated independently; there is no cross-cell or cross-column match. The "whole-row raw text" reading from ed is **not** imported (it would expose CSV quoting artifacts).
2. **Row selection aggregates per row.** `/re/` selects a row if *any* of its cells matches; `[col]~/re/` selects on that one column's cell. Both yield a row-set that intersects (space) with a column-set.
3. **`g` is within-cell.** Every in-scope cell is always visited; `g` means "all occurrences inside this cell," no flag means "first occurrence inside this cell." `g` is never "global across cells."
4. **Anchors are cell-bounded.** `^` and `$` inside `/…/` anchor to cell start/end; `^$` matches an empty cell. (In *address* position `$` means the last row — disambiguated by position: inside `/…/` it is the end-anchor.)
5. **One empty value.** A missing cell on a ragged short row and an explicitly empty cell (`,,`) are the same: the empty string `""`. Stringly-typed, no null/nil distinction. Reading a past-the-end column on a short row yields `""`.
6. **Cast failure is non-halting.** A failed `num()`/`bool()` (e.g. `num("abc")`) leaves that cell **unchanged** and adds it to a tally shown after the command ("3 cell(s) skipped (not computable): rows 12, 88, 415 — left unchanged."). Halting on row 5000 of a live session is hostile, silent corruption is worse, and a visible tally sits between them. The rule covers a bad *value*; it does not cover a bad *program*. Where a failure is a property of the command rather than of the cell — an ambiguous bare `date()`, a date function reached without the cast — it is identical on every row, so it halts once with the corrected form instead of tallying thousands of skips that all say the same thing (`expr-grammar.md`, Dates).
7. **Assignment is per-row, writes one column.** There is no separate LHS — the **column component of the address** is the write target, and it must resolve to exactly one column (or a new name/letter to create), else an error. `[total] = num([qty]) * num([price])` writes `[total]`; `2:4 = "x"` (rows only, no column) and `[a],[b] = …` (two columns) are both errors. The row component of the address scopes which rows are written, so `/active/ [status] = "approved"` writes only matching rows — the same address grammar that scopes `s///` scopes assignment, with the column part naturally becoming the target. Reads see the pre-command buffer, so `[a] = [a] + [b]` is well-defined. **Pad-on-write:** writing a column beyond a short row's current width pads the intervening cells with `""` (consistent with rule 5's missing-cell value).
8. **Header is not a data row.** Row numbers address **data** rows — row `1` is the first data row, never the header. The header overlay is never touched by a normal scope; only `header` (promote) and `rename` (edit) reach it. This is the "letters/numbers are the data grid, names are an overlay" model made executable.
9. **Commands apply sequentially; the buffer is mutable; save is deliberate.** Each command fully applies before the next (REPL or script). Nothing is written to disk until an explicit save: `write` in the REPL, `-i` in a one-shot. Every command can be previewed before it is applied: `preview` in the REPL, `--diff` in a one-shot.
10. **`del` needs a clean rank.** Deleting requires whole rows or whole columns — a partial rectangle has no meaning as a deletion. `2:4 [status] del` is an error ("this address names a rectangle, and a rectangle has no row-or-column to drop. To clear those cells, s/.*// over them; …"); `2:4 del` and `[status] del` are fine.
