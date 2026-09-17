---
name: local-dev
description: Durable record of local dev onboarding for mnesis-bombay (Rust workspace, no services)
---

# local-dev — mnesis-bombay onboarding record

First successful onboarding run: 2026-09-17, sandbox computer `cmp_ymZtMuhq`
(snapshot `8h2x0hxm0sm5fbyxd001:default`).

## What this repo needs

Rust **1.96.0** exactly (pinned in `rust-toolchain.toml`; also consumed by the Nix
flake). A C linker. Network to crates.io (all deps are published registry crates).
**Nothing else** — no DB, no Docker, no env vars.

## Reproduce from a bare machine

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
source ~/.cargo/env
rustup show          # auto-installs 1.96.0 + clippy/rustfmt/llvm-tools + wasm32/thumbv7em targets
cargo fetch
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
BENCH_ITERATIONS=10000 cargo bench --bench architecture_comparison
```

Optional Nix (single-user, no daemon):

```bash
curl -sL https://nixos.org/nix/install -o /tmp/nix-install.sh && sh /tmp/nix-install.sh --no-daemon --yes
mkdir -p ~/.config/nix && printf 'experimental-features = nix-command flakes\naccept-flake-config = true\n' > ~/.config/nix/nix.conf
. ~/.nix-profile/etc/profile.d/nix.sh
nix flake check -L --no-update-lock-file   # full CI gate; slow (compiles via crane)
```

## Verification evidence from the 2026-09-17 run

- `cargo test --workspace --all-features`: **30 passed / 0 failed**
  (routing 2, compile_fail 1, protocol 8, command_execution 12, bombay_entity_contract 7, doc-tests ok)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass
- `cargo fmt --all -- --check`: pass
- `RUSTDOCFLAGS=-D warnings cargo doc --workspace --all-features --no-deps`: pass
- `BENCH_ITERATIONS=10000 cargo bench --bench architecture_comparison`: pass —
  `direct_ns_per_op=913 service_pipeline_ns_per_op=2889 service_roundtrip_ns_per_op=66279
  tower_pipeline_ns_per_op=1480 tower_roundtrip_ns_per_op=68713
  service_pipeline_ratio=3.164 service_roundtrip_ratio=72.582
  tower_pipeline_ratio=1.621 tower_roundtrip_ratio=75.247`

## Gotchas learned

1. `nix flake check --no-build` fails on a cold store: fenix `fromToolchainFile`
   uses import-from-derivation, and under `--no-build` the
   `channel-rust-1.96.0.toml.drv` is never written → "path … is not valid".
   Use the full `nix flake check -L` or rely on CI. UPDATE: the full `nix flake check -L --no-update-lock-file` PASSED locally on 2026-09-17 (exit 0) — prefer it over --no-build.
2. The bench is a **custom harness** (`harness = false`, its own `main`), not criterion.
   With no env vars it runs a single smoke iteration; `BENCH_ITERATIONS` runs all
   five architectures and prints one metrics line; `BENCH_ACTORS` + `BENCH_COMMANDS_PER_ACTOR`
   run the multi-actor scaling probe.
3. Fresh shells must `source ~/.cargo/env` (rustup installed with default-toolchain none;
   the pinned toolchain resolves automatically inside the repo).
4. Clippy re-checks the whole workspace with the driver — expect a separate ~5 min
  compile pass after the test build on a cold cache.
5. Never commit `[patch.crates-io]` sibling overrides (docs/local-sibling-development.md).

## Primary flow to exercise on restore

Run `cargo test --workspace --all-features` and confirm the
`bombay_entity_contract` suite (7 tests) passes — that is the end-to-end
entity contract against published Bombay/Mnesis crates. For a performance
smoke, run the bench with `BENCH_ITERATIONS=10000`.
