# Vehicle Manager

A self-hosted record of a household's vehicles and equipment: fuel-ups and meter readings, maintenance and repairs, issues, the maintenance schedule, recalls and bulletins, registrations, insurance, license plates, photos and PDFs, and each vehicle's life from purchase to sale. It runs on a home server (a Synology NAS) and is used from a browser on a phone or a computer. Everyone in the household signs in with their own account and sees only what their role allows.

*"Vehicle Manager" is a working name.*

**Status: `v0.2.0`**, the end of Phase 2 of the roadmap ([`docs/plan.md`](docs/plan.md) §9). The server and the web app are in everyday use. The desktop app (macOS, Windows, Linux) runs from source. Working offline and syncing between devices come in Phase 3; the mobile apps come after the MVP.

## Getting started (for the household)

Your Admin gives you the app's address (a `https://….ts.net` address on the household's tailnet) and a **temporary password**.

1. **Open the address** in your phone's or computer's browser, and **sign in** with your username and the temporary password. You'll choose your own password straight away (at least 12 characters; a few words strung together are fine).
2. **The Garage** lists the vehicles you can see. Tap one for its page: its **Overview**, **Issues**, **Timeline**, **Maintenance**, **Fuel**, and **Documents**.
3. **+ Log** (the **+** button on a phone) records something: a **fuel-up**, a **reading** (just the odometer or hours), or an **issue** ("the brake light is out"). What it offers depends on your role. A fuel-up or reading can be changed for 24 hours, then it locks; after that, a mistake is fixed with **Correct**.
4. **Modes:** the chip at the top right shows your mode. You start in **Driver** mode (fuel-ups, readings, issues). If you're a Maintainer, switch to **Maintainer** mode for jobs, the schedule, and documents. The household's Admin also has **Admin** mode (users, groups, vehicles), which asks for the password and ends after 15 idle minutes.
5. **Messages** (the inbox) shows what needs attention: a job left as a draft, a vehicle not back from the shop.
6. **Settings** has your units (miles or kilometers, gallons or liters), date format, and currency, and when you're signed out after being idle.

Something wrong or missing? Tell the Admin, or open an issue on this repository.

## Running it

- **On a Synology NAS** (Docker, Container Manager, and Tailscale): [`docs/deploying-nas.md`](docs/deploying-nas.md). It covers the first setup, updating, backups and nightly snapshots, the main and test servers, and logs.
- **Images:** `ghcr.io/inner-marker/vehicle_manager_2`. A release's image is `:X.Y.Z`, which the main server runs (decision #070). The test server runs `:dev` and `:milestone-<name>`.
- **The desktop app,** from the repository: `cd crates/app && dx serve --platform desktop`, then the server's address.

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
- [`docs/test-plans/`](docs/test-plans/): the manual test plans, with their results.
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
- **Pull requests into `main`** (the "Main gate", again when a release is tagged): `cargo doc`, the Linux, Windows, and macOS builds, the WASM build, `cargo-deny`, and the changelog check.
- **Before committing,** agents also run the app's desktop clippy and the WASM check (`cargo check -p app --target wasm32-unknown-unknown`); see [`AGENTS.md`](AGENTS.md).

## License

Dual-licensed under either of the [MIT license](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
