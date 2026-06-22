# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately through GitHub Security Advisories at https://github.com/excelano/xled/security/advisories/new. If you would rather not use GitHub, email david.anderson@excelano.com instead. I aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest 0.x release receives security fixes. Older versions are not supported.

## What xled can access

xled is a CLI that runs locally on your machine. It reads the CSV or DSV file you point it at (or standard input), holds it in memory for the duration of the session, and writes the buffer back to disk only when you issue a write command. It makes no network calls of any kind, has no auth layer, and implements no administrative operations. It can only read and write files your operating-system user already has access to.

## What xled stores

xled stores nothing outside the files you explicitly write. The interactive REPL keeps a line-editing history file in your home directory (the standard `rustyline` behavior); there is no config directory, no telemetry, no analytics, and no remote logging.

## Verifying releases

Every GitHub release includes a `.sha256` file next to each archive listing its SHA-256 hash. Verify any download before running it:

    sha256sum xled-x86_64-unknown-linux-gnu.tar.xz
    # compare against the value in xled-x86_64-unknown-linux-gnu.tar.xz.sha256

Release artifacts are built by GitHub Actions from a tagged commit using the cargo-dist configuration in this repo (`dist-workspace.toml` and the generated `.github/workflows/release.yml`). The workflow and build configuration are public and auditable.
