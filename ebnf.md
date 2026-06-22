# EBNF — the whole xled grammar, formalized

Slice 6. The five prior slices decided the language; this file makes it precise. It
consolidates `composition-grammar.md` (addressing), `semantics.md` (commands), `expr-grammar.md`
(compute), and the slice-5 intake verbs into **one grammar**, and uses the act of doing so as a
check: anything the proving ground (`proving-ground.md`, Parts A/B/C) writes that this grammar
cannot derive is either drift to fix or a real open call. Three such cases surfaced — recorded
under *Findings* below, since slice 6 is consolidation, not a fresh design round.

This is a specification, not a decision. Where the consolidation exposed an unsettled point, it is
flagged for David's gate, not resolved by fiat.

## Meta-notation

Relaxed EBNF (W3C/XML-spec flavor), chosen over ISO 14977 because ISO uses `,` for concatenation
and `,` is one of xled's own operators — visual collision avoided.

| Form | Means |
|---|---|
| `::=` | is defined as |
| juxtaposition | concatenation (in order) |
| `\|` | alternation |
| `( … )` | grouping |
| `x?` `x*` `x+` | optional / zero-or-more / one-or-more |
| `"…"` | a literal terminal — **all terminals are quoted** |
| `[A-Z]` | a character class (lexical rules only) |
| `(* … *)` | comment |

Rule: meta-symbols (`?` `*` `+` `|` `(` `)`) are never quoted; xled's own `?`-less operators
(`*` `/` `+` `-` `,` `|`-free) are always quoted. So `"*"` is xled's multiply; `*` is "zero-or-more."

