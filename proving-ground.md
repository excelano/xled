# Proving ground

The grammar's test of fitness. Two parts: an **adversarial battery** that drives at the known blind spots and seams, and a **comprehensive in-scope battery** — every operation a regex (plus the thin compute layer) can perform across tabular data.

## How to use this

For each item: write the `address command` form, then judge it on three axes.

1. **Reads natural?** Does the form write itself for someone with sed/awk/Excel muscle memory, or does it fight? A form that needs explaining is a failed form.
2. **Forces a decision?** Items tagged `⚑decision` can't be expressed until an open grammar question is settled. Settle it here, against real shapes, not in the abstract.
3. **Becomes a test?** Every item is `fixture + command → expected output`. The "today" column (the sed/awk/Excel equivalent) is the oracle: xled's output should match what that pipeline produces. When an item has a concrete `before → after`, that pair *is* the unit test.

Fixtures are in `fixtures/`. Check an item when its syntax is decided *and* it reads natural; the `⚑decision` tags map to the open questions in `DESIGN.md`. Scale-only cases (very wide / very deep) are validated against the real corpus, which never enters this repo.

Legend: `⚑decision` = blocked on an open grammar call · `⚐boundary` = tests whether the in/out-of-scope line holds · `≈today` = the muscle-memory oracle.

---

# Part A — Adversarial battery

Each case targets a specific seam. "Pass" is stated per item: either the form reads cleanly, or we consciously rule the case out.

## A1. Named column meets a row range  `⚑decision`
Names cannot fuse into A1 (`B2:C3` has no `[status]` equivalent), so names force the split form. This is the core composition problem.
- [ ] Uppercase `[status]`, but only rows 2–4. (`app-portfolio`) — what separates the row part from the column part?
- [ ] Substitute in `[price (USD)]` for rows matching `/r2/` only. (`tricky-headers`)
- [ ] Same op, column addressed by letter instead (`D`, rows 2–4) — does the letter form get to use A1 (`D2:D4`) while the name form can't, and is that split acceptable?
- **Pass:** one composition rule expresses both `letter × rows` and `[name] × rows` without a special case that only letters enjoy.

## A2. Whole-row regex semantics  `⚑decision`
The ed-inherited "match the whole line" reading leaks CSV artifacts in 2D.
- [ ] Select rows where *any* cell contains `tools`. (`headerless`) — is `/tools/` an any-cell match?
- [ ] Select rows where the raw line contains a comma. (`quoted-hell`) — if raw-line matching exists, this matches quoted-field commas; is that ever wanted?
- [ ] Select rows where specifically `[note]` contains a quote `"`. (`quoted-hell`) — column-scoped match form.
- **Pass / recommendation:** `/re/` = "any cell matches"; `[col]~/re/` = column-scoped; raw-line matching is **cut**. Confirm nothing real needs raw-line.

## A3. Single comparison as scope  `⚑decision` `⚐boundary`
Filter-as-scope is in; filter-as-query is out. One comparison is the knife-edge.
- [ ] Scope an edit to rows where `[qty] < [reorder]`. (`inventory`) — is a lone comparison a legal address?
- [ ] Scope to rows where `num([amount]) < 0`. (`messy-money`)
- [ ] Attempt `[qty] < [reorder] and [status]~/active/` — this **must not** parse. (boundary holds only if combinators are refused)
- **Pass:** exactly one comparison allowed as an address; the first `and`/`or`/`not` is a hard error pointing at xql.

## A4. Header is addressable, not just protected  `⚑decision`
"Never mistaken for data" ≠ "never reachable."
- [ ] Rename the header `notes.txt` to `notes`. (`tricky-headers`)
- [ ] Lowercase every header. (`app-portfolio`) — does a header op scope exist?
- [ ] Confirm a normal column op (`[price] s/.../.../ `) leaves the header cell untouched.
- **Pass:** a deliberate header address exists; default scope excludes it.

## A5. Open-ended and last-row addressing  `⚑decision`
"To the end" is the most common shape and A1 can't say it.
- [ ] Operate on `[amount]` from row 2 to the end. (`messy-money`) — `2:` ?
- [ ] Operate on the last row only. (`products`) — `$` ?
- [ ] Operate on rows 1 through 3 with `:3`. (`products`)
- [ ] Delete the final (trailing-empty) row. (`ragged`)
- **Pass:** `2:`, `:N`, and a last-row token all parse; `$` doesn't collide with regex end-anchor in context.

