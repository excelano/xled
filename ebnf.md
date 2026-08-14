# EBNF — the whole xled grammar, formalized

The whole of xled's grammar in one place. It consolidates `composition-grammar.md` (addressing),
`semantics.md` (commands), `expr-grammar.md` (compute), and the intake verbs into **one grammar**.
The consolidation doubles as a check: anything the proving ground (`proving-ground.md`, Parts
A/B/C) writes that this grammar cannot derive is drift.

This is the formal surface. The three split spec files above carry the prose rationale; where they
disagree with this file, this file is the grammar.

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

rowset      ::= regexSel | colRegexSel | comparison | callSel
regexSel    ::= "/" regexBody "/" "i"?
colRegexSel ::= name ( "~" | "!~" ) "/" regexBody "/" "i"?
comparison  ::= concat cmpOp concat              (* exactly one cmpOp — operands are sub-comparison exprs *)
callSel     ::= call                             (* a *call* only — not any bool expr; see below *)
```

`comparison`'s operands are `concat` (defined under Expr), which sits *below* comparison
precedence and therefore cannot itself contain a `cmpOp`. That single fact enforces "one comparison
per address atom, no `and`/`or` chaining" structurally — the combinator wall
(composition-grammar resolved item 2) needs no special rule, the grammar just can't express it.

`callSel` is a function call standing alone as the row-set: `regexmatch([org], "^APP$") del`. It
exists because a call is the one expr shape that answers true or false without a `cmpOp` in it, and
requiring `== true` to address on one would be ceremony over a value that is already a bool.

It is deliberately `call` and not `concat`. Widening the atom to any bool-valued expr would put a
bool on both sides of a would-be `and` and turn the combinator wall from a fact of the grammar back
into a rule a parser has to enforce; a call form leaves it standing, since `f(…) and g(…)` still has
no production. A call that returns something other than a bool is a *semantic* error — the same
shape of correction as a call to a function the library does not have — because whether a name
returns a bool is not knowable from the grammar.

Covered forms (all from the Part A/C battery): `2` `2:4` `2:` `:4` `$` `C` `AF` `B:D` `B2` `B2:C3`
`[price]` `[a]:[d]` `[day_05]:AF` `/re/` `/re/i` `[col]~/re/` `[col]!~/re/` `[qty]<[reorder]`
`num([qty])<num([reorder])` `regexmatch([org],"^APP$")` `!1` `!/active/i`, and every
intersection/union/paren combination of them.

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

fnName      ::= lowerWord                        (* any; the library is expr-grammar.md's *)
```

A function name is syntactically just a lowercase word followed by `(` — which is why columns must
be bracketed. Whether that word names a function the library has is a *semantic* question, answered
with a correction that names the intended form, not a parse failure. Enumerating the library here
would only give it a second place to go stale.

Columns are **always bracketed in expr** (a bare identifier is a function name — expr-grammar). The
only difference between an address-position `comparison` and an expr-position comparison is which
production reaches it: in address position a `cmpOp` makes a row-set; in RHS position it makes a
bool value. Same operators, same operands — defined once here.

No boolean `and`/`or`/`not`: there is no production for them. Multi-condition logic nests through
`if()`/`coalesce()`; a real predicate is xql's job (expr-grammar, locked). A variadic *function*
whose arguments are values rather than predicates — `in(x, a, b)` — is not a combinator by another
name and does not reopen this; the argument is in expr-grammar, beside the refusal.

### Lexical tokens

```
column      ::= [A-Z]+                           (* A..Z, AA.., positional — always uppercase *)
cell        ::= [A-Z]+ [0-9]+                     (* e.g. B2 — fused positional, letters only *)
rownum      ::= [0-9]+
number      ::= "-"? [0-9]+ ( "." [0-9]+ )?
bool        ::= "true" | "false"
lowerWord   ::= [a-z]+                           (* a function name; "true"/"false" bind first *)
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

   `callSel` needs no lookahead at all: a lowercase word followed immediately by `(` is a call and
   nothing else, because a column is bracketed or uppercase and a command word is followed by a
   space. The two words that shape are *not* — a reserved command (`del(`, a malformed command) and
   a catalogued capability (`sum(`, which gets its refusal) — are decided by name, before the
   grammar is consulted.

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
All three batteries derive cleanly against the productions above, and the grammar and the battery
agree end to end.

---

## Three productions that exist for a reason

Each of these looks like a small syntactic choice and is load-bearing. They are recorded here so a
future change doesn't quietly undo one.

1. **`drop blanks` takes an *optional* axis.** Bare `drop blanks` trims fully-empty **edge** rows and
   columns; `rows`/`cols` restricts it to one axis. The bare form is what the conformance battery
   writes, and edge-only matches the crop-before-drop seam.

2. **`fill down` is optional-direction sugar over bare `fill`.** `"fill" "down"?` makes both spellings
   the same command. This future-proofs `fill up` and `fill right` as non-breaking later additions
   rather than a token that would have to be retrofitted.

3. **`y/set/set/` and `append` are reserved, not granted a production.** Transliterate is deferred
   (`semantics.md`) and row-`append` is not available yet (`errors.md`). They are named in the
   reserved space so a later addition slots in without collision; committing syntax now would violate
   the no-syntax-we-haven't-designed rule. Their *absence* from the grammar is intentional.

---

## What this grammar settles

- **One grammar, four sources reconciled.** Addressing, commands, compute, and intake all derive from
  a single EBNF. The three split spec files remain the prose rationale; this is the formal surface.
- **The combinator wall is structural, not a rule.** `comparison` operands are `concat` (below
  comparison precedence), so `and`/`or` chaining is *inexpressible* rather than merely rejected.
  `callSel` is held to a `call` rather than a `concat` to keep it that way.
- **The three things EBNF cannot hold are named and bounded** — ref/command munch, column/comparison
  lookahead, and `$` by position. A parser implements exactly these three, nothing more.
- **Scope contracts and the error catalog are one boundary.** Listed once as a table here, voiced once
  as corrections in `errors.md`.
