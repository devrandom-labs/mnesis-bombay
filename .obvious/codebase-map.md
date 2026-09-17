# mnesis-bombay — codebase map

Folder-level overview (depth 2). Dependency direction is strict: see ADR 0001.

| Path | What lives there |
|---|---|
| `crates/core/` | `mnesis-bombay-core` — runtime-neutral, `no_std` application protocol: `command.rs`, `context.rs`, `outcome.rs`, `phase.rs`, `addressed.rs`. Compile-fail UI tests under `tests/ui/` enforce boundary rules. |
| `crates/execution/` | `mnesis-bombay-execution` — composes Mnesis `CommandRepository` without a Bombay dependency; durable execution + factual outcomes. Integration tests: `tests/command_execution.rs` (12). |
| `crates/bombay/` | `mnesis-bombay` — the only crate translating Bombay delivery into the core port (`src/routing.rs`, `src/transport.rs`); tests: `tests/routing.rs`. |
| `crates/tower/` | `mnesis-bombay-tower` — optional Tower interoperability; no Bombay dependency. |
| `src/`, `tests/` | Root package `mnesis-bombay-workspace` (`publish = false`): `tests/bombay_entity_contract.rs` — end-to-end entity contract test against published Bombay/Mnesis crates. |
| `benches/` | `architecture_comparison.rs` — custom-harness comparative probe (direct vs service vs Tower routing; env-var driven). |
| `docs/` | `adr/0001-runtime-neutral-command-execution.md`, `implementation-roadmap.md`, `production-readiness-research.md`, `local-sibling-development.md`. `docs/plans/` and `docs/prompts/` are gitignored local working docs. |
| `.github/workflows/` | `checks.yml` (Nix flake check on every push/PR), release-plz + release + reserve-crate + update/upgrade_deps automation, codspeed benches. |
| `.cargo/`, `clippy.toml`, `rustfmt.toml`, `taplo.toml`, `deny.toml`, `audit.toml` | Toolchain/lint policy: cargo net config, clippy+pedantic denied, formatting, TOML fmt, cargo-deny/audit policy. |
| `flake.nix`, `flake.lock`, `rust-toolchain.toml` | Nix flake (crane/fenix/advisory-db) mirroring the pinned 1.96.0 toolchain; `nix develop` shell. |
| `.githooks/`, `.codex/`, `.envrc` | pre-commit hook, codex config, direnv (`use flake`). |

Architecture (from README, enforced):

```text
domain using Mnesis
        ↓
mnesis-bombay-core
        ↓
mnesis-bombay-execution ← direct hosts
        ↓                      ↖
      mnesis-bombay      optional Tower adapter
          ↙       ↘
 bombay-entity   Bombay
```
