# Grammar notes

Why the address grammar is shaped the way it is. Each decision below was forced or ratified by hammering the synthetic fixtures (`fixtures/`) against a large real-world CSV corpus (~108 files, kept out of this repo). Examples here are synthetic; the corpus only supplied the *shapes*.

## The collision that decided named-column syntax

A sed-style grammar wants the address as a bare prefix: `/re/s/a/b/`. That works for line text. It breaks the moment a column *name* is the address, because real headers carry the characters the grammar reserves:

| Hostile shape | Example header | Collides with |
|---|---|---|
| slash | `Owner / Region` | the `s///` delimiter |
| hyphen | `X-RECORD-VALUE` (iCal-export style) | a `-` range operator |
| spaces + parens | `price (USD)`, `first name` | token boundaries |
| punctuation | `notes.txt` | nothing yet, but fragile |

A bare name address is therefore impossible in general. Three real naming conventions also coexist in one corpus — `SCREAMING_SNAKE`, `camelCase`, `snake_case` — so the grammar cannot assume a normalized header.

**Resolution: bracket named columns, keep positional addresses bare.** This is Excel's own structured-reference idiom (`Table[Column Name]`), so it transfers for free.

```
C          column by letter (intrinsic — always available, A..Z, AA..)
AF         multi-letter column past Z
C:AF       column span
2          row by number
2:4        row range
B2:C3      A1 rectangle (fused positional)
/re/       rows matching regex
[price]    column by name
[first name]      name with a space
[price (USD)]     name with spaces + parens
[X-RECORD-VALUE]  name with hyphens
[2024]     numeric-looking name
[B]        column literally named "B"
```

Bare = positional, bracketed = named. That one rule resolves every disambiguation the fixtures were rigged to force:

- column **named** `B` is `[B]`; column at **letter** B is `B`
- header `2024` is `[2024]`; row 2024 is `2024`
- quoting-in-addresses is moot — brackets are the device, quotes are not

## Corollaries the same data settled

**Range operator is `:`, never `-`.** Hyphenated headers make a `-` range unparseable, and A1 already uses `:`. Decided, not preference.

**Name matching is case-sensitive and exact.** `camelCase` headers (`userId`) sit next to `snake_case` ones. `[userId]` must not match `userid`. A header is data; silently case-folding it is the same class of surprise as dropping a leading zero from `02134`. Case-insensitive matching is an explicit opt-in, never the default. (Confirmed with David, 2026-06-21.)

**Letters are intrinsic; names are an overlay.** Every file has an A,B,C grid regardless of whether row 1 is a header. Names are a convenience layer over that grid when a header exists. Consequences:

- addressing never depends on header presence — letters always work
- a headerless file is just one with no overlay; no mode-detection needed for addressing
- a blank or duplicate header column is still reachable by letter
- the header row needs exactly one flag in the model: present or absent

## Still open

How a row component and a column component **compose** into one address. The tokens are settled; the composition is not. Excel fuses the axes (`B2:C3`); ed keeps them separate (an address, then a command). One grammar has to make both the fused form and the split form (`/re/` rows × `[status]` column) legal. That is the next working session against the corpus.

## Validated by real data, no change needed

- **Lossless string storage** — codes like `7AH5`, `02134`, zero-padded dept codes appear constantly; any numeric coercion destroys them.
- **Strip-and-cast on money** — quoted thousands-comma values (`"4,001"`, `"37,441.48"`) are the single most common real transform; the `s///`-then-`num()` path is the headline use case.
- **The `csv` crate earns its place** — embedded commas, escaped `""quotes""`, and multi-line records inside quoted fields all occur; naive split-on-comma breaks on them.
- **Multi-letter columns are mandatory** — real files reach ~99 columns, so addressing must go past Z (AA, AB, …) for real, not as a curiosity.
