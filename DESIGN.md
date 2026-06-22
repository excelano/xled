# xled — design

**Status:** Design draft, 2026-06-21. Name: `xled` (excel + ed, pronounced "sled"); confirmed available on crates.io.

**One line:** sed and awk for tabular data — a live-buffer editor that applies regex transformations to spreadsheet-style ranges over CSV and other delimiter-separated files.

## The problem

Two genuinely good ideas live inside a spreadsheet: ranges and formulas. Ranges are a clean way to think about a region of tabular data; formulas, at their best, are little functions applied across that region. Both are absent from the command line, and so is the whole mindset behind them, even though that mindset is the one most people bring to tabular data. The scriptable tools (awk, pandas, SQL, DuckDB, xql) threw ranges away and replaced them with positional slicing or set queries; the spreadsheet-mindset tools (Excel, sc-im, VisiData) kept ranges but are not scriptable, not pipeable, and — in Excel's case — have no regular expressions at all. Nothing occupies the cell where spreadsheet ranges meet a scriptable, regex-aware command language. That empty cell is what this tool fills.

This is built for one user's real need, not a market. The need is the recurring shape in day-to-day CSV work: scrub and reshape specific columns and cells by pattern, iteratively, on messy data. Filtering, joining, and aggregating are explicitly someone else's job.

## The user

The primary user is Claude Code, working alongside David inside a live data session. That is the design's organizing principle, not a footnote. It changes the success metric from "is this elegant" or "will a market adopt it" to "does this collapse the friction between us when we are both elbow-deep in a CSV." Two consequences follow directly and are requirements, not polish. The tool must ship a one-page, example-dense reference that maps every feature to its sed or awk analog, so an LLM gains working fluency on first contact. And every operation must be cheaply previewable before commit, because an operation whose effect cannot be seen will be distrusted and routed around in favor of `print(df)`.

The corollary that governs every syntax decision: the tool is adopted because it is *unoriginal*. Its sed and awk fluency transfers wholesale. Inventing a clever unified dialect that resembles both but is identical to neither destroys that transfer and guarantees the tool goes unused. Borrowed syntax stays verbatim in its own position. (See the addressing rule below for the one principled exception.)

## What it is

A combination of three ancestors, decomposed by what each genuinely contributes:

The **field model is awk's**, and it is the non-negotiable foundation, because awk is the only one of the three that natively knows what a column is. Fields are columns; the record is a row. sed cannot name a column and so cannot be the base.

The **headline command is sed's** `s/re/rep/flags`. awk can substitute, but its `gsub` is function-call-flavored and clunky; sed's substitute is the ergonomic gold standard and the operation the user reaches for by reflex.

The **buffer is ed's**. sed and awk are stream processors that read, transform, and forget. This tool holds a live, mutable, in-memory buffer that is edited and then deliberately saved — the editor model, not the stream model. The execution model's grandparent is ed.

So: awk's column-aware world as the skeleton, sed's best verb as the headline command, ed's buffer as the heartbeat. It runs both as an interactive REPL (where the live buffer and incremental editing pay off) and as a non-interactive runner that takes a script and a file, for pipeline composition.

## Data model

A file is an ordered, positional list of rows; each row is an associative array of fields keyed by column header. Rows are positional and unnamed (CSV has no row labels); columns are named. That dual nature is the source of the two-axis addressing below. CSV is the working term but stands in for every delimiter-separated format the nved family already handles.

Values are stored as strings — always, losslessly, so a save round-trips exactly and `02134` is never silently mangled into `2134`. There are three types: string, number, and boolean. A cell is a string until an explicit cast (`num`, `bool`) commits it to a type at the point of arithmetic or logic. awk's own "string unless used as a number" philosophy is close to this and is on our side; the one deliberate departure is requiring the explicit cast rather than inheriting awk's implicit coercion, to keep leading zeros and ID-like fields safe. Recompute and stored formulas are explicitly out of scope; a computed value is written once, not maintained as a live dependency.

## Addressing — the core

This is where the language actually lives. An address selects a region; a command operates on it. The grammar is two-axis: a row component and a column component, each of which can be positional or content-based.

Rows are addressed by number (`2`), range (`2:4`), or regex match (`/re/`). Columns are addressed by spreadsheet letter (`C`, and past Z, `AF`) or by header name. Spreadsheet A1 range notation (`B2:C3`) is a first-class positional address. Regex against a single column is column-scoped selection (the structured, spreadsheet-faithful reading); regex against a whole row's raw text is the ed-faithful reading; both are supported because both are useful.

**Letters are intrinsic; names are an overlay.** Every file has an A, B, C… letter grid whether or not its first row is headers — letters are positional and always available. Header names are an optional naming layer laid over that grid when a header row is present. This single model dissolves several questions at once: addressing never depends on whether a header exists (letters always work), a headerless file is simply one with no overlay, and a column with a blank or duplicate header is still reachable by its letter. The header row gets one explicit flag in the model — present or absent — and is never mistaken for data.

**Named columns are bracketed; positional addresses stay bare.** This is the one place real data forced the grammar's hand. Real-world headers are hostile to a bare-token grammar: they contain the substitute delimiter `/`, hyphens (iCal-export headers shaped like `X-RECORD-VALUE`), and spaces and parens (`first name`, `price (USD)`). A bare name address is therefore impossible in general. The resolution borrows Excel's own idiom for "a column name that may contain anything" — structured-reference brackets: `[price]`, `[first name]`, `[price (USD)]`, `[2024]`, `[B]`. Letters, numbers, and A1 ranges stay bare: `C`, `2:4`, `B2:C3`. This is maximally unoriginal — `Table[Column Name]` is exactly how an Excel power user already references a messy header — and it pays off three times over: it disambiguates the column named `B` (`[B]`) from the column at letter `B` (`B`), the header `2024` (`[2024]`) from row 2024 (`2024`), and it answers quoting-in-addresses (brackets do the job; quotes are not an address device). Bracketed names match case-sensitively and exactly — `[userId]` is not `[userid]` — because a header is data and silent case-folding is the same class of surprise as silently dropping a leading zero; a case-insensitive match is an explicit opt-in, never the default.

