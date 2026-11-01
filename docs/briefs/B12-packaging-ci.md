# B12 — Packaging, signing and release CI
**Paths** `.github/workflows/release.yml`, `Packager.toml` / `[package.metadata.packager]`, `xtask` · **Model** Sonnet ·
**Milestone** M1 · **Depends on** nothing.

## Goal
Tag `vX.Y.Z` → signed installers for Windows (MSI + NSIS), macOS (universal .dmg, notarised when secrets exist), Linux (AppImage,
.deb) attached to a GitHub Release with SHA256 sums and third-party licence report.

## Read first
`docs/research/rust-stack.md` §5 (tool comparison, runner images, platform specifics, signing from Linux with rcodesign),
`.github/workflows/ci.yml`, `deny.toml`.

## Scope
1. cargo-packager configuration (icons from `assets/icon.svg` → generated PNG/ICNS/ICO via xtask), product metadata, file
   associations for `.heartsim.ron`.
2. `release.yml`: matrix build on ubuntu-22.04 / windows-2025 / macos-15 (arm64 + x86_64 → `lipo`), `cargo-about` HTML, checksums,
   draft release; secrets-gated steps for Windows code signing (signtool or `osslsigncode`) and Apple notarisation (`rcodesign`).
3. `xtask release-check`: version consistency (workspace, CHANGELOG), `cargo deny`, licence report.
4. Linux: `.desktop` file, AppImage with bundled libxkbcommon/wayland deps as per cargo-packager defaults; test on a clean Ubuntu
   22.04 container (document).
5. Windows: `windows_subsystem` already set; verify no console; MSVC runtime static.
6. macOS: Info.plist with `NSHighResolutionCapable`, minimum 12.0, entitlements for hardened runtime.

## Acceptance tests
* Dry-run workflow on a test tag produces all artefacts; the AppImage runs `heartsim --version` in a container; MSI installs
  silently in a Windows runner step; `.dmg` mounts and the app launches `--version` on the macOS runner.
* `cargo xtask release-check` fails on version mismatch (unit-tested).
