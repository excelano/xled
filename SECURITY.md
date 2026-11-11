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

Every GitHub release includes a `.sha256` file next to each archive listing its SHA-256 hash. Verify any download before running it:

    sha256sum xled-x86_64-unknown-linux-gnu.tar.xz
    # compare against the value in xled-x86_64-unknown-linux-gnu.tar.xz.sha256

Release artifacts are built by `build-release`, the release program in excelano/shipping, from a tagged commit using the cargo-dist configuration in this repository (`dist-workspace.toml`). That build configuration is public and auditable.
