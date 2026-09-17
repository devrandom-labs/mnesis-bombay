# mnesis-bombay — agent guide

Typed integration between Mnesis and the Bombay runtime ecosystem. Rust **library
workspace** — there is no runnable app, no web server, and no external service; the
dev stack is the pinned Rust toolchain plus the cargo/Nix gates below.

## Snapshot

- Snapshot ID: `8h2x0hxm0sm5fbyxd001:default` — captured 2026-09-17T15:29:36.364Z from
  sandbox `idwknofz4jho8ou575omv` (computer `cmp_ymZtMuhq`)
- Restoring it gives you: rustup with pinned Rust 1.96.0 (clippy/rustfmt/llvm-tools,
  wasm32 + thumbv7em targets), a warm `target/` (debug + release), single-user Nix
  2.35.2 with flake inputs cached, and an authed `gh`.

## Stack

| Layer | Choice |
|---|---|
| Language | Rust, edition 2024, resolver 3 |
| Toolchain | Pinned **1.96.0** via `rust-toolchain.toml` — single source of truth for rustup AND the Nix flake |
| Package manager | cargo workspace, `Cargo.lock` committed |
| Crates | `mnesis-bombay-core` (runtime-neutral, `no_std`), `mnesis-bombay-execution`, `mnesis-bombay` (Bombay adapter), `mnesis-bombay-tower` (optional interop) |
| Key deps (published) | mnesis 0.3.1 / mnesis-store / mnesis-inmemory, bombay-rs 0.1.0, bombay-behavior 0.9.5, bombay-entity 0.1.0, tower 0.5 |
| Alt provisioning | Nix flake (crane + fenix + advisory-db) — also the CI gate |
| Services | None. Tests use in-memory stores. No Docker/Compose. |
| Env vars | None required. Optional bench knobs: `BENCH_ITERATIONS`, `BENCH_ACTORS`, `BENCH_COMMANDS_PER_ACTOR` |

## Commands

Run from the repo root; `source ~/.cargo/env` first in a fresh shell (rustup install).
Canonical entry per CONTRIBUTING.md: `nix develop` (or `direnv allow`), then:

```bash
cargo test --workspace --all-features                                # unit + integration + doc tests
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
BENCH_ITERATIONS=10000 cargo bench --bench architecture_comparison  # comparative probe
nix flake check -L --no-update-lock-file                             # full CI gate (needs Nix)
```

## Codebase map

See [codebase-map.md](codebase-map.md). Dependency direction is strict and enforced
by compile-fail tests — read `AGENTS.md` and ADR 0001 before touching a public boundary.

## Local Verification Summary

Verified 2026-09-17 on sandbox `cmp_ymZtMuhq` (Debian 13, 8 cores) with pinned Rust
1.96.0 installed via rustup:

| Gate | Result |
|---|---|
| `cargo test --workspace --all-features` | pass — 30 tests, 0 failures across 5 suites + doc-tests |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo fmt --all -- --check` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps` | pass |
| `BENCH_ITERATIONS=10000 cargo bench --bench architecture_comparison` | pass — direct 913 ns/op, service_pipeline 2889, service_roundtrip 66279, tower_pipeline 1480, tower_roundtrip 68713; ratios 3.164 / 72.582 / 1.621 / 75.247 |
| `nix flake check -L --no-update-lock-file` (exact CI gate) | pass (exit 0) |

Primary flow exercised end-to-end: `tests/bombay_entity_contract.rs` (7 tests) —
command execution through the Bombay adapter into Mnesis with committed aggregate
reload — plus the comparative probe asserting reloaded state matches the iteration
count. This workspace is libraries + tests, so the green gates above ARE the health
check; there is no server or DB to probe.

## Notes for agents

- Lints deny `clippy::pedantic` workspace-wide and `missing_docs` warns — new public items need doc comments. `unsafe_code` is forbidden.
- `crates/core` is `no_std` and must never depend on Bombay, Tokio, Tower, or a concrete Mnesis store.
- Local sibling `[patch.crates-io]` overrides are temporary and must never be committed (`docs/local-sibling-development.md`).
- `nix flake check --no-build` fails on a cold store (fenix IFD never writes `channel-rust-*.toml.drv`); run the full `nix flake check -L` or rely on CI.
- Read before structural work: `AGENTS.md`, `CONTRIBUTING.md`, `docs/adr/0001-runtime-neutral-command-execution.md`, `docs/production-readiness-research.md`.
- Durable skill record: [skills/local-dev/SKILL.md](skills/local-dev/SKILL.md).
