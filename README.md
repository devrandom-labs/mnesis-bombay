# mnesis-bombay

Typed integration between Mnesis and the Bombay runtime ecosystem. The current
`bombay-rs` core supersedes the legacy implementation formerly stored in the
Bombay repository; this project targets only the current runtime.

## Architecture

The dependency direction is strict:

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

The execution crate composes Mnesis's existing `CommandRepository` without a
Bombay dependency; direct hosts and aggregate actors share that exact durable
path. The leading design keeps Behavior pure. Bombay Entity owns stable local entity
identity, activation, admission, routing, draining, and passivation; Bombay
owns exact-incarnation execution and effect interpretation. The Mnesis adapter
owns hydration, durable execution, conflict/ambiguity policy, and factual
replies. Reliable event delivery originates from the committed Mnesis log,
never a transient post-append actor send.

Read [ADR 0001](docs/adr/0001-runtime-neutral-command-execution.md) before
changing dependency direction or introducing a handler/mediator abstraction.
The [production-readiness audit](docs/production-readiness-research.md) assigns
aggregate resolution, activation, supervision, backpressure, deduplication,
event relay, operations, security, testing, and upstream changes explicitly.

## Status

ADR 0001 records the recommended architecture. Its public API remains
provisional until the failure, allocation, durable-store, and high-cardinality
gates in the ADR pass. The workspace includes the comparative executable probe.

```console
nix develop
cargo test --workspace
cargo bench --bench architecture_comparison
```
