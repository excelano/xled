# Command set & execution semantics

Slice 2 locks. The address grammar (Excel reference algebra + `[name]`/`/re/` atoms) lives in `composition-grammar.md`; this is what follows the address — the commands — and how they execute over a buffer. Top-level shape is always `reference command`.

## Command set

Two kinds, lexically distinct from reference atoms so the space-intersection operator never collides with the address/command boundary.

### Sigil-led (recognized by the character that follows — never a column ref)

| Command | Does | Scope it expects |
|---|---|---|
| `s/re/rep/flags` | Substitute, per cell. Flags: `g` all-in-cell, `i` case-insensitive, `N` Nth occurrence. No flag = first occurrence per cell. | any cell scope |
| `= expr` | Assign / compute, evaluated per row, written to the column scope. Bare `=` is assignment only; equality in an address comparison is `==`. The expr language (operators, precedence, function library `num bool len left right mid round default coalesce if`) is specified in `expr-grammar.md`. | one column (write target) × a row scope |

`re` and `rep` may use any delimiter (`s#…#…#`) so slashes in data don't fight the syntax — standard sed. `=` is never a column letter, so it self-disambiguates.

**Replacement dialect is sed's, not the regex crate's.** xled writes its own replacement expander over the `regex` crate's captures rather than calling `.replace()`'s `$1` syntax: it supports `\1`–`\9` backrefs, `&` (whole match), and the case-folding escapes `\U \L \u \l \E`. No crate provides this (`sedregex` is a `$1` wrapper, unmaintained; `fancy-regex` adds *pattern* backrefs xled doesn't need and forfeits linear-time matching). This is why **case-folding and trimming live in `s///`, not in expr** — they are pattern rewrites of text. The split is sharp: `s///` rewrites characters by pattern (sub, capture, case, trim, whitespace); `= expr` computes a value (`expr-grammar.md`).

### Reserved words (≥3 letters; a column literally named one is reachable bracketed, e.g. `[fill]`)

| Command | Does | Scope it expects |
|---|---|---|
| `del` | Delete. Rows scope → drop those rows; columns scope → drop those columns. | whole rows **or** whole columns (a rectangle/cell is an error) |
| `show` | Print/inspect the scope. Also the default when a bare reference has no command. | any |
| `crop` | Make the scope the working table; discard everything outside for the session. | a rectangle/range |
| `header` | Promote the scoped row to the name overlay. | exactly one row |
| `rename` | `[old] rename newname` — rename a header in place. Takes the rest of the line as the new name, so real headers with spaces, slashes, and parens (`Owner / Contact (named)`) are nameable without quoting. | one column |
| `fill` | Forward-fill **down**: empty cells in scope take the value above (the merged-cell body artifact). Down-only in v1 — across-fill edges into reshaping and the merged-*header* case is handled by rename-by-letter instead. | a column or columns |

### Not separate commands (folded into the above)

- **Create a column** — assign to a new name or letter: `[markup] = num([price]) * 0.1`, or `F = …` on a narrower file. Assignment past the width appends.
- **Duplicate a column** — assignment from a column: `[price_copy] = [price]`.
- **Clear cells** — `… s/.*//` (no dedicated verb).

### Deliberately rejected / deferred

- `sort` — **rejected by design**: `[col] sort` errors and points at `sort`/DuckDB (reshaping toward query). The message is written in `errors.md` (out-of-scope → query engine).
- `y/set/set/` (transliterate) — **deferred**: niche; `s///` with alternation covers the common smart-punctuation case. Revisit only if a real need appears.
- **Row insert/append** — the one structural gap with no clean home yet (assignment creates *columns*, not rows). Flagged open below.

## Execution semantics

1. **Regex is per cell.** Every cell in scope is evaluated independently; there is no cross-cell or cross-column match. The "whole-row raw text" reading from ed is **not** imported (it would expose CSV quoting artifacts).
2. **Row selection aggregates per row.** `/re/` selects a row if *any* of its cells matches; `[col]~/re/` selects on that one column's cell. Both yield a row-set that intersects (space) with a column-set.
3. **`g` is within-cell.** Every in-scope cell is always visited; `g` means "all occurrences inside this cell," no flag means "first occurrence inside this cell." `g` is never "global across cells."
4. **Anchors are cell-bounded.** `^` and `$` inside `/…/` anchor to cell start/end; `^$` matches an empty cell. (In *address* position `$` means the last row — disambiguated by position: inside `/…/` it is the end-anchor.)
5. **One empty value.** A missing cell on a ragged short row and an explicitly empty cell (`,,`) are the same: the empty string `""`. Stringly-typed, no null/nil distinction. Reading a past-the-end column on a short row yields `""`.
6. **Cast failure is non-halting.** A failed `num()`/`bool()` (e.g. `num("abc")`) leaves that cell **unchanged** and increments a warning tally shown after the op ("3 cells skipped: not numeric"). Halting on row 5000 of a live session is hostile; silent corruption is worse; a visible tally is the trustworthy middle. **Lenient is the locked default** (David confirmed 2026-06-21); a strict-halt mode is a later opt-in, not the baseline.
7. **Assignment is per-row, writes one column.** There is no separate LHS — the **column component of the address** is the write target, and it must resolve to exactly one column (or a new name/letter to create), else an error. `[total] = num([qty]) * num([price])` writes `[total]`; `2:4 = "x"` (rows only, no column) and `[a],[b] = …` (two columns) are both errors. The row component of the address scopes which rows are written, so `/active/ [status] = "approved"` writes only matching rows — the same address grammar that scopes `s///` scopes assignment, with the column part naturally becoming the target. Reads see the pre-command buffer, so `[a] = [a] + [b]` is well-defined. **Pad-on-write:** writing a column beyond a short row's current width pads the intervening cells with `""` (consistent with rule 5's missing-cell value).
8. **Header is not a data row.** Row numbers address **data** rows — row `1` is the first data row, never the header. The header overlay is never touched by a normal scope; only `header` (promote) and `rename` (edit) reach it. This is the "letters/numbers are the data grid, names are an overlay" model made executable.
9. **Commands apply sequentially; the buffer is mutable; save is deliberate.** Each command fully applies before the next (REPL or script). Nothing is written to disk until an explicit save. Operations should be previewable before commit (the trust requirement).
10. **`del` needs a clean rank.** Deleting requires whole rows or whole columns — a partial rectangle has no meaning as a deletion. `2:4 [status] del` is an error ("can't delete a partial region — clear it with `s/.*//`, or address whole rows/columns"); `2:4 del` and `[status] del` are fine.

