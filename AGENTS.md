# Contributor and agent guide

`mnesis-bombay` integrates Mnesis with Actorpass without making either system
the application's architectural centre. Read
`docs/adr/0001-runtime-neutral-command-execution.md` before changing a public
boundary.

## Non-negotiable boundaries

- `mnesis-bombay-core` is runtime-neutral and `no_std`; it must not depend on
  Actorpass, Tokio, Tower, a concrete Mnesis store, transport, or serializer.
- `mnesis-actorpass` is the only crate that may translate Actorpass delivery
  into the core application port.
- `mnesis-bombay-tower` is optional interoperability, not the core abstraction.
- A Behavior validates and routes; the application service owns load, decide,
  append, conflict handling, and the command receipt.
- Durable follow-up events come from the committed Mnesis log. Never publish
  them as though durable before append succeeds.
- Preserve command identity, causation, correlation, tenant, and trace context
  across every boundary. Do not hide retry or idempotency policy in adapters.

## Local sibling layout

The current development manifest expects sibling checkouts named `actorpass`,
`behaviorpass`, and `nexus`; see `docs/local-sibling-development.md`. Actorpass
must be published or pinned as an accessible Git dependency before releases and
hermetic CI can be enabled.

## Required checks

Run `cargo fmt --all -- --check`, `cargo test --workspace`, and
`cargo clippy --workspace --all-targets --all-features -- -D warnings`.
Also run `nix flake check --no-build` for Nix evaluation. Record benchmark
changes with the machine, profile, worker count, sample size, and distribution;
do not use a single latency number as architectural proof.

Use `apply_patch` for hand edits, preserve unrelated work, and update the ADR
when changing ownership, failure, ordering, retry, or durability semantics.
