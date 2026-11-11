# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately through GitHub Security Advisories at https://github.com/excelano/xled/security/advisories/new. If you would rather not use GitHub, email david.anderson@excelano.com instead. I aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest release receives security fixes. Older versions are not supported. A fix ships in a new release; update the way you installed.

## What xled can access

xled is a CLI that runs locally on your machine. It reads the CSV or DSV file you point it at (or standard input), holds it in memory for the duration of the session, and writes to disk only when told to: `write` in the REPL, or `-i` in a one-shot run, which rewrites the input file in place and, given a suffix (`-i.bak`), first copies the original beside it under that suffix. `--install-skill` writes xled's Claude Code skill to `~/.claude/skills/xled/`, and `--uninstall-skill` removes that directory. It makes no network calls of any kind, has no auth layer, and implements no administrative operations. It can only read and write files your operating-system user already has access to.

## What xled stores

xled stores nothing outside the files you explicitly write and the skill directory above. The REPL's line-editing history lives in memory for the session and is never saved to disk; there is no config directory, no telemetry, no analytics, and no remote logging.

## Verifying releases

Every GitHub release carries the `.deb` packages and a Windows zip, built by `build-release`, the release program in excelano/shipping, from a tagged commit. The Excelano apt repository is signed, so `apt` verifies each package it installs, and WinGet checks the zip against the SHA-256 recorded in its manifest. To check a file downloaded from the release page by hand, compare its `sha256sum` with the digest GitHub lists for that asset.
