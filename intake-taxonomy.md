# Intake taxonomy — what people do to CSVs in Excel

A catalog of the structural and content pathologies a human-authored spreadsheet carries when it lands as a CSV. It drives two things: the **Part C — structural intake** battery in `proving-ground.md`, and a second synthetic fixture set (`fixtures/messy/`, built later from real `.xlsx` conversions). Examples here are synthetic; shapes tagged *(seen)* were confirmed in real human-authored exports.

## The governing approach

Restated from the design discussion so the table below reads against it. Messy-table handling is an **addressing problem, not a detection problem**. The real table is a rectangle buried in junk; the human's instinct in Excel is to *select the real range*, and ranges are exactly what xled is best at. So the answer is a small set of deterministic primitives, every one of which is addressing:

- **`crop` / use-range** — declare the working table as a rectangle, discard surrounding junk for the session.
- **`header N`** — promote row N to the name overlay (the header needn't be row 1; the overlay model already allows this).
- **`drop blanks`** — remove blank rows / blank columns.
- **`fill down`** — forward-fill a grouping column where the value appears once then blanks beneath it.
- **`describe`** — *advisory* detection only: report the best-guess table region ("3 preamble rows, header at 4, blank-row break at 204") and never act on it. Detection as advice the human confirms, never magic that fires silently.
- **`s///` + cast** — the core transforms, once the table is carved, do the value-level cleaning.
- **tolerant read** — the parser absorbs ragged rows, BOM, mixed line endings, inconsistent quoting on load.

Two honest limits stated up front. Some Excel damage is **irreversible by the time it is CSV** — a leading zero already stripped, a long ID already in scientific notation, a code already auto-converted to a date — and no tool downstream can un-destroy it; the fix is at the source (re-export as text, or open correctly). And heavy *reshaping* — collapsing genuinely multi-row headers, unpivoting, merging stacked tables into one — stays **out**, the same line we drew for joins and aggregates. xled carves one rectangular table out of junk and cleans its cells. It does not restructure the data's shape.

Severity legend: **common** (most real exports) · **occasional** · **rare**. Approach legend: `crop` `header N` `drop blanks` `fill down` `describe` `s///`/`cast` `tolerant read` · **upstream** (lossy / fix at source) · **out** (deliberately not ours → cleaner/DuckDB/xql).

---

## Group 1 — Junk around the table

| Pathology | What it looks like | Approach | Severity |
|---|---|---|---|
| Title / preamble rows | One or more report-title / "Generated 2026-06-21" / department lines above the header | `crop` / `header N` (advised by `describe`) | common |
| Leading blank rows | Empty rows between a title block and the table | `crop` / `drop blanks` | common |
| Trailing footnotes | "Source: …", disclaimers, a totals line, notes below the last record | `crop` | common |
| Trailing empty columns | Every row ends in one or more empty fields, `,,` *(seen)* | `drop blanks` (cols) | common |
| Side-notes column | A free-text column off to the right, often after a blank spacer column | `crop` / `drop blanks` | occasional |
| Leading spacer column | A blank col A used as a margin or to hold an unlabeled row index, so the table starts at col B with a blank header cell *(seen)* | `crop` (exclude col A) / `drop blanks` if fully empty; reach by letter if it holds an index | common |

## Group 2 — Header problems

| Pathology | What it looks like | Approach | Severity |
|---|---|---|---|
| No header at all | Data starts on row 1; columns reachable only by letter *(seen)* | letters intrinsic; optional `header` later | common |
| Header not on row 1 | Preamble pushes the real header to row 3–5 | `header N` | common |
| Multi-row header | A category row plus a units/sub-label row that *together* name a column | `header N` for the simple case; collapsing two rows into one is **out** (reshaping) | occasional |
| Merged header cell | One label spanning several columns, exported as label + blanks | rename by letter (`fill` is down-only — across-fill is reshaping); deep cases **out** | occasional |
| Duplicate header names | Two columns named the same *(seen — side-by-side tables repeat a header across the gap)* | reach by letter; names ambiguous (case-sensitive exact still collides) — `describe` warns | occasional |
| Blank header cell | A named file with one unnamed column | reach by letter | occasional |
| Units in the header text | `Amount ($)`, `Weight (kg)`, `Cost (USD)` | addressed via brackets `[Amount ($)]`; strip units with `s///` if wanted | common |

## Group 3 — Grouping / merged-cell artifacts in the body

| Pathology | What it looks like | Approach | Severity |
|---|---|---|---|
| Forward-fill needed | A grouping column with the value once, then blanks beneath (merged-cell export) *(seen)* | `fill down` | common |
| Repeated parent value | The same grouping label repeated on every child row *(seen)* | already filled; no action, or it's intended | common |
| Outline / hierarchy | Levels encoded by dotted IDs (`1 > 1.1 > 1.2`) or by leading-space indent *(seen — dotted IDs and parent-ref both observed)* | regex on the ID to derive level; structural unnest is **out** | occasional |
| Subtotal / total rows | A "Total"/"Subtotal" row interleaved or at the bottom — an aggregate, not a record *(seen)* | delete by `/^(sub)?total/i` match; recompute is **out** | common |
| Section-label rows | A row that is really a category banner spanning the width, other cells empty | delete or `crop` past it; `describe` flags it | occasional |

## Group 4 — Multiple tables in one file

| Pathology | What it looks like | Approach | Severity |
|---|---|---|---|
| Stacked tables | Two+ tables one above another, each with its own header, blank-row separated *(seen — incl. a totals row ending the first)* | `crop` to one at a time (per session; see seam 1 below) | occasional |
| Side-by-side tables | Two tables horizontally adjacent, separated by a blank spacer column, headers often repeated *(seen)* | `crop` to one at a time — **crop before `drop blanks`** so the spacer isn't dropped (seam 2) | occasional |
| Summary + detail | A small summary block above or beside the main table | `crop` to the detail | occasional |

## Group 5 — Cell-content pathologies (value-level)

| Pathology | What it looks like | Approach | Severity |
|---|---|---|---|
| Format chars in numbers | `$`, thousands `,`, `%`, paren-negatives `(45)` | `s///` + `cast` (the headline use case) | common |
| Excel-mangled numbers | scientific notation for long IDs (`1.2E+11`), serial dates (`44197`), **stripped leading zeros** | **upstream** — likely lost before CSV; re-export as text | common |
| Dates, many formats | 7+ formats in one column; US vs EU ambiguity; serials leaking through | `s///` capture-reorder to ISO; ambiguity needs a human call | common |
| Smart punctuation | curly quotes, em/en dashes, non-breaking spaces from Excel/Windows autocorrect *(seen)* | `s///` to normalize | common |
| Encoding mismatch | UTF-8 vs Windows-1252, mojibake, BOM | BOM via `tolerant read`; re-encode is **upstream** | occasional |
| Whitespace | trailing spaces, non-breaking space `U+00A0`, tabs inside cells | `s///` trim/normalize | common |
| Embedded newlines | a legit multi-line cell (RFC-4180) that breaks naive tools | `tolerant read` (the `csv` crate) | common |
| Multi-value cell | one cell holding several values newline- or comma-stacked (`100-A\n100-B\n100-C`) — *should* have been N rows *(seen)* | `s///` to normalize in place; splitting into rows is reshaping → **out / upstream** | occasional |
| Float-precision noise | `449.29999999999995`, `0.0736669…` from Excel/float export | `= round(num([c]),2)` or `s///` | occasional |
| Error values | `#N/A`, `#REF!`, `#DIV/0!`, `#VALUE!` leaked from formulas | `s///` to blank them; the value itself is **upstream**-lost | occasional |
| Blank sentinels | `N/A`, `-`, `TBD`, `null`, `(blank)`, `.` standing in for empty | `s///` to canonical empty | common |
| Free-text noise | a notes cell with commas, quotes, dates, currency inline *(seen)* | left alone, or `s///` extract; usually not a data column | common |
| Inconsistent categoricals | `Active` / `active` / `IN USE` for one concept | `s///` to a canonical set | common |

## Group 6 — Structural / format-level

| Pathology | What it looks like | Approach | Severity |
|---|---|---|---|
| Inconsistent delimiter | semicolon (EU locale) or tab instead of comma | delimiter flag on read | occasional |
| Inconsistent quoting | some fields quoted only when needed, others bare | `tolerant read` | common |
| Ragged rows | varying field counts per row *(seen)* | `tolerant read` → empty cells (decided) | common |
| Trailing delimiter | every row ends with a stray `,` → a phantom empty column *(seen)* | `drop blanks` (cols) | common |
| Mixed line endings | CRLF / LF / CR mixed in one file | `tolerant read` | occasional |
| Unescaped quotes | a bare `"` inside an unquoted field | `tolerant read` best-effort; some cases **upstream** | rare |

---

## What the taxonomy demands of the grammar

The whole catalog reduces to a short, deterministic primitive set — no new magic:

1. **`crop` / use-range** carries every Group 1 and Group 4 case and the section-label rows in Group 3. This is the headline intake primitive and it is pure addressing.
2. **`header N`** carries the Group 2 "header isn't row 1" cases and extends the overlay model with no new concept.
3. **`drop blanks`** (rows and cols) carries the padding and trailing-delimiter cases.
4. **`fill down`** carries the merged-cell grouping artifact — the one primitive that edges toward reshaping, included because it is bounded, deterministic, and the most common Excel export shape.
5. **`describe`** turns the scary cases (where's the table? multi-table? subtotal rows?) into *advice* the human confirms, preserving the trust the design rests on.
6. **`s///` + cast** — already the core — does all of Group 5's value cleaning once the table is carved.
7. **tolerant read** absorbs Group 6 at the parser.

Everything else — irreversible Excel damage, multi-row-header collapse, unpivoting, subtotal recompute, merging stacked tables — is **upstream** or **out**, and saying so plainly is as much a part of the design as the primitives. One boundary the corpus added: **sheet selection is upstream too.** Real workbooks are multi-sheet and the *active* sheet is often a scratch/pilot tab, not the table you want; picking the sheet happens at `xlsx2csv --sheet`, before a CSV exists. xled operates on one CSV; "which sheet" is never its question.

## Validated against the corpus (slice 5, 2026-06-22)

Converted a 16-file sample of a real client `.xlsx` corpus (findings out-of-repo in `~/xled-corpus/CORPUS-FINDINGS.md`; nothing real entered this repo). The structural taxonomy held up point-for-point — every Group 1–4/6 prediction appeared in real human-authored layouts, and several "rare"/untagged cases were confirmed and promoted (above). Synthetic distillations live in `fixtures/messy/`; the Part C battery is rendered in `proving-ground.md`.

Conversion caveat: the sample was read with openpyxl (`data_only=True`), which returns **typed** values and **pads rows to max width**. So it confirms *structure* but cannot reproduce the export-time value damage (serial dates, sci-notation, stripped zeros) or true raggedness — those stay synthetic, which is fine: their shapes were already known and their messages already written (`errors.md` "not recoverable", semantics rule 5/8).

Three design seams the corpus forced (rendered in `proving-ground.md` Part C):

1. **`crop` is destructive-for-session; multi-table files have N tables.** "Crop to one at a time" resolves as **one working table per open** — extracting all N is repeat-open (or an upstream split). xled carves *a* rectangle; it is not a file splitter.
2. **`drop blanks` (cols) vs an interior spacer column.** Dropping all-blank columns would fuse two side-by-side tables. Rule: **`drop blanks` is for edge padding; isolate one table with `crop` first**, so the spacer falls outside scope. Order is crop-then-drop.
3. **`header N` addresses the post-crop buffer.** Sequential semantics (semantics rule 9): `A6:I10 crop` then `1 header` promotes row 1 *of the cropped region*. Consistent, no new rule.
