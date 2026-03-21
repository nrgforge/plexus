# Releasing Plexus

Releases are automated via [cargo-dist](https://github.com/axodotdev/cargo-dist). Pushing a version tag triggers CI to build binaries, create a GitHub Release, and publish the Homebrew formula.

## Cutting a Release

```bash
# 1. Make sure main is clean and tests pass
cargo test
git status

# 2. Bump the version in Cargo.toml
$EDITOR Cargo.toml

# 3. Regenerate the release workflow (picks up any dist config changes)
dist generate

# 4. Commit the version bump
git add Cargo.toml Cargo.lock dist-workspace.toml .github/workflows/release.yml
git commit -m "chore: bump version to X.Y.Z"

# 5. Tag and push
git tag vX.Y.Z
git push && git push --tags
```

CI handles the rest: builds binaries, creates the GitHub Release, and publishes to the Homebrew tap.

## Install Methods

```bash
# Homebrew (macOS/Linux)
brew install nrgforge/tap/plexus

# Shell installer (macOS/Linux)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/nrgforge/plexus/releases/latest/download/plexus-installer.sh | sh

# PowerShell (Windows)
powershell -ExecutionPolicy ByPass -c "irm https://github.com/nrgforge/plexus/releases/latest/download/plexus-installer.ps1 | iex"
```

## Updating cargo-dist

Run `dist init` again to update — it will walk through config and regenerate the workflow.
