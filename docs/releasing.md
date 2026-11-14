# Releasing

*How to cut a release (plan §8 Versioning & changelog, CI/CD). The human runs every step; agents may draft the commands.*

A release is a **version tag `vX.Y.Z` on `main`**. Pushing the tag starts `.github/workflows/release.yml`, which checks the release, re-runs the full gate, and creates the GitHub Release.

## One-time setup

- Install git-cliff (MIT/Apache): `cargo binstall git-cliff` (or `cargo install git-cliff --locked`). Check with `git cliff --version` (2.14 or later).
- The changelog format lives in `cliff.toml` at the repo root.
- Install cargo-about (MIT/Apache) for the third-party notices: `cargo install cargo-about --locked --features cli --version 0.9.3`. The `cli` feature is needed; without it, nothing is installed. The version is pinned so the notices come out the same everywhere (decision #079).

## Choosing the version

Semantic versioning: `MAJOR.MINOR.PATCH`.
- **Before 1.0:** bump MINOR for new features or breaking changes, PATCH for fixes only.
- **Any `!` / `BREAKING CHANGE:` commit** since the last release means at least a MINOR bump before 1.0, and MAJOR after.
- **The sync API version is separate** from the app version. Don't change it as part of a release unless the API itself changed.

## Steps

1. **Start from an up-to-date `dev`** with every milestone for this release merged:

   ```sh
   git switch dev && git pull
   git switch -c release/vX.Y.Z
   ```

2. **Set the version** in the root `Cargo.toml` (`[workspace.package] version = "X.Y.Z"`), then run `cargo check --workspace` so `Cargo.lock` picks it up.

3. **Generate the changelog** and review it:

   ```sh
   git cliff --tag vX.Y.Z -o CHANGELOG.md
   scripts/release/check-changelog.sh   # confirms a section exists for X.Y.Z
   ```

   - **Read the new section.** It becomes the GitHub Release notes word for word.
   - **If an entry reads badly,** don't edit `CHANGELOG.md`. Fix it at the source next time (commit messages); for now accept it, or regenerate after rewording an unpushed commit.

   **Bring the third-party notices up to date** (decision #079). The Main gate fails if they're stale:

   ```sh
   scripts/notices.sh
   ```

   If `crates/app/assets/THIRD-PARTY-NOTICES.txt` changed, commit it with the release.

4. **Commit and open the PR into `dev`** (it's outside a milestone, so it gets its own small PR):

   ```sh
   git add Cargo.toml Cargo.lock CHANGELOG.md crates/app/assets/THIRD-PARTY-NOTICES.txt
   git commit -m "chore: release vX.Y.Z"
   git push -u origin release/vX.Y.Z
   gh pr create --base dev --title "chore: release vX.Y.Z"
   ```

   Merge it once the fast checks pass.

5. **Open the release PR from `dev` into `main`**, and merge it with a **merge commit** (decision #021):

   ```sh
   gh pr create --base main --head dev --title "chore: release vX.Y.Z"
   ```

   The **Main gate** must pass: docs, `cargo-deny`, the Linux/Windows/macOS builds, the WASM build, and the CHANGELOG check.

6. **Tag the merged commit on `main` and push the tag:**

   ```sh
   git switch main && git pull
   git tag -a vX.Y.Z -m "vX.Y.Z"
   git push origin vX.Y.Z
   ```

7. **Watch the Release workflow** (Actions → Release). It should:
   - pass the guards (tag = `Cargo.toml` version, commit on `main`, changelog section present);
   - pass the Main gate again;
   - create the GitHub Release `vX.Y.Z` with the changelog section as its notes.

## If the release workflow fails

| Failed step | Likely cause | Fix |
|---|---|---|
| Tag matches the Cargo.toml version | Typo in the tag, or the version bump wasn't merged to `main` | Delete the tag (`git push --delete origin vX.Y.Z` and `git tag -d vX.Y.Z`), fix, re-tag |
| Tagged commit is on main | Tagged a commit on `dev` or a branch | Delete the tag; tag the merge commit on `main` |
| CHANGELOG.md has notes | Step 3 was skipped or used a different version | Redo steps 3–5, then re-tag |
| Main gate | A real failure on the tagged commit | Fix on `dev`, release a new PATCH version |
| Create the release | A release with that name already exists | Delete the draft or duplicate release in GitHub, then re-run the job |

## First release checklist (v0.1.0, end of Phase 1)

The first release proves the pipeline. After it, confirm:
- [ ] The guards job passed, and a deliberately wrong tag (e.g. `v0.0.0-test` on a branch) fails it. Delete that tag afterwards.
- [ ] The Main gate ran inside the Release workflow and passed.
- [ ] The GitHub Release `v0.1.0` exists, with notes matching the `## [0.1.0]` section of `CHANGELOG.md`.
- [ ] The server image was published as `:0.1.0` and `:latest` (GitHub → Packages → `vehicle_manager_2`).
- [ ] **Weekly checks** can be started from Actions → "Run workflow". (Since decision #026 made `dev` the default branch, scheduled and manual workflows run from `dev` and no longer wait for `main`.)

## Third-party notices (done in `v0.3.0`, decision #079)

The app embeds other people's work whose licenses ask for their notice to travel with every copy. The source repo already keeps each notice beside its files; **packages and the server image** must ship them too (the human, 2026-11-12):
- [x] **Lucide icons** (`crates/app/assets/icons/`, embedded in the app): ISC, with six icons (`chevron-down`, `chevron-up`, `monitor`, `moon`, `plus`, `x`) under Feather's MIT license; both in `LICENSE-lucide`.
- [x] **pdf.js** and its WebAssembly decoders (`crates/app/assets/pdfjs/`): Apache-2.0 and the licenses in `pdfjs/LICENSE`, `pdfjs/iccs/LICENSE`, and `pdfjs/wasm/LICENSE_*`.
- [x] **Rust crates** compiled in (including SQLCipher on the desktop): their licenses are already checked by `cargo deny`; `scripts/notices.sh` lists them with `cargo-about`, from the same data. It also adds two that `cargo-about` can't see: **SQLCipher's** own license (inside `libsqlite3-sys`) and **OpenSSL's** (built into the Windows desktop app by a build-time crate).
- [x] **Where they go:** one file, `crates/app/assets/THIRD-PARTY-NOTICES.txt`, made by `scripts/notices.sh` and committed. It's bundled with the app (web and desktop), and Diagnostics → About → **Licenses** shows it. The server image also has it at `/app/THIRD-PARTY-NOTICES.txt`. Desktop packages will carry it inside the app when they come (Phase 6).
