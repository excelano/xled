# Releasing xled

The release loop lives in `~/notes/releasing.md` — the ordered steps, the apt
step, crates.io, the winget submission, the spent-tag rule, and the standing
facts about tokens and secrets. Failure recipes are in
`~/notes/build_release_gotchas.md`. This file carries what is true of xled and
not of its siblings.

| | |
|---|---|
| Loop | cargo-dist |
| Version lives in | `version` in `Cargo.toml` |
| `apt-ship` argument | `xled` |
| crate | `xled` |
| winget package | `Excelano.xled` |
| Windows asset | `xled-x86_64-pc-windows-msvc.zip` |

**The crate, the command, the Homebrew formula, and the apt package are all
`xled`** — one name everywhere, unlike xray, whose crate is the hyphenated
`x-ray`. cargo-dist's tarballs and installer are named after it:
`xled-installer.sh`, `xled-<target>.tar.xz`.

**The release builds** the five platform tarballs, the shell and PowerShell
installers, the Homebrew formula, and the checksums, then creates the GitHub
Release. The `.deb` packages come from the separately dispatched `deb.yml`.

**xled trips `Validation-Executable-Error` by design.** Bare invocation with no
arguments takes an intentional usage guard and exits non-zero, which winget's
bare-invocation sweep reports as a failure. Recipe in the gotchas file; do not
change the guard to appease it.
