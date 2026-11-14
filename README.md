# Vehicle Manager

A self-hosted record of a household's vehicles and equipment: fuel-ups and meter readings, maintenance and repairs, issues, the maintenance schedule, recalls and bulletins, registrations, insurance, license plates, photos and PDFs, and each vehicle's life from purchase to sale. It runs on a home server (a Synology NAS) and is used from a browser on a phone or a computer, or from the desktop app, which keeps its own encrypted copy and works offline. Everyone in the household signs in with their own account and sees only what their role allows.

*"Vehicle Manager" is a working name.*

**Status: `v0.3.0`**, the end of Phase 3 of the roadmap ([`docs/plan.md`](docs/plan.md) §9). The server and the web app are in everyday use. The **desktop app** (macOS, Windows, Linux) keeps an encrypted copy of the household's records, works offline, and syncs when the server is back; it's built from source for now (published packages come in Phase 6). The mobile apps come after the MVP.

## Getting started (for the household)

Your Admin gives you the app's address (a `https://….ts.net` address on the household's tailnet) and a **temporary password**.

1. **Open the address** in your phone's or computer's browser, and **sign in** with your username and the temporary password. You'll choose your own password straight away (at least 12 characters; a few words strung together are fine).
2. **The Garage** lists the vehicles you can see. Tap one for its page: its **Overview**, **Issues**, **Timeline**, **Maintenance**, **Fuel**, and **Documents**.
3. **+ Log** (the **+** button on a phone) records something: a **fuel-up**, a **reading** (just the odometer or hours), or an **issue** ("the brake light is out"). What it offers depends on your role. A fuel-up or reading can be changed for 24 hours, then it locks; after that, a mistake is fixed with **Correct**.
4. **Modes:** the chip at the top right shows your mode. You start in **Driver** mode (fuel-ups, readings, issues). If you're a Maintainer, switch to **Maintainer** mode for jobs, the schedule, and documents. The household's Admin also has **Admin** mode (users, groups, vehicles), which asks for the password and ends after 15 idle minutes.
5. **Messages** (the inbox) shows what needs attention: a job left as a draft, a vehicle not back from the shop.
6. **Settings** has your units (miles or kilometers, gallons or liters), date format, and currency, and when you're signed out after being idle.

### The desktop app

The desktop app does everything the web app does, and keeps working when the server can't be reached.

1. **Sign in** with the server's address, your username, and your password. This **pairs** the computer with the server: it shows in **Settings → Devices**, where it can be signed out (from the web too, if the computer is lost).
2. **Choose a PIN** (at least 6 digits, not all the same and not a run like 123456). The app **locks** after 15 idle minutes (Settings → You → This device), and the PIN or your password unlocks it. After 5 wrong PINs only the password works; after 10 wrong tries, the computer signs out and its copy is deleted.
3. **Its copy:** the first time, the app downloads the household's records (what you may see) into an encrypted database on this computer, then keeps it up to date every minute. Thumbnails and PDFs download by themselves; a photo downloads when you open it, then it's kept (up to 2 GB; Diagnostics → Offline copy).
4. **Offline** (an **OFFLINE** tag at the top): you can read everything here and log fuel-ups, readings, issues, and jobs, with photos. Each change waits (**↑ 2 to send**) and goes to the server, in order, when it's back. Admin mode needs the server.
5. **A change the server refuses** (say, an odometer reading lower than one logged meanwhile) is never lost: it waits in **Messages** for you to fix or discard.
6. **PROTECTED** (a red tag and strip) means the server looks reset, restored to an older copy, or about to delete a lot: the app stops syncing and keeps this computer's copy safe until you choose, under **Details**. Tell the Admin. The household's Admin can rebuild a new server from their copy ([`docs/deploying-nas.md`](docs/deploying-nas.md), "Rebuilding the server from a desktop").
7. **Snapshots:** the app keeps an encrypted snapshot of its copy each day and before anything risky (the newest 7; Diagnostics → Snapshots of this copy).

Something wrong or missing? Tell the Admin, or open an issue on this repository.

## Running it

