# mnesis-bombay

Typed integration between Mnesis and the Bombay ecosystem's current runtime,
Actorpass.

Actorpass replaces the historical Bombay runtime. The repository name denotes
the wider Bombay family (`behaviorpass`, `bombay-communication`,
`bombay-address`, `bombay-observe`, and `bombay-timers`); it does not mean this
project targets both runtimes.

## Architecture

The dependency direction is strict:

```text
domain using Mnesis
        ↓
mnesis-bombay-core
   ↙             ↘
direct hosts   optional Tower adapter
        ↘       ↙
      mnesis-actorpass
             ↓
          Actorpass
```

The leading design keeps Behavior pure. It emits a typed execution request;
Actorpass interprets the request through a typed service route that owns
durability, key affinity, cache policy, conflicts, and replies. Reliable event
delivery originates from the committed Mnesis log, never a transient
post-append actor send.

Read [ADR 0001](docs/adr/0001-runtime-neutral-command-execution.md) before
changing dependency direction or introducing a handler/mediator abstraction.

## Status

ADR 0001 records the recommended architecture. Its public API remains
provisional until the failure, allocation, durable-store, and high-cardinality
gates in the ADR pass. The workspace includes the comparative executable probe.

```console
nix develop
cargo test --workspace
cargo bench --bench architecture_comparison
```