**Whitespace is insignificant everywhere except one place:** between two reference atoms a run of
spaces is the *intersection operator* (Excel's). That single significant-space rule is written as
`SP` below and called out again in Disambiguation note 1.

---

## The grammar

### Program & statement

```
program     ::= statement*
statement   ::= reference command?      (* command omitted ⇒ implicit `show` (report-state) *)
              | command                 (* reference omitted ⇒ whole-table scope *)
```

The top-level shape is `reference command` (DESIGN's organizing rule). A bare reference inspects;
a bare command scopes the whole table. How the parser finds the seam between the two is
Disambiguation note 1.

### Reference — the address (Excel reference algebra + two atoms)

Precedence, lowest binding to highest: `,` union  <  `SP` intersection  <  `:` range  <  `!` negate.
Parentheses override (composition-grammar resolved item 1).

```
reference   ::= union
union       ::= intersect ( "," intersect )*
intersect   ::= negate ( SP negate )*           (* SP = one+ literal spaces = intersection *)
negate      ::= "!" negate | primary
primary     ::= "(" reference ")" | rowset | range

range       ::= positional ( ":" positional? )? | ":" positional
positional  ::= cell | column | rownum | name | "$"

rowset      ::= regexSel | colRegexSel | comparison
regexSel    ::= "/" regexBody "/" "i"?
colRegexSel ::= name ( "~" | "!~" ) "/" regexBody "/" "i"?
comparison  ::= concat cmpOp concat              (* exactly one cmpOp — operands are sub-comparison exprs *)
```

`comparison`'s operands are `concat` (defined under Expr), which sits *below* comparison
precedence and therefore cannot itself contain a `cmpOp`. That single fact enforces "one comparison
per address atom, no `and`/`or` chaining" structurally — the combinator wall
(composition-grammar resolved item 2) needs no special rule, the grammar just can't express it.

Covered forms (all from the Part A/C battery): `2` `2:4` `2:` `:4` `$` `C` `AF` `B:D` `B2` `B2:C3`
`[price]` `[a]:[d]` `[day_05]:AF` `/re/` `/re/i` `[col]~/re/` `[col]!~/re/` `[qty]<[reorder]`
`num([qty])<num([reorder])` `!1` `!/active/i`, and every intersection/union/paren combination of them.

### Command

```
command     ::= subst | assign | word

subst       ::= "s" DELIM regexBody DELIM replBody DELIM substFlag*
assign      ::= "=" expr

word        ::= "del"
              | "show"
              | "crop"
              | "header"
              | "rename" REST_OF_LINE
              | "fill" "down"?            (* see Finding 2 *)
              | "drop" "blanks" dropAxis?  (* see Finding 1 — newly formalized this slice *)
              | "describe"                 (* see Finding 1 — newly formalized this slice *)

dropAxis    ::= "rows" | "cols"
substFlag   ::= "g" | "i" | rownum        (* rownum = the Nth-occurrence flag *)
```

`subst` borrows sed's any-delimiter rule: the character immediately after `s` is `DELIM`, and the
same character closes both fields — so `s#(..)/(..)#…#` lets slashes live in the data
(semantics.md). `rename` takes `REST_OF_LINE` so spaced/slashed/parenthesized header names need no
quoting (`[notes.txt] rename notes`). Reserved words are ≥3 letters; a column literally named one
is reached bracketed (`[fill]`, `[drop]`).

### Expr — the compute layer (RHS of `=`, and the operands of `comparison`)

Precedence, lowest to highest: comparison < `&` < `+ -` < `* /` < unary `-` < atom/call.

```
expr        ::= concat ( cmpOp concat )?         (* the optional comparison yields a bool *)
concat      ::= addsub ( "&" addsub )*
addsub      ::= muldiv ( ( "+" | "-" ) muldiv )*
muldiv      ::= unary ( ( "*" | "/" ) unary )*
unary       ::= "-" unary | atom
atom        ::= number | string | bool | name | call | "(" expr ")"
call        ::= fnName "(" ( expr ( "," expr )* )? ")"

fnName      ::= "num" | "bool" | "len" | "left" | "right" | "mid"
              | "substr" | "round" | "default" | "coalesce" | "if"
```

Columns are **always bracketed in expr** (a bare identifier is a function name — expr-grammar). The
only difference between an address-position `comparison` and an expr-position comparison is which
production reaches it: in address position a `cmpOp` makes a row-set; in RHS position it makes a
bool value. Same operators, same operands — defined once here.

No boolean `and`/`or`/`not`: there is no production for them. Multi-condition logic nests through
`if()`/`coalesce()`; a real predicate is xql's job (expr-grammar, locked).

### Lexical tokens

```
column      ::= [A-Z]+                           (* A..Z, AA.., positional — always uppercase *)
cell        ::= [A-Z]+ [0-9]+                     (* e.g. B2 — fused positional, letters only *)
rownum      ::= [0-9]+
number      ::= "-"? [0-9]+ ( "." [0-9]+ )?
bool        ::= "true" | "false"
cmpOp       ::= "==" | "!=" | "<=" | ">=" | "<" | ">"
DELIM       ::= any one character                (* sed-style; same char closes the field *)

name        ::= "[" nameChar* "]"
                (* a literal "]" inside the name is doubled "]]"; "[" needs no escape.
                   case-sensitive, exact: [userId] ≠ [userid]. *)
string      ::= '"' strChar* '"'                 (* '\"' escapes a double-quote inside *)
```

`regexBody`, `replBody`, `REST_OF_LINE` are opaque char runs, defined by prose not by char-class:

- **`regexBody`** — `regex`-crate syntax. `^`/`$` are **cell-bounded** (anchor to cell start/end,
  not row/line); `^$` matches an empty cell (semantics rule 4).
- **`replBody`** — xled's **sed replacement dialect**, written by xled over the crate's captures:
  `\1`–`\9` backrefs, `&` whole match, `\U \L \u \l \E` case-folding (semantics, slice-3 lock).
  This is why case/trim live in `s///`, not in expr.
- **`REST_OF_LINE`** — the literal remainder of the input line, verbatim, used only by `rename`.

---

## Disambiguation — what the EBNF alone cannot encode

Three resolutions are lexical/lookahead facts, not context-free productions. The grammar above is
written to *agree* with them, but a parser implements them directly.

1. **Reference ↔ command boundary (maximal munch + lexical command class).** `SP` is both the
   intersection operator *and* the space before a command, so `2:4 del` could read two ways. The
   rule (composition-grammar, the load-bearing decision): the parser consumes the **maximal leading
   reference expression**, then exactly one command. Commands are lexically distinct from reference
   atoms — *sigil-led* (`s` immediately followed by a delimiter; `=`) or a *reserved word*
   (`del show crop header rename fill drop describe`). A column ref `S` is `s` followed by space /
   `,` / `:` / end — never by a delimiter — so `s/` is always substitute and `=` is never a column.
   This is why the boundary is decidable without a separator (the rejected Candidate-2 `|`).

2. **Column atom ↔ comparison (one-token lookahead).** A leading `[name]`, `rownum`, `number`,
   `(` … could begin either a positional/range *or* the left operand of a `comparison`. Resolve by
   lookahead: parse one `concat`, then peek — a `cmpOp` next ⇒ it's a `comparison` (row-set);
   otherwise it was a plain reference atom. (Bare letters like `C` never start an expr — expr
   columns are bracketed — so `C` is unambiguously a column.)

3. **`$` — last-row vs end-anchor (by position).** As a `positional`, `$` is the last data row. The
   *same character* inside `regexBody` (`/active$/`) is the regex end-anchor — but there it lives
   inside the opaque regex field, so the address grammar never sees it. No conflict; disambiguated
   by which production owns the character.

---

## Scope contracts — syntax legal, but the address must fit the verb

The EBNF accepts any `reference command` pairing; these constraints are **semantic** (enforced at
execution, with the `errors.md` voice), not grammatical. Listed here so the spec is complete in one
place.

| Command | Required address shape | Empty address? | On violation |
|---|---|---|---|
| `s///` | any cell scope | whole table | — |
| `= expr` | exactly **one** column (existing or new name/letter) × a row scope | error | "assignment writes exactly one" (semantics 7) |
| `del` | whole rows **or** whole columns | error | "can't delete a partial region" (semantics 10) |
| `show` | any | whole table | — |
| `crop` | a rectangle / range | error | needs a region |
| `header` | exactly one row | error | one row only |
| `rename` | exactly one column | error | one column |
| `fill` | a column or columns | error | column scope |
| `drop blanks` | edges of the working table (rows and/or cols) | whole table | — |
| `describe` | any (advisory only — never mutates) | whole table | — |

`del`'s "whole rows or whole columns" and `=`'s "exactly one column" are the cases the *corrections*
section of `errors.md` already voices — the scope contract and the error catalog are the same
boundary stated twice.

---

## Conformance — the proving ground is the test suite

Every command line in `proving-ground.md` Parts A, B, and C must be derivable from this grammar.
Walking all three batteries against the productions above: **all of Part A and Part B derive
cleanly**, confirming slices 2–4 were internally consistent. Part C exposed three forms the
*intake* work (slice 5) used that the *command set* (locked in slice 2) never absorbed — the
Findings below. With those three productions added (`drop blanks`, `describe`, `fill down`), Part C
derives cleanly too. The grammar and the battery now agree end to end.

---

## Findings — drift slice 6 caught (David ratified all three, 2026-06-22)

Consolidation did its job: it found three command forms that live in the slice-5 fixtures, taxonomy,
and Part C render but were never added to `semantics.md`'s command set. None is a new feature; each
is a form already in use that needed an official production. The productions above carry the
ratified resolutions — `"fill" "down"?`, `"drop" "blanks" dropAxis?`, and `describe` are now law.

1. **`drop blanks` and `describe` were never in the command set.** `semantics.md` locks
   `del show crop header rename fill` — but slice 5 introduced `drop blanks` and `describe` as
   first-class intake verbs (taxonomy, Part C). They are real and used; they were just never written
   into the reserved-word list. *Resolution taken:* added both as reserved words (`drop` takes a
   required `blanks` object; `describe` is bare). **Open sub-question:** does `drop blanks` name its
   axis explicitly (`drop blanks rows` / `drop blanks cols`) or infer it? I made the axis an
   *optional* qualifier — bare `drop blanks` trims fully-empty **edge** rows and columns (matches
   seam 2: edge-only, crop-before-drop), `rows`/`cols` restricts it. Recommend keeping it optional;
   the bare form is what the battery actually writes.

2. **`fill` vs `fill down`.** `semantics.md` locks the verb as bare `fill` (down-only in v1). The
   slice-5 fixtures and Part C render write `fill down`. *Resolution taken:* `"fill" "down"?` — bare
   `fill` and `fill down` are the same command, `down` an optional explicit direction. This keeps
   semantics.md's lock valid, keeps the battery valid, and future-proofs `fill up`/`fill right` as a
   non-breaking later addition rather than a v1 token we'd have to retrofit. Recommend adopting.

3. **`y/set/set/` and `append` are reserved-not-grammar.** Transliterate (`y///`) is *deferred*
   (semantics) and row-`append` is *not available yet* (errors.md, "still open"). Neither has a
   production here — deliberately. They are named in the reserved space so a future slice slots them
   in without collision, but committing syntax now would violate the "no syntax we haven't designed"
   rule (errors.md, the version-number rule). Flagging only so their *absence* reads as intentional.

---

## Resolved (slice 6, 2026-06-22)

- **One grammar, four sources reconciled.** Addressing, commands, compute, and intake now derive
  from a single EBNF; the three split spec files remain the prose rationale, this is the formal
  surface.
- **The combinator wall is structural, not a rule.** `comparison` operands are `concat` (below
  comparison precedence), so `and`/`or` chaining is *inexpressible*, not merely rejected — the
  cleanest possible statement of the slice-2 boundary.
- **The three things EBNF can't hold are named and bounded** (ref/command munch, column/comparison
  lookahead, `$` by position) — a parser implements exactly these three, nothing more.
- **Scope contracts and the error catalog are the same boundary** — listed once as a table here,
  voiced once as corrections in `errors.md`.
- **Drift surfaced, not buried** — `drop blanks` / `describe` / `fill down` were used but
  unlegislated; now written and **ratified (David, 2026-06-22)**: `fill down` is optional-direction
  sugar over bare `fill` (future-proofs `fill up`/`right` non-breaking); `drop blanks` takes an
  optional `rows`/`cols` qualifier, bare = trim empty edges; `y///`/`append` stay reserved-not-grammar.

With the gate closed, **the design phase is complete.** Six slices: composition → commands →
full render → errors → intake → EBNF. The grammar is locked, the proving ground is the
conformance suite, the boundaries are voiced. Next is code.

Next: implementation. The proving ground becomes executable conformance tests; the parser is built
to this EBNF and looped (parser → evaluator) until Parts A/B/C are green. That is where `/loop`
fits.
