# xled fixtures

Synthetic CSV/DSV files for hammering the address grammar and transform syntax until the right form becomes obvious. David brings real client CSVs alongside these; together they span the scenario space.

Guiding principle (from the design): xled has to leverage sed/awk/Excel muscle memory so it reads as the obviously-right tool on first contact. So each fixture below is paired with how the task is solved *today*. We design xled syntax to sit right next to that established pattern — close enough that fluency transfers, departing only where the departure reads instantly (column letters, header names, A1 ranges).

## The fixtures

| File | Shape | What it stresses | How it's done today |
|---|---|---|---|
| `products.csv` | clean, 4 cols | baseline addressing — column by name (`price`) and letter (`C`), row ranges (`2:4`), A1 rectangles (`B2:C3`) | awk `$3`, Excel `C2:C9` |
| `app-portfolio.csv` | messy, domain (APM) | the realistic multi-task file: `$`/comma currency, status casing (`Active`/`active`/`IN USE`), mixed dates, trailing spaces, empty cell | a pile of `sed` substitutes + manual Excel find/replace |
| `contacts.csv` | normalization | email lowercasing, name title-casing, phone reformatting, unicode (`Müller`), apostrophe (`O'Brien`) | `sed -E` per pattern; `=LOWER()`/`=PROPER()` |
| `messy-money.csv` | numbers | strip `$`/`,`, paren-negatives `($45.20)` → `-45.20`, mixed negative styles, then arithmetic | awk `gsub` + a numeric cast |
| `ids-zips.csv` | leading zeros | the lossless-string proof: `02134`, `00042`, `0001-2345` must survive untouched; regex-reformat structured IDs | the file Excel *destroys* on open; awk with everything-as-string |
| `mixed-dates.csv` | one column, 7 date formats | regex capture-and-reorder to ISO; US vs EU ambiguity (`15/01` vs `01/15`) | `sed -E` capture groups, by hand |
| `quoted-hell.csv` | RFC-4180 edges | embedded commas, escaped `""quotes""`, an embedded newline (record 2); addressing must hold over quoted fields | this is where naive `cut`/`awk -F,` break; the `csv` crate earns its place |
| `headerless.csv` | no header row | forces letter-only addressing (`A`,`B`,`C`); how does xled know there's no header? | awk `$1`; design Q for xled |
| `inventory.tsv` | tab-separated | DSV ≠ only CSV — delimiter handling; low-qty rows for scope-an-edit selection | `awk -F'\t'`; Excel import wizard |
| `daily-sales.csv` | 32 cols (A..AF) | multi-letter column addressing past Z; wide ranges like `C:AF`; operate across a span of day-columns | Excel `C2:AF5`; awk gets ugly fast |
| `tricky-headers.csv` | grammar collisions | column **named** `B` (at position C) vs the **letter** `B`; a column named `2024`; `price (USD)` (spaces+parens); `notes.txt` (dot); `first name` (space) | the cases that break every quick parser; the disambiguation test |
| `ragged.csv` | broken rows | missing trailing fields, extra fields, a one-field row, trailing empty | robustness — what does an address *mean* on a short row? |

## Messy intake fixtures (`messy/`)

A second set in `messy/`, distilled from a real 16-file `.xlsx` corpus (validation in `../intake-taxonomy.md`; client specifics out-of-repo). These stress the **structural intake** primitives — `crop`, `header N`, `drop blanks`, `fill down`, `describe` — rendered as Part C of `../proving-ground.md`. Every file is synthetic; the real data never entered this repo.

| File | Shape | What it stresses |
|---|---|---|
| `preamble.csv` | title + meta rows, blank, header at row 5, trailing blanks | `crop` to the rectangle, `header N`, `drop blanks` (rows) |
| `stacked.csv` | two tables stacked, a totals row ending the first, blank-separated | `crop` one-at-a-time (seam 1), `/^Total/i del`, `describe` |
| `side-by-side.csv` | two tables horizontal, blank spacer column, duplicate headers | crop-before-`drop blanks` (seam 2) |
| `fill-down.csv` | grouping column value once then blanks (merged-cell artifact) | `fill down` |
| `spacer-column.csv` | blank col A holding an index (blank header), trailing empty cols | `drop blanks` keys on empty *column* not empty *header* |
| `outline.csv` | dotted-ID hierarchy, trailing-space status | select-by-level regex address, `s///` trim |
| `multivalue.csv` | a cell with newline-stacked values + currency | the split→out boundary; normalize in place |

## Operations battery

Moved to `../proving-ground.md`, which is the single battery now: Part A drives the adversarial cases against the grammar's seams (the disambiguations, ragged rows, embedded newlines, header collisions these fixtures are rigged to force), and Part B is the comprehensive in-scope list of every regex-over-tabular operation. Each item there cites the fixture it runs against and doubles as a unit-test spec.