## Resolved at slice-3 render (2026-06-21)

- **`s///` replacement dialect is sed's, written by xled** — `\1`–`\9`, `&`, `\U \L \u \l \E` over the `regex` crate's captures. No crate provides it. Solves capture-rearrange (B3) and case (B4) together; case/trim stay in `s///`.
- **The compute layer is specified** in `expr-grammar.md` (value model, operators/precedence, function library). The "thin compute layer" was load-bearing for B9/B10/B8 and is no longer hand-waved.
- **Comparisons are string-wise unless `num()`-cast.** `[qty] < [reorder]` is lexicographic; numeric order needs `num([qty]) < num([reorder])`. Operands are exprs (so `num([amount]) < 0` is legal). Consistent with no-coercion; the footgun is documented.
- **`if(cond, a, b)`** chosen over awk's `?:` (the `:`-collides-with-range tiebreaker; reuses function-call machinery). It is the no-control-flow line: conditional *expression* in, statement branching/loops out.
- **Bracket-escape is `]]`** — a header containing `]` is addressed by doubling it (`[notes [draft]]]`).
- **Pad-on-write `""`** for assignment past a short row's width (rule 7).

## Resolved at slice-2 review (2026-06-21)

- **Equality is `==`, assignment is bare `=`.** awk-faithful; resolves the overload that made `[price] = [cost]` ambiguous. Full comparison set `== != < > <= >=` lives in `composition-grammar.md`.
- **`rename` takes rest-of-line** so spaced/sla­shed/parenthesized headers are nameable.
- **`fill` is down-only in v1.** Merged-header case → rename-by-letter, not across-fill (taxonomy updated to match).
- **Cast failure stays lenient + tally** (David confirmed). Strict-halt is a later opt-in.

## Still open — row insert/append

The only structural operation assignment doesn't cover (assignment creates *columns*, not rows). **Deferred to the Part B/C battery to design, not to guess now** (David's call). xled edits existing rows; row *generation* leans upstream — but David expects a real need will surface (e.g. appending a record or a computed totals row), and if the battery shows it more than once we design `append` against the actual shapes rather than inventing syntax in the dark. Until then a row-append attempt gets the transient "not available yet" refusal in `errors.md` (with the `printf … >> file.csv` workaround), not a permanent rejection.
