# Releasing Orchestrix

This repo now uses a tag-driven release flow built around GitHub Releases, signed updater artifacts, and a stable auto-update channel.

## Policy

- `main` is the releasable branch.
- Manifest versions use SemVer without a `v` prefix, for example `0.2.0` or `0.2.0-beta.1`.
- Git tags and GitHub releases use the same version with a `v` prefix, for example `v0.2.0`.
- Stable releases use plain SemVer tags like `v0.2.0`.
- Prereleases use SemVer prerelease tags like `v0.3.0-beta.1` or `v0.3.0-rc.1`.
- The in-app updater tracks only the stable channel because it is wired to GitHub's `releases/latest/download/latest.json`, which ignores prereleases.
- Commit messages should keep the existing conventional format (`feat:`, `fix:`, `chore:`) so generated GitHub release notes stay readable.

## Supported Release Assets

- Windows: NSIS installer `.exe` and MSI
- macOS: Apple Silicon DMG
- Linux: AppImage and `.deb`

Each tagged release also uploads updater metadata (`latest.json`) and signatures so installed builds can update in place.

Updater signing is in place. Platform-native code signing and notarization are separate concerns and are not automated by this release flow yet.

## One-Time Setup

1. Generate an updater signing key pair.

```powershell
bunx tauri signer generate -w $HOME/.tauri/orchestrix-updater.key
```

2. Store the private key in GitHub Actions secrets for this repo.

```powershell
Get-Content $HOME\.tauri\orchestrix-updater.key -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY
```

3. If you protect the key with a password, add that too.

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

The public key is safe to commit and is already embedded in [src-tauri/tauri.conf.json](C:\Users\ghost\Desktop\Coding\Rust\Tauri\Orchestrix\src-tauri\tauri.conf.json).

## Release Workflow

1. Sync the version everywhere.

```powershell
bun run version:set 0.2.0
```

2. Run the local release verification pass.

```powershell
bun run release:verify
```

3. Commit the version bump.

```powershell
git add package.json bun.lock src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/tauri.benchmark.conf.json
git commit -m "chore(release): v0.2.0"
```

4. Tag the release.

```powershell
git tag -a v0.2.0 -m "Release v0.2.0"
```

5. Push the commit and tag.

```powershell
git push origin main --follow-tags
```

6. Watch or open the GitHub release once the workflow finishes.

```powershell
gh release view v0.2.0 --web
```

## CI and Release Workflows

- [desktop-ci.yml](C:\Users\ghost\Desktop\Coding\Rust\Tauri\Orchestrix\.github\workflows\desktop-ci.yml) builds installable bundles on pushes and pull requests and keeps workflow artifacts for smoke testing.
- [release.yml](C:\Users\ghost\Desktop\Coding\Rust\Tauri\Orchestrix\.github\workflows\release.yml) runs on `v*` tags, asserts the tag matches every tracked version file, signs updater artifacts, uploads release assets, and publishes generated release notes.

## Local Release Builds

When you want to test a signed updater build locally, export the signing key first and build with the release-specific Tauri config:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $HOME\.tauri\orchestrix-updater.key -Raw
bun run tauri build -- --config src-tauri/tauri.release.conf.json
```

`src-tauri/tauri.release.conf.json` turns on `createUpdaterArtifacts` only for release builds, so day-to-day CI and dev builds do not require signing secrets.
