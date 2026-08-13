# Contributing to mnesis-bombay

Contributions are welcome. Architectural changes should begin by updating or
superseding ADR 0001 so ownership, failure, ordering, and durability semantics
are reviewable independently of code.

## Development setup

The committed graph uses released Bombay and Mnesis crates:

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
- Put Bombay translation in `mnesis-bombay` and Tower interoperability in
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

Bombay is available from crates.io. Published packages must retain registry
dependencies; local Mnesis path patches belong only in the workspace root and
must not leak into package manifests.

Contributions are licensed under MIT OR Apache-2.0, at your choice.