## A6. Command reach — create, multi-select, delete disambiguation  `⚑decision`
- [ ] Add a new column by assigning to the next letter/name (`E = num([price]) * 1.1` on a 4-col file). (`products`) — does assigning past the width *create*?
- [ ] Apply one scrub across non-contiguous columns `[price],[sku]`. (`products`)
- [ ] Delete row 3 vs delete column `[dept]` vs delete column `C` — do `3 d`, `[dept] d`, `C d` disambiguate by address type alone? (`ragged`)
- [ ] Append a row of values. (`products`)
- **Pass:** create/delete/append all reachable; row-vs-column intent is unambiguous from the address.

## A7. Bracket edges and the three case rules  `⚑decision`
- [ ] Address a header that itself contains `]`, e.g. `notes [draft]`. — escape rule inside brackets?
- [ ] Address column letter `c` lowercase; confirm it equals `C`. (letters case-insensitive)
- [ ] Confirm `[userId]` does **not** match a header `userid`. (names case-sensitive)
- [ ] Confirm `/active/i` matches `ACTIVE`. (regex case by flag)
- **Pass:** bracket-escape defined; the three different case rules are each documented and each correct for its domain.

## A8. Ragged rows — what a column address means on a short row  `⚑decision`
- [ ] Read `[dept]` (3rd col) on a 2-field row. (`ragged`) — missing → empty string? error?
- [ ] Assign `[dept] = "Unknown"` on that short row — does it pad the row out?
- [ ] Address `D` on a row that has only 1 field. (`ragged`)
- **Pass:** a defined missing-cell value (recommend empty string, consistent with stringly-typed) and a defined pad-on-write rule.

