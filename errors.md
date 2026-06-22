# Errors & negative space

What xled refuses, and the exact words it refuses with. The address/composition grammar (`composition-grammar.md`), the commands (`semantics.md`), and the compute layer (`expr-grammar.md`) define what xled *does*; this file defines the edge — what it won't do, and how it says so. Examples here are synthetic.

## The thesis: an error is a router

Every xled refusal names where the capability actually lives. The user asked for something real; xled's job is not to apologize for lacking it but to point at the tool, the upstream step, or the corrected form that has it. A refusal that only says "no" wastes the one moment the user is paying attention. A refusal that says "no — it's over there, like this" keeps them moving. This is the same instinct as the SP-rejection style in xql (reject with a concrete rewrite), generalized: xql had one destination (SharePoint can't, so rewrite the SQL); xled has several, and the verb names which one.

## The message shape

One template, filled differently per destination:

```
<attempt> is not <verb>: <reason, one phrase>. <action>.
```

The `<verb>` is load-bearing — it tells the user, in one word, what *kind* of dead end they hit and therefore where to go next:

| Verb | Means | Goes to | Permanent? |
|---|---|---|---|
| not in **scope** | a different tool's job — query, not edit | xql / DuckDB | yes |
| not **supported** | reshaping — changes the table's shape | upstream reshape, or in-place `s///` | yes |
| not **recoverable** | the data was already destroyed before xled saw it | upstream re-export | yes |
| not available **yet** | a real xled feature, still being designed | a later xled | no |

A fifth case is *not* a boundary and does not use this shape: a **correction**, where the user wrote a legal form that means something other than they intended. That gets a different voice — name what they wrote, name what it does, then give the form for what they meant:

```
<what you wrote> <does X>; for <what you meant>, write <form>.
```

Two rules keep the catalog honest. The version-number rule: "not available yet" never carries a release number, because we will not promise syntax we have not designed (`append`, `undo`). It promises the direction, not a date. And the convert-on-ship rule (from the SP-rejection memo): transient wording is only for things genuinely in flight; the moment one ships, its error is deleted, and the moment one is ruled out forever, it moves up to a permanent verb.

---

## Catalog

### Out of scope → xql / DuckDB

xled edits cells; it does not answer questions about the table. The line is filter-as-scope (in) versus filter-as-result (out): selecting which rows to *edit* is addressing; producing rows, counts, or a reordering as the *answer* is a query. Every item here is permanent.

**Sort.** xled never reorders rows — order is the file's, and a save preserves it.
```
sort is not in xled's scope: xled edits cells in place and never reorders rows.
Sort upstream and pipe in — sort -t, -k3 file.csv | xled '…' — or let the
query engine do it: duckdb -c "FROM file.csv ORDER BY price".
```

**Multi-condition filter (`and` / `or` / `not`-as-logic).** One comparison is a legal address; the first combinator is the wall. This is the same boundary the address grammar draws (`composition-grammar.md` resolved item 2) and the same one expr draws (`expr-grammar.md`, no boolean operators) — stated once, enforced in both positions.
```
combining conditions with and/or is not in xled's scope: an address selects rows
to edit, it is not a query. For one more condition, run a second xled command on
the result; for a real predicate, query first — xql 'SELECT * WHERE qty < reorder
AND status = "active"' file.csv | xled '…'.
```

**Aggregate / group-by.** expr computes one value per row and never across rows.
```
aggregation (sum / count / group) is not in xled's scope: expr produces one value
per row, never a value across rows. Aggregate in the query engine —
duckdb -c "FROM file.csv SELECT dept, sum(cost) GROUP BY dept".
```

**Join.** One buffer, no cross-file matching.
```
joining tables is not in xled's scope: xled holds one buffer and never matches
rows across files. Join upstream, then scrub the result here —
duckdb -c "FROM a.csv JOIN b.csv USING (id)" | xled '…'.
```

### Reshaping → upstream, or `s///` in place

xled carves one rectangular table out of junk (`crop`, `header`, `drop blanks`, `fill down`) and cleans its cells. It does not change the carved table's *shape* — its row count by generating rows, its column count by splitting, or its column *order*. That invariant ("xled never reshapes") is what keeps it on the safe side of the line it shares with joins and aggregates. Permanent, but several have a real in-place rewrite.

**Split one cell into several columns.** Assignment writes one column; widening the row mid-table is reshaping.
```
splitting one cell into several columns is not supported: assignment writes one
column and never widens the table. For the common two-part case, rearrange in
place with s/// — [name] s/(.*), (.*)/\2 \1/ — or split upstream:
duckdb -c "FROM file.csv SELECT split_part(name, ', ', 1) AS last, …".
```

**Reorder / move columns.** Addressing reaches columns where they sit; it does not move them.
```
moving columns is not supported: xled addresses columns where they sit and never
reshapes the table. Reorder in the query engine — duckdb -c "FROM file.csv
SELECT dept, name, cost" — which is also where you would rename or drop them.
```