A corollary the same data settles: the range operator is `:`, never `-`. Hyphenated headers (`X-RECORD-VALUE`) make a hyphen range operator unparseable, and A1 notation already uses `:`. Real headers ratify the choice.

The departure-from-fluency rule, which governs all syntax: depart from sed and awk only where the departure reads instantly on sight — column letters, header names, A1 ranges, and now bracketed names all qualify, because nobody fumbles "A is the first column" or "`[price]` is the column named price." Stay verbatim-faithful everywhere the syntax is arbitrary: substitute mechanics, function names, expression syntax. Self-evident departures cost nothing; arbitrary ones cost adoption.

What remains open is narrower than the first draft thought. With bracket-vs-bare settling every individual *token*, the unresolved problem is purely how a row component and a column component *compose* into one address: the fused A1 form (`B2:C3`, axes jammed into one coordinate) alongside the split form (rows `/re/`, column `[status]` or `C:AF`). Excel fuses the axes; ed keeps them separate and composable. One grammar still has to make both legal, and that composition is the first thing to settle in the next working session against the real corpus.

## Commands

Two command styles, chosen by task, kept syntactically faithful to their origins:

sed's substitute, for pattern rewrites: `price s/\$//g` strips the dollar sign from the price column. This is the primary, most-used operation — the regex transform Excel never had, now scoped to a range.

awk's compute-and-assign, for derived values: `price = num(price) * 1.1`. Different operation from a rewrite, so different syntax is honest rather than redundant. The "little functions" are awk built-ins (sum, average, a match, text functions) invoked inside the command position, not procedural blocks.

The hard line: **no general control flow.** awk's `if`/`for`/`while` and BEGIN/END are the mechanism by which a small language metastasizes into a bad general-purpose one, and the ed family's freedom from that mechanism is exactly what keeps it thin after fifty years. We take awk's fields and built-in functions and refuse awk's control flow. The top-level grammar stays flat: address, then command.

## Non-goals — the boundary

Filter-as-query, join, and aggregate belong to DuckDB and xql, which do them better. Join and aggregate are clean total exclusions; the tool has nothing in common with them. Filter requires a scalpel, because filtering is half the addressing model and cannot be excluded — only divided. Filter-to-scope-an-edit is in: it is the `address command` core, since you cannot strip a prefix from "the tools rows" without first selecting them. Filter-to-produce-a-result is out: that is a query. The usable rule is whether the filter exists in order to change the selected cells (stay) or whether the filtered subset is itself the answer (leave). Printing matching rows is fine as inspection; the moment the subset wants to be saved as a deliverable, it has become a query.

The trap to refuse explicitly is letting addressing grow a boolean predicate algebra plus ORDER BY plus GROUP BY. Each step feels small; the bottom of the slope is a worse SQL. The address selects rows by range or regex match, full stop.

Accepting this boundary obligates clean handoff, which is the price of staying narrow. The real workflow is: xql or DuckDB shapes and subsets the data, this tool scrubs the result by pattern, then hands back. That only works if the seams are frictionless — read what those tools emit, emit what they read, and provide an awk-style one-shot invocation (`xled 'price s/\$//g' data.csv`) so the tool drops cleanly into a pipe: `duckdb … | xled … | xql …`. A one-trick tool that cannot hand off is an island, and an island is worse than pandas.

## Platform and approach

Rust. The in-memory buffer over potentially large files (the real corpus runs to 600k-row files) rewards Rust's low overhead and absence of GC pauses; the tool is a real parser-and-evaluator that suits Rust's enums and parsing ecosystem; it joins the existing compiled-CLI-via-cargo-dist line; and it advances an active learning goal. The accepted cost is slower iteration than Go, justified by performance, fit, and learning value.

Two crates carry the load that would otherwise be hand-rolled: `regex` for the engine and `csv` for the parser. This is a deliberate reversal of ved's choice. ved hand-rolled its regex engine as a learning exercise, not as a design principle; here the value is the two-dimensional addressing, the engine is a means to it, and a battle-tested crate that handles bounded repetition, group-star, and Unicode for free frees all effort for the novel work. The `csv` crate likewise earns its place the moment real data appears — embedded commas, escaped quotes, and embedded newlines (all present in the corpus) are exactly where naive splitting breaks.

Not a fork of ved. ved is a one-dimensional line editor; this is a two-dimensional tabular tool with a different buffer structure, a different address grammar, and a different command set. Forking would start us with a large amount of scaffolding that is wrong for the new shape — ved is a cousin, not a parent variant, and the nved precedent (a fresh take sharing concepts, not a fork) is the right model. Build the two-dimensional buffer and address grammar fresh; the regex and CSV handling come from crates rather than from ved.

## Open questions

The live one is the address grammar's composition rule: how a row component and a column component combine into a single address, making both the fused A1 form (`B2:C3`) and the split form (rows `/re/`, column `[status]` or `C:AF`) legal under one grammar. The token-level questions are settled — bare is positional, bracketed is named — so this is what remains, and it is the first thing to settle in the next session against the real corpus.

Settled since the first draft: the regex engine (the `regex` crate, not ved's hand-rolled one); named-vs-positional disambiguation (bracket vs bare); the range operator (`:`, not `-`); case-sensitivity of name matches (exact, with an opt-in insensitive mode); and header handling (letters intrinsic, names an optional overlay, one present/absent flag, never mistaken for data).
