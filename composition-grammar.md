# Composition grammar — candidate draft

The open problem: how a row component and a column component combine into one address, making both the fused A1 form (`B2:C3`) and the split form (rows `/re/`, column `[status]`) legal under one grammar, with a command appended. Two candidates, rendered against the batteries, then scored for compactness and muscle-memory fidelity.

## Candidate 1 — `xlref`: Excel's reference algebra + sed verbs

The bet: the composition rule already exists, in Excel's own reference operators. Excel has exactly three, and they are the whole grammar:

- **`:` range** — `2:4` (rows), `B:D` (columns), `B2:C3` (rectangle), `2:`/`:4` (open-ended), `$` (last row)
- **`,` union** — `[price],[cost]`, `1,3,5`
- **` ` (space) intersection** — `[status] 2:4` is "column status ∩ rows 2–4"

Excel already splits the axes natively: `C:C` is the whole column, `2:2` the whole row, `B2` a cell. xled adds exactly two atoms to Excel's set — `[name]` for a named column and `/re/` for a row-set — and inherits everything else verbatim. A row-set and a column-set intersect with a space; that *is* the composition rule.

Reference atoms:

| Atom | Means |
|---|---|
| `2`, `2:4`, `2:`, `:4`, `$` | rows by number / range / open / last |
| `C`, `B:D` | columns by letter / letter-range |
| `[name]`, `[a]:[d]` | column by name / named span (header order). A literal `]` in a name is doubled (`]]`): the header `notes [draft]` is `[notes [draft]]]`. |
| `B2`, `B2:C3` | cell / rectangle (fused — letters only) |
| `/re/` | rows where any cell matches |
| `[col]~/re/` | rows where that column matches |
| `[a] < [b]` | rows where the comparison holds (one comparison, no combinators — `⚐boundary`). Operators: `==` `!=` `<` `>` `<=` `>=`. Equality is `==`, never bare `=` — bare `=` is the assignment sigil, so overloading it would make `[price] = [cost]` ambiguous between "rows where equal" and "set price to cost." awk-faithful. Operands are **exprs** (`expr-grammar.md`), so `num([amount]) < 0` is legal; comparison is **string-wise unless `num()`-cast** (`[qty]<[reorder]` is lexicographic — `num([qty])<num([reorder])` for numeric order). |
| `!<rowset>` | negation of a row-set |
| *(omitted axis)* | all rows, or all columns |

### The command-disambiguation rule (the load-bearing decision)

Space is the intersection operator, so it can't also be "the gap before the command" — `2:4 d` would be ambiguous (delete rows 2–4? or rows 2–4 ∩ column D, inspected?). Resolved by making **commands lexically distinct from reference atoms**, two kinds:

- **Sigil-led**, self-disambiguating by the character that follows: `s/re/rep/flags`, `y/set/set/`, `= expr`. A column ref `S` is `s` followed by space/`,`/`:`/end — *never* by `/`. So `s/` is always substitute, `=` is never a column. These are the two primary verbs and they cost zero ambiguity.
- **Reserved words** for the secondary/structural verbs: `del`, `crop`, `header`, `fill`, `show`. The one casualty is ed's single-letter `d`/`p` — in a 2-D tool "delete" has to say row-or-column anyway, and the address already does (`3 del` vs `[dept] del`), so the word reads at least as well. Reserved words are ≥3 letters; a column literally named one is reachable bracketed (`[fill]`).

Rule: the parser reads the maximal leading reference-expression, then one command. A bare reference with no command inspects (report-state). That's the entire top-level shape: **`reference command`**.

## Candidate 2 — `piped`: explicit command separator

The deliberate competitor. Keep Excel-ish axes, but put a `|` between address and command. The `|` makes the boundary explicit, so commands can be *anything*, including ed's single letters `d`/`p`.

```
[status] 2:4 | s/x/y/g
2:4 | d
| s/x/y/g            (empty address = whole sheet)
```

Win: zero disambiguation rule needed; ed's `d`/`p` survive. Cost: a `|` on every mutating line — visual noise, and sed has no such separator, so the muscle memory is weaker.

## Rendering — the batteries in both