## A9. Address negation
- [ ] Select rows **not** matching `/active/i`. (`app-portfolio`) — `!/re/` (sed) or other?
- [ ] Substitute in every row except the first. (`products`)
- **Pass:** negation reads as scope, not as a query predicate (it's still selecting rows to edit).

## A10. Scoped assignment — target is also the scope
- [ ] On rows matching `/active/i`, set `[status] = "approved"`. (`app-portfolio`) — `[status]` is both the row-filter subject and the write target; does the form stay unambiguous?
- **Pass:** LHS-as-target vs LHS-in-scope never collide.

## A11. Logical value vs raw bytes inside quoted fields
- [ ] Substitute in `[address]` where the value contains a comma. (`quoted-hell`) — the op sees the *parsed* value `123 Main St, Apt 4B`, never the surrounding quotes.
- [ ] Operate on the cell whose value spans an embedded newline; confirm it stays one cell. (`quoted-hell` record 2)
- **Pass:** all matching/substitution is on logical cell values; quoting is the serializer's concern only.

## A12. A column named like a coordinate
- [ ] Address columns named `B`, `2024`, and (hypothetically) `C:AF` or `B2:C3`. (`tricky-headers`) — brackets should neutralize all of them.
- **Pass:** `[B2:C3]` reaches the column named that; bare `B2:C3` is the rectangle. No ambiguity.

## A13. Default (omitted) scope
- [ ] Bare `s/old/new/g` with no address. — every cell in the sheet?
- [ ] Bare `[price]` with no command. — report/print the column (bare-command-reports-state)?
- **Pass:** omitted column = all columns; omitted row = all rows; bare address = inspect.

---

# Part B — In-scope battery

Everything regex (and the thin compute layer) does across a grid. Grouped by operation family. Each is `goal (fixture) ≈today`. These are the happy-path forms whose syntax must be obvious, and the bulk of the unit suite.

## B1. Inspect / select (proves the addressing matrix)
- [ ] Print column `[price]`; print column `C`; confirm identical. (`products`) ≈ `awk -F, '{print $3}'`
- [ ] Print rows 2–4, all columns. (`products`) ≈ `sed -n '2,4p'`
- [ ] Print the rectangle `B2:C3`. (`products`)
- [ ] Print rows matching `/tools/`. (`headerless`) ≈ `grep tools`
- [ ] Print `day_05`..`day_10` across all rows. (`daily-sales`) — multi-letter span
- [ ] Print a single cell `B2`. (`products`)
- [ ] Print the whole column span `C:AF`. (`daily-sales`)
- [ ] Print with the matching count / row numbers shown (inspection aid; not a saved aggregate). `⚐boundary`

## B2. Substitute — the headline `s/re/rep/flags`
- [ ] Replace first match in a cell. (`products`) ≈ `sed 's/a/b/'`
- [ ] Replace all in a cell, `g`. (`messy-money`) ≈ `sed 's/a/b/g'`
- [ ] Case-insensitive match, `i`. (`app-portfolio`) ≈ `sed 's/active/x/I'`
- [ ] Replace the Nth occurrence. (`quoted-hell`) ≈ `sed 's/a/b/2'`
- [ ] Delete a match (empty replacement): strip `$`. (`messy-money`) `"$45.20" → "45.20"`
- [ ] Strip thousands separators: `,`. (`messy-money`) `"1,250,000" → "1250000"`
- [ ] Strip a set in one pass `[$,]`. (`app-portfolio`) `"$1,250,000" → "1250000"`
- [ ] Literal (non-regex) replace. (`contacts`)
- [ ] Collapse internal whitespace runs `\s+ → " "`. (`app-portfolio`)
- [ ] Fill blank cells: `^$ → "N/A"`. (`app-portfolio` empty owner)
- [ ] Substitute scoped to a column. (`messy-money`)
- [ ] Substitute scoped to a rectangle `B2:C3`. (`products`)
- [ ] Substitute scoped to regex-selected rows. (`app-portfolio`)
- [ ] Substitute across the whole sheet (no address). (`products`)

## B3. Capture & rearrange (backreferences)
- [ ] Reorder date `M/D/Y → Y-M-D` via captures. (`mixed-dates`) ≈ `sed -E 's#(..)/(..)/(....)#\3-\1-\2#'`
- [ ] Swap `"Last, First" → "First Last"`. (`quoted-hell` customer) ≈ `sed -E 's/(.*), (.*)/\2 \1/'`
- [ ] Reformat phone digits → `+1-555-XXX-XXXX`. (`contacts`)
- [ ] Restructure ID `0001-2345 → 00012345`, zeros intact. (`ids-zips`)
- [ ] Wrap a match (insert around): `(\d+) → [\1]`. (`products`)
- [ ] Extract the email domain into place: `.*@(.*) → \1`. (`contacts`)
- [ ] Pull the first run of digits out of a messy string. (`app-portfolio`)

## B4. Case transformation
- [ ] Lowercase a column (email). (`contacts`) ≈ `=LOWER()`
- [ ] Uppercase a column (status code). (`inventory`)
- [ ] Title-case a column (name). (`contacts`) ≈ `=PROPER()` — needs per-word capitalization
- [ ] Capitalize first letter only. (`products`)
- [ ] Normalize a categorical to one canonical form: `Active/active/IN USE → active`. (`app-portfolio`)

## B5. Whitespace & cleaning
- [ ] Trim leading/trailing whitespace. (`app-portfolio` trailing spaces) ≈ `sed -E 's/^ +| +$//g'`
- [ ] Remove all spaces. (`ids-zips`)
- [ ] Strip a trailing punctuation char. (`contacts`)
- [ ] Remove surrounding currency/format chars then leave a number-shaped string. (`messy-money`)
- [ ] Normalize an apostrophe/quote variant. (`contacts` O'Brien)

## B6. Anchors & boundaries
- [ ] Match whole-cell exactly `^...$`. (`products`)
- [ ] Match a prefix `^TL-`. (`headerless` sku)
- [ ] Match a suffix `\.txt$`. (`tricky-headers`)
- [ ] Word-boundary match `\bInc\b`. (`contacts`)
- [ ] Confirm `^`/`$` anchor to cell bounds, not file/line. (`quoted-hell`)

## B7. Extraction / projection
- [ ] Output only a captured group from each cell (extract mode). (`contacts`) ≈ `sed -nE 's/.*(\d{4}).*/\1/p'`
- [ ] Output only rows matching, only a chosen column. (`app-portfolio`)
- [ ] Keep only the digits of a cell. (`contacts` phone)

## B8. Split & join cells/columns
- [ ] Split a cell on a delimiter into adjacent columns. (`quoted-hell` customer → last, first) `⚑decision` (does split create columns?)
- [ ] Join two columns into one with a separator. (`contacts` → "First Last")
- [ ] Split on a regex, not just a literal. (`app-portfolio`)

## B9. Compute & derive (`col = expr`, awk side)
- [ ] Arithmetic across columns: `[total] = num([qty]) * num([price])`. (`inventory`)
- [ ] Percentage change: `[annual_cost] = num([annual_cost]) * 1.03`. (`app-portfolio`)
- [ ] Concatenate: `[full] = [first] & " " & [last]` (or chosen concat op). (`contacts`)
- [ ] Substring / left-N. (`ids-zips`)
- [ ] Length of a cell into a new column. (`products`)
- [ ] Round / format a number to 2 decimals. (`messy-money`)
- [ ] Sign conversion `($45.20) → -45.20`. (`messy-money`) — regex + cast
- [ ] Cast-and-compare producing a boolean column. (`inventory`)

## B10. Conditional & blank handling
- [ ] Default a blank: `[owner]` empty → `"Unassigned"`. (`app-portfolio`) — via regex `^$` and/or a `default()` function
- [ ] Coalesce two columns (first non-empty). (`app-portfolio`) `⚑decision` (function set)
- [ ] Single-condition value selection `if(cond, a, b)` — **contested**: tests the no-control-flow line. `⚐boundary`

## B11. Structural edits (the buffer/ed layer)
- [ ] Delete a row by number. (`products`) ≈ `sed '3d'`
- [ ] Delete all rows matching `/retired/i`. (`app-portfolio`)
- [ ] Delete a column by name and by letter. (`products`)
- [ ] Insert/append a new column. (`products`)
- [ ] Append a row. (`products`)
- [ ] Rename a header. (`tricky-headers`)
- [ ] Duplicate a column. (`products`)
- [ ] Reorder/move columns — **contested** (reshaping vs scope). `⚐boundary`
- [ ] Sort rows — expected **out** (→ `sort`/DuckDB); confirm it stays out. `⚐boundary`

## B12. Multi-step / REPL behavior
- [ ] Chain: scrub `[annual_cost]` to a number, then derive `[total]` from it in the next command. (`app-portfolio`)
- [ ] Preview an operation's effect before committing it. (any) — the previewability requirement
- [ ] Undo the last operation — **decision**: does the buffer support undo? `⚑decision`
- [ ] Save deliberately; confirm no write happened before save. (any)
- [ ] Run a sequence as a script (non-interactive) and get the same result as the REPL. (any)

## B13. IO / pipe boundary
- [ ] One-shot invocation `xled 'cmd' file.csv`. (`products`)
- [ ] Read from stdin, write to stdout, file untouched. (`products`)
- [ ] Round-trip preserves quoting, embedded newlines, and leading zeros byte-for-byte when unchanged. (`quoted-hell`, `ids-zips`)
- [ ] Operate on a TSV with the same grammar. (`inventory.tsv`)
- [ ] Drop cleanly into a pipe `… | xled '…' | …`. (any) ≈ the handoff contract
- [ ] Handle ragged input without crashing. (`ragged`)
- [ ] Operate correctly on a very wide file (multi-letter columns past Z). (real corpus, ~99 cols)
- [ ] Stay responsive on a very deep file (buffer over ~600k rows). (real corpus)

---

## Scoring the proof

The grammar is proven when: every Part B item has a form that reads natural to the muscle-memory test; every `⚑decision` is settled by a form that generalizes (no special-casing); every `⚐boundary` item lands on the correct side (in-scope ones natural, out-of-scope ones awkward or refused); and the awkward forms are *exactly* the out-of-scope set. Any in-scope item without a natural form is a hole in the grammar, not a missing feature.

---

# Slice-3 render (2026-06-21)

The full battery rendered against the locked grammar (`composition-grammar.md` + `semantics.md` + `expr-grammar.md`). Verdict: **the address/composition grammar survived intact** — every Part A seam resolved with no special case. All breakage clustered in two layers that were hand-waved as "thin" and are not: the `s///` replacement dialect and the `= expr` compute layer. Both are now specified (sed-faithful replacement interpreter; `expr-grammar.md`). This section is the answer key and doubles as the cheat-sheet.

## Part A — how each seam landed

| Case | Rendered form | Verdict |
|---|---|---|
| A1 name × rows | `[status] 2:4 = upper([status])` · `D2:D4 s/x/y/` ≡ `D 2:4 s/x/y/` | **clean** — space-intersection expresses `[name]×rows` and `letter×rows`; A1-fusion is a letter-only typing convenience, both reduce to the same intersection |
| A2 whole-row regex | `/tools/ show` · `[note]~/"/ show` · raw-line match = **no form** | **clean** — `/re/` any-cell, `[col]~/re/` scoped; raw-line correctly has no form (field commas are data, the cut is right) |
| A3 comparison scope | `num([qty])<num([reorder]) [status] = "low"` · `num([amount])<0 …` · `… and …` → **error → xql** | **clean, with the cast** — operands are exprs; **string-wise unless `num()`-cast** (the footgun: bare `[qty]<[reorder]` is lexicographic). A3 examples corrected to cast form |
| A4 header addressable | `[notes.txt] rename notes` · per-column rename · `[price] s/…/` leaves header untouched | **clean for the named case**; bulk "lowercase all headers" has no scope — **deferred** (rename is one-at-a-time; a header pseudo-address is a later call) |
| A5 open / last row | `[amount] 2: s/…/` · `$ s/…/` · `:3 s/…/` · `$ del` | **clean** — `$`=last-row vs regex end-anchor disambiguates by position |
| A6 create / multi / delete | `E = num([price])*1.1` · `[price],[sku] s/…/` · `3 del` / `[dept] del` / `C del` | **clean** for create/multi/delete; **append-row = deferred** (the parked structural gap) |
| A7 bracket & case rules | `[notes [draft]]]` (escape `]]`) · `c`≡`C` · `[userId]`≠`userid` · `/active/i`=`ACTIVE` | **clean** — bracket-escape `]]` locked; three case rules each correct for its domain |
| A8 ragged rows | `[dept] show`→`""` · `[dept] = "Unknown"` pads intervening cells `""` · `D` on 1-field row→`""` | **clean** — missing = `""` (rule 5); pad-on-write rule added |
| A9 negation | `!/active/i s/…/` · `!1 s/…/` (or `2: s/…/`) | **clean** — negation reads as scope |
| A10 scoped assignment | `/active/i [status] = "approved"` | **clean** — filter-subject and write-target never collide (rule 7) |
| A11 logical value vs bytes | `[address]~/,/ [address] s/,/;/` · embedded newline stays one cell | **clean** — all ops on parsed cell values; quoting is the serializer's concern |
| A12 coordinate-named column | `[B]` · `[2024]` · `[B2:C3]` (the column) vs bare `B2:C3` (the rectangle) | **clean** — bracket is the universal quoter |
| A13 default scope | `s/old/new/g` (all cells) · `[price]` (inspect) | **clean** — omitted axis = all; bare address = inspect |

## Part B — the cheat-sheet (goal → xled)

**B1 inspect/select** — `[price]` ≡ `C` · `2:4` · `B2:C3` · `/tools/` · `[day_05]:[day_10]` · `B2` · `C:AF`. All clean.

**B2 substitute** — `[c] s/a/b/` (first) · `s/a/b/g` (all-in-cell) · `s/a/b/i` · `s/a/b/2` (Nth†) · strip `s/\$//g` · `s/[$,]//g` · fill blank `s#^$#N/A#` · scoped `[c] s/…/`, `B2:C3 s/…/`, `/re/ s/…/`, bare `s/…/`. Clean. †`N`-flag = "the Nth match" needs custom find-all (crate `replacen` is "first N") — implementation note, not a grammar hole.

**B3 capture & rearrange** — `[d] s#(..)/(..)/(....)#\3-\1-\2#` · `s/(.*), (.*)/\2 \1/` · phone/ID/wrap/domain/digit-run. **Clean once the replacement dialect is sed-style `\1`/`&` — locked** (own interpreter over the `regex` crate's captures; the crate's `$1` is *not* exposed).

**B4 case** — `[email] s/.*/\L&/` · upper `\U&` · title `s/\b(.)/\U\1/g` · capitalize `s/^(.)/\U\1/` · categorical `s/(?i)^(active|in use)$/active/`. **Clean once `\U\L\u\l\E` exists in the replacement dialect — locked.** Case stays in `s///`, not expr.

**B5 whitespace** — trim `s/^ +| +$//g` · `s/ //g` · trailing-punct · normalize apostrophe `s/’/'/g`. Clean (s/// territory).

**B6 anchors** — `^...$` · `^TL-` · `\.txt$` · `\bInc\b` · cell-bounded (rule 4). Clean.

**B7 extraction** — in-place `s/.*(\d{4}).*/\1/` · only-matching-rows-one-column via `/re/ [col] show` · digits `s/\D//g`. Clean.

**B8 split & join** — join `[full] = [first] & " " & [last]` (clean, `&` concat). **Split one cell → N columns: no clean form — boundary/deferred** (assignment makes one column; true split edges into reshaping; common case = `s///` in place).

**B9 compute** — `[total] = num([qty])*num([price])` · `[c] = num([c])*1.03` · concat `&` · `left/mid` · `len` · `round(x,2)` · sign `s/\(([\d.]+)\)/-\1/` then num · bool `[low] = num([qty])<num([reorder])`. **Clean against `expr-grammar.md`** — this family *is* what forced that spec.

**B10 conditional/blank** — `[owner] = default([owner],"Unassigned")` (or `s#^$#Unassigned#`) · `coalesce([a],[b])` · `if(cond,a,b)`. **Clean** — `if()` locked as a pure expression (the no-control-flow line).

**B11 structural** — `3 del` · `/retired/i del` · `[dept] del`/`C del` · create via `=` · `[old] rename new` · dup `[copy] = [orig]`. **Append-row deferred**; column-reorder **contested → lean out** (reshaping); sort **out** (`[c] sort` → error → DuckDB). 

**B12 multi-step/REPL** — chain = sequential commands (rule 9); preview/save = design; **undo `⚑` → lean yes** (live editor, ed had `u`), settled in implementation.

**B13 IO/pipe** — `xled 'cmd' file.csv` · stdin→stdout · byte-for-byte round-trip (csv crate + stringly) · TSV same grammar · pipe · ragged · wide · deep. All design contracts, grammar-clean.

## What slice 3 changed

1. **`s///` gets a sed-faithful replacement interpreter** — `\1`–`\9`, `&`, `\U \L \u \l \E` — written over the standard `regex` crate's captures (no crate does this; `sedregex` is a `$1` wrapper, unmaintained; `fancy-regex` is for *pattern* backrefs we don't need and costs linear-time). Solves B3 + B4 together.
2. **`expr-grammar.md`** — the compute layer is specified: value model, operators/precedence, and the function library (`num bool len left right mid round default coalesce if`), all derived from B9/B10/B8.
3. **Comparisons are string-wise unless `num()`-cast** — A3 corrected; the footgun is documented, not hidden.
4. **`if(cond,a,b)`** over `?:` (the `:`-collision tiebreaker); **bracket-escape `]]`**; **pad-on-write `""`** for ragged rows.

Still parked: **append-row** (design from battery evidence), **bulk header transform**, **column-reorder** (lean out), **split-to-columns** (boundary), **undo** (lean yes, implementation phase).

---

# Slice-5 render (2026-06-22) — Part C, structural intake

The intake primitives (`crop`, `header N`, `drop blanks`, `fill down`, `describe`) rendered against `fixtures/messy/`, the synthetic distillation of a real 16-file `.xlsx` corpus (validation in `intake-taxonomy.md`; client specifics out-of-repo in `~/xled-corpus/`). Verdict: **the primitives carry the whole structural taxonomy, and they are all addressing** — no detection magic, no reshaping. Three composition seams surfaced (crop/drop-blanks ordering, multi-table per session, header-after-crop); all three resolve inside the existing grammar with no new rule. The cell-cleaning that *follows* intake is Part A/B's job and is not re-rendered here.

## Part C — how each intake seam landed

| Case (fixture) | Rendered form | Verdict |
|---|---|---|
| C1 carve the real table out of preamble (`preamble.csv`) | `A5:E8 crop` then `1 header` | **clean** — crop to the rectangle, then promote row 1 *of the cropped buffer* (seam 3) |
| C2 header isn't row 1 (`preamble.csv`) | `5 header` (pre-crop) ≡ `1 header` after `A5:E8 crop` | **clean** — `header N` is pure addressing; the overlay model already allowed a non-row-1 header |
| C3 trailing blank rows (`preamble.csv`) | `drop blanks` → rows 9–10 gone | **clean** — fully-empty rows only; a partly-filled row is never silently dropped |
| C4 leading spacer + trailing empty cols (`spacer-column.csv`) | `drop blanks` drops empty cols D–F; col A **kept** (blank header, holds the index) → reach by `A` | **clean, and it sharpens the rule**: `drop blanks` keys on an empty *column*, not an empty *header*. Blank-header-with-data is a named-by-letter column, not junk |
| C5 forward-fill a merged grouping col (`fill-down.csv`) | `[Vendor] fill` | **clean** — down-only, deterministic; the one primitive that edges toward reshaping, kept because it's bounded |
| C6 totals row interleaved (`stacked.csv`) | `/^Total/i del` | **clean** — delete-by-match; recomputing the total is **out** |
| C7 stacked tables (`stacked.csv`) | table 1: `A1:D4 crop` then `/^Total/i del`; table 2: **reopen**, `A7:D9 crop` | **clean per table — seam 1**: `crop` is one working table per open. Pulling all N is repeat-open (or an upstream split). xled carves *a* rectangle, not a splitter |
| C8 side-by-side tables, blank spacer col (`side-by-side.csv`) | left: `A1:C4 crop`; right: `E1:G4 crop` | **clean — seam 2**: must `crop` *before* `drop blanks`. Naive `drop blanks` (cols) would delete the spacer and fuse the two tables; crop-first puts the spacer outside scope |
| C9 dotted-ID outline (`outline.csv`) | top level `[#]~/^\d+$/ show` · sub-rows `[#]~/\./ show` · trim status `[Status] s/ +$//` | **clean for select/trim**; deriving an integer *depth* is awkward (no `count`/dot-count fn) — but structural unnest is **out** anyway, so depth-as-a-number isn't a target. Selecting by level is the real need and it's just a regex address |
| C10 multi-value newline cell (`multivalue.csv`) | normalize in place `[Amount] s/[$,]//g`; the newline-stacked `[Contract IDs]` **stays one cell** | **clean for in-place; splitting → out** — exploding one cell into N rows is reshaping (the A11 logical-cell rule already says the embedded newline is one value) |
| C11 advisory region report (all messy fixtures) | `describe` → e.g. *"rows 1–3 preamble · row 4 blank · header row 5 · data 6–8 · blank tail 9–10"* | **clean as advice** — `describe` reports the best-guess regions and the totals/section-banner rows it suspects, and **never acts**; the human turns it into a `crop`/`header`/`del`. Output format locked as a region summary, not a patch |

## What slice 5 settled

1. **The structural taxonomy is validated.** A 16-file real-corpus pass confirmed every Group 1–4/6 prediction in human-authored layouts, including cases promoted from rare/untagged (side-by-side tables, totals rows, stacked tables, forward-fill, leading spacer column). Details and the openpyxl conversion caveat are in `intake-taxonomy.md`; specifics out-of-repo.

2. **`crop` is one working table per open (seam 1).** A multi-table file is handled one rectangle at a time across reopens; xled is not a file splitter. This keeps `crop` on the safe side of the reshaping line.

3. **`drop blanks` is edge-padding only; crop-then-drop (seam 2).** It removes fully-empty leading/trailing rows and columns. Interior all-blank separators (the side-by-side spacer) are *not* its job — `crop` isolates the table first so the separator falls outside scope. And it keys on an empty *column*, not an empty *header* (C4): a blank-header column that holds data is a real column reached by letter.

4. **`header N` addresses the post-crop buffer (seam 3).** Sequential semantics (semantics rule 9) make `crop` then `header` compose with no new rule: the promoted row is row 1 of the cropped region.

5. **New value-pathology rows folded in:** multi-value newline cell (→ normalize in place, split is out) and float-precision noise (→ `round`/`s///`). Sheet selection is named as **upstream** (an `xlsx2csv --sheet` concern, before a CSV exists).

Nothing in Part C demanded new grammar — the intake primitives are addressing plus `del`, and they slot under the same `reference command` shape as everything else. The remaining design step is **slice 6 (EBNF)**, then implementation.
