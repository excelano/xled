# Releasing xled

The release loop for a new version. Run it from a clean `main` with the working tree committed.

1. **Bump the version.** Edit `version` in `Cargo.toml` (e.g. `0.1.0` → `0.1.1`). Update `Cargo.lock` with a build (`cargo build`), run `cargo test`, commit.

2. **Tag and push.** `git tag v0.1.1 && git push origin main --tags`. The `v*` tag triggers cargo-dist (`.github/workflows/release.yml`), which builds the five platform tarballs, the shell/PowerShell installers, and the checksums, then creates the GitHub Release.

3. **Build the .debs.** cargo-dist creates the release with the default `GITHUB_TOKEN`, and GitHub does **not** fire `release: published` for token-created releases (a documented anti-recursion safeguard). So `deb.yml` won't auto-run — dispatch it by hand:
   ```sh
   gh workflow run deb.yml -f tag=v0.1.1
   ```
   It builds amd64 + arm64 packages and uploads them to the release.

4. **Publish to crates.io.** From the repo root:
   ```sh
   cargo publish --dry-run   # full package + build + verify, no upload
   cargo publish
   ```
   You stay logged in after the first `cargo login`, so this is a single command. **Versions are immutable** — you can `cargo yank` a bad release to hide it from new dependency resolution, but never re-publish the same number. A fix is always a fresh version bump, never a re-push.

5. **Add the .debs to the Excelano apt repo.** Download the two `.deb`s from the release, then in `~/excelano-apt/`: `add-deb.sh` each one → `rebuild.sh` (GPG-signs) → `updatesite excelano.com.apt -y`. **Dry-run the rsync first** (`rsync … --delete -n`) and confirm zero deletions before the real push — the apt pool is a superset of live, and a stray `--delete` wipe is the standing hazard. See `feedback_rsync_parent_wipes_subpath`.

## Notes

- **crates.io API needs a User-Agent.** Requests without one return empty (`name: None`). To verify a publish from a script: `curl -s -H "User-Agent: …" https://crates.io/api/v1/crates/xled`.
- **First-time crates.io setup** (already done, kept for reference): GitHub-auth only, no separate signup; a **verified email** is required before the first publish; the API token needs the `publish-new` + `publish-update` scopes (crate-scoping can't restrict a not-yet-existing crate); `cargo login` with no argument reads the token from stdin, keeping it out of shell history.
- **docs.rs** rebuilds automatically on each publish — no action needed.
- The README, the landing page (`excelano.com/xled`), and `SECURITY.md` all reference the version implicitly via "latest"; none need a per-release edit.