| Goal (battery) | C1 `xlref` | C2 `piped` |
|---|---|---|
| Substitute in a column (B2) | `[price] s/\$//g` | `[price] \| s/\$//g` |
| Whole-sheet substitute (A13) | `s/x/y/g` | `\| s/x/y/g` |
| Column ∩ row range (A1) | `[status] 2:4 s/x/y/g` | `[status] 2:4 \| s/x/y/g` |
| Matching rows ∩ column (A10) | `/active/i [status] = "approved"` | `/active/i [status] \| = "approved"` |
| Fused rectangle (B2) | `B2:C3 s/x/y/g` | `B2:C3 \| s/x/y/g` |
| Union of two columns (A6) | `[price],[cost] s/,//g` | `[price],[cost] \| s/,//g` |
| Open-ended rows (A5) | `2: s/x/y/g` | `2: \| s/x/y/g` |
| Last row (A5) | `$ s/x/y/g` | `$ \| s/x/y/g` |
| Negation (A9) | `!/active/i s/x/y/g` | `!/active/i \| s/x/y/g` |
| Comparison scope (A3) | `[qty]<[reorder] [status] = "low"` | `[qty]<[reorder] [status] \| = "low"` |
| Column-scoped regex (A2) | `[note]~/".*"/ show` | `[note]~/".*"/ \| show` |
| Capture-reorder date (B3) | `[raw_date] s#(..)/(..)/(....)#\3-\1-\2#` | `[raw_date] \| s#…#…#` |
| Compute (B9) | `[total] = num([qty])*num([price])` | `[total] \| = num([qty])*num([price])` |
| Delete a row (B11) | `3 del` | `3 \| d` |
| Delete a column (B11) | `[dept] del` | `[dept] \| d` |
| Crop to the real table (intake) | `A4:M203 crop` | `A4:M203 \| crop` |
| Promote row to header (intake) | `4 header` | `4 \| header` |
| Fill down (intake) | `[cat] fill` | `[cat] \| fill` |
| Inspect a column (A13) | `[price]` | `[price]` |

## How close are we to the compact spot

Close. C1 is the stronger candidate by the project's own yardstick: it's more compact (no per-line separator), and it's *more* unoriginal than expected — it doesn't invent a composition operator, it adopts Excel's, including the space-intersection almost nobody knows is there. The A1 blind spot inverts into a feature: names can't fuse into A1, so they use the space form *precisely because* that's what the intersection operator is for. The disambiguation rule is the only real cost, and it's small and learnable.

C2's only advantage is preserving ed's single-letter `d`/`p`, bought with visual noise on every mutating line. That's a bad trade for a tool whose whole pitch is terse, native-feeling editing.

**Recommendation: build on C1.** It is plausibly the compact spot itself, not just a step toward it.

## Resolved (slice 2, 2026-06-21 — direction committed)

1. **Precedence / grouping** — adopt Excel's verbatim: `:` (range) > ` ` (intersection) > `,` (union), with **parentheses for grouping**, exactly as Excel allows. So `[price],[cost] 2:4` binds as `[price] , ([cost] 2:4)`; to mean "(both columns) ∩ rows" write `([price],[cost]) 2:4`. Faithful and escape-hatchable.
2. **Comparison token** — a single comparison (`[qty] < [reorder]`) is a row-set atom; combinators (`and`/`or`/`not`) are a hard error pointing at xql. Parentheses are optional but recommended when intersecting, for the eye: `([qty]<[reorder]) [status] = "low"`.
3. **Named ranges** — `[a]:[d]` is the positional column span from one endpoint to the other, inclusive, in physical order, auto-normalized (`[d]:[a]` == `[a]:[d]`). Endpoints may mix kinds: `[day_05]:[day_10]`, `[day_05]:AF`.
4. **`d`/`p` dropped** — accepted. `del` and `show` replace ed's single letters; a 2-D delete must say row-or-column anyway, and the address does.
5. **Column-scoped regex** — `[col]~/re/` (and `[col]!~/re/`) select rows where that column matches / doesn't, awk-faithful. Bare `/re/` is the any-cell match. Both are row-set atoms.
6. **Comparison operators** — `==` `!=` `<` `>` `<=` `>=`. Equality is `==` (awk), reserving bare `=` exclusively for the assignment sigil so `[price] = [cost]` (assign) and `[price] == [cost]` (filter) never collide. One comparison per atom still holds; combinators error and point at xql.

## Resolved (slice 3, 2026-06-21 — full battery rendered)

7. **Comparison operands are exprs, string-wise unless cast** — `num([amount]) < 0` legal; `[qty]<[reorder]` is lexicographic, `num()`-cast for numeric order. Shares the operand grammar and operator set with expr (`expr-grammar.md`).
8. **Bracket-escape is `]]`** — a `]` inside a bracketed name is doubled.
9. **`s///` replacement dialect is sed's** (`\1`, `&`, `\U\L\u\l\E`), written by xled; **case/trim stay in `s///`**, compute lives in `expr-grammar.md`. **`if(cond,a,b)`** over `?:` (`:` collides with range).

Command set and execution semantics are locked in `semantics.md`; the compute layer in `expr-grammar.md`.