- **On a Synology NAS** (Docker, Container Manager, and Tailscale): [`docs/deploying-nas.md`](docs/deploying-nas.md). It covers the first setup, updating, backups and nightly snapshots, the main and test servers, and logs.
- **Images:** `ghcr.io/inner-marker/vehicle_manager_2`. A release's image is `:X.Y.Z`, which the main server runs (decision #070). The test server runs `:dev` and `:milestone-<name>`.
- **The desktop app,** built from the repository with the Rust toolchain and the Dioxus CLI (`cargo install --locked dioxus-cli@0.7.10`):
  - **To use it:** `cd crates/app && dx bundle --platform desktop --package-types app` (on macOS; `msi` on Windows, `deb` or `appimage` on Linux). The bundle is in `target/dx/app/bundle/`. It isn't signed, so the first launch needs a right-click → **Open** on macOS, or **More info → Run anyway** on Windows.
  - **To develop it:** `cd crates/app && dx serve --platform desktop`.

## The repository

Rust everywhere: [Dioxus](https://dioxuslabs.com) 0.7 fullstack for the server and the app, and SQLite through sqlx. The versions are pinned in `rust-toolchain.toml` and `Cargo.toml`.

| Crate | What it is |
|---|---|
| `crates/domain` | The domain: types, rules, checks, and messages. Pure (no I/O), so it also builds for the browser. |
| `crates/storage` | The SQLite database, its migrations, and the file store for photos and PDFs. |
| `crates/server` | The server's logic: permissions, the command pipeline, scheduled jobs. |
| `crates/app` | The Dioxus app (web and desktop), and the server binary that serves it. |
| `crates/geocoder` | The offline lookup from coordinates to a city and state. |

**Documents:**
- [`docs/plan.md`](docs/plan.md): the plan, the source of truth for what the app does and why.
- [`docs/decisions.md`](docs/decisions.md): settled decisions, numbered.
- [`docs/phase_plans/`](docs/phase_plans/): each phase's plan, milestone by milestone, with what was built.
- [`docs/sub-plans/`](docs/sub-plans/): detailed designs for one area each (drafts until confirmed).
- [`docs/ideas-to-discuss.md`](docs/ideas-to-discuss.md): ideas not decided yet.
- [`docs/test-plans/`](docs/test-plans/): the manual test plans still to be run; finished ones, with their results, are in [`docs/test-plans/archive/`](docs/test-plans/archive/).
- [`docs/ui-sketches/`](docs/ui-sketches/): the SVG layout options each screen was chosen from.
- [`docs/releasing.md`](docs/releasing.md): how a release is cut.
- [`AGENTS.md`](AGENTS.md): how the code is written and checked (for AI agents and people alike).

**Branches:**
- **`dev`** is the default branch, where work comes together.
- **`main`** holds releases only. It's updated from `dev` by a pull request merged with a merge commit (decision #021), and each release is a `vX.Y.Z` tag on it.
- Work happens on short branches from `dev`: `milestone/<phase>-<milestone>-<name>` for a milestone, `fix/<name>` for a fix, `release/vX.Y.Z` to prepare a release.
- **Pull request titles are [Conventional Commits](https://www.conventionalcommits.org)** (`feat: …`, `fix(app): …`, `docs: …`; a workflow checks them), because the changelog is made from them by [git-cliff](https://git-cliff.org).

**Checks:**
- **Every pull request** (the "Fast checks" workflow): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, the server build's clippy (`cargo clippy -p app --all-targets --no-default-features --features server -- -D warnings`), `cargo test --workspace`, and the server build's tests (`cargo test -p app --no-default-features --features server`).
- **Pull requests that touch the database code** (the "Desktop database" workflow): the storage tests with SQLCipher, the desktop's encrypted database, on Linux, Windows, and macOS.
- **Pull requests into `main`** (the "Main gate", again when a release is tagged): `cargo doc`, the Linux, Windows, and macOS builds, the WASM build, `cargo-deny`, the changelog check, and the third-party notices check.
- **Before committing,** agents also run the app's desktop clippy and the WASM check (`cargo check -p app --target wasm32-unknown-unknown`); see [`AGENTS.md`](AGENTS.md).

## License

Dual-licensed under either of the [MIT license](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option.

The app includes other people's work under their own licenses: icons from Lucide, pdf.js, SQLCipher, and many Rust crates. Their notices are in [`crates/app/assets/THIRD-PARTY-NOTICES.txt`](crates/app/assets/THIRD-PARTY-NOTICES.txt) (made by `scripts/notices.sh`), shown in the app under Diagnostics → About → **Licenses**.