**Unpivot / collapse a multi-row header / merge stacked tables.** Shape changes, full stop.
```
unpivoting (wide → long) is not supported: it changes the table's shape, which
xled never does. Reshape in the query engine — duckdb -c "UNPIVOT file.csv …".
```

### Not recoverable → upstream re-export

The hardest refusal to accept, because it looks like xled's failure when it is Excel's. By the time the data is a CSV, the original digits are already gone; no downstream tool can un-destroy them. The error has to make clear the loss happened *before* xled and point at the only real fix — re-export from the source with the column typed as text. Where a deterministic reconstruction exists, offer it as a hedge the user opts into, never as a silent default.

**Stripped leading zeros.**
```
the leading zeros in [zip] are not recoverable: Excel dropped them on import, so
"00501" is already the number 501 in this file. Re-export from the source with the
column typed as text. If the width is fixed you can re-pad — [zip] s/^/0/ repeated
to 5 wide — but only you know the original width.
```

**Long IDs in scientific notation.**
```
the IDs in [account] are not recoverable: Excel stored them as 1.23E+11 and the
trailing digits are gone from the file. xled cannot reconstruct digits the
spreadsheet discarded — re-export from the source as text.
```

**Dates left as serial numbers.**
```
the dates in [date] are serial numbers, not dates: Excel exported 45292 instead of
2024-01-01. Re-export formatted as text if you can. If you trust the origin you can
convert by the epoch (Excel day 1 = 1899-12-30), but a wrong epoch corrupts every
row silently — re-export is safer.
```

**Leaked formula errors (`#REF!`, `#DIV/0!`, `#N/A`).** The cell *text* is scrubbable; the *value* it should hold is gone.
```
the #REF! in [total] is not a recoverable value: the formula that produced it broke
in Excel before export. You can blank the marker — [total] s/^#\w+!?$//  — but the
number it should hold has to be recomputed at the source.
```

### Not available yet → a later xled

Real features still being designed against real cases, not boundaries. Transient wording, no version number (convert-on-ship rule). Both below carry a usable workaround so "not yet" is never a full stop.

**Append a row.** The one structural op assignment doesn't cover — assignment creates columns, not rows. Deferred to design from battery evidence (`semantics.md`, "Still open"); David expects a real need (appending a record, a computed totals row), so it is in flight, not refused forever.
```
appending a row is not available yet: xled edits existing rows; row generation is
still being designed against real cases. For now append upstream —
printf 'a,b,c\n' >> file.csv — then reopen.
```

**Undo.** Leans in; lands in the implementation phase (ed had `u`, the buffer is live).
```
undo is not available yet: the buffer is mutable and reopening the file is the
current reset. Save in stages so you can reload a known-good state — undo is planned.
```

### Corrections — legal form, unintended meaning

Not boundaries; the user is inside the grammar but a step from a footgun. Name what they wrote, what it does, and the intended form. These already live as rules in `semantics.md` (10, 7, 6) and `composition-grammar.md` (6); this is their shared voice.

**Partial-rectangle delete** (semantics rule 10).
```
2:4 [status] del names a rectangle, and a rectangle has no row-or-column to drop.
To clear those cells, s/.*// over them; to delete, address whole rows (2:4 del)
or whole columns ([status] del).
```

**Assignment with not exactly one target** (semantics rule 7).
```
[price],[cost] = … assigns to two columns; assignment writes exactly one. Assign to
one column, or run two commands. (2:4 = … names rows with no column — same fix.)
```

**`=` where `==` was meant** (composition resolved item 6).
```
[price] = [cost] assigns cost into price — it is not a filter. For "rows where price
equals cost", use ==: [price]==[cost]. (Numeric compare needs a cast:
num([price])==num([cost]).)
```

**String comparison surprise** (expr value model). Not an error — comparisons are valid string-wise — but the previewer should *warn* when both operands parse as numbers yet differ from numeric order, since that is the A3 footgun in the wild.
```
note: [qty] < [reorder] compared "9" > "10" as text, not numbers. For numeric order,
cast: num([qty]) < num([reorder]).
```

**Cast-failure tally** (semantics rule 6). Lenient, non-blocking; the standard voice for the warning shown after a run.
```
3 cells skipped in [amount] (not numeric): rows 12, 88, 415 — left unchanged.
```

---

## Resolved (slice 4, 2026-06-21 — David gated all three)

1. **Column reorder is permanently "not supported"** (→ DuckDB). The invariant is *xled never reshapes the table*, and shape includes column *order*, not just row and column counts. Reorder is the camel's nose — "insert between" and "transpose" follow it — so order belongs to the file and the query engine; reordering for output is a one-line `SELECT`, and xled isn't an output formatter.

2. **No bulk header transform in v1** (permanent, gentle). `rename` stays one-at-a-time; "lowercase all headers" is rejected rather than served by a new header *scope* that would let `s///` reach the name overlay. A header pseudo-address is real new grammar for a rare need. Revisit only if the Part C intake battery shows bulk header rewrites are common.

3. **Every "not available yet" message ships a workaround.** A transient refusal without an escape hatch reads as a missing feature the user must wait on — the exact frustration the router thesis exists to kill. Costs one clause. `append` and `undo` both carry one.
