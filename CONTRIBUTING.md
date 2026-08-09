# Contributing to mnesis-bombay

Contributions are welcome. Architectural changes should begin by updating or
superseding ADR 0001 so ownership, failure, ordering, and durability semantics
are reviewable independently of code.

## Development setup

This early workspace uses local sibling checkouts. Place `mnesis-bombay`,
`actorpass`, `behaviorpass`, and `nexus` under the same parent directory, then:

```bash
direnv allow # or: nix develop
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo bench --bench architecture_comparison
```

See `docs/local-sibling-development.md` for dependency and release constraints.

## Changes

- Keep the runtime-neutral core free of runtime, executor, transport, and
  concrete storage dependencies.
- Put Actorpass translation in `mnesis-actorpass` and Tower interoperability in
  `mnesis-bombay-tower`.
- Test pure domain decisions, application execution, adapter contracts, and
  end-to-end runtime behavior separately.
- Include failure-path tests for conflicts, cancellation, lost replies,
  duplicates, backpressure, and poison events when implementing those paths.
- Record benchmark environment and latency distribution; pipeline enqueue and
  durable round-trip are different measurements.
- Use focused conventional commits such as `feat:`, `fix:`, `docs:`, and
  `test:`.

## Release

Publishing is intentionally blocked until Actorpass is available from an
accessible immutable Git revision or crates.io. Do not publish manifests with
local path dependencies.

Contributions are licensed under MIT OR Apache-2.0, at your choice.
