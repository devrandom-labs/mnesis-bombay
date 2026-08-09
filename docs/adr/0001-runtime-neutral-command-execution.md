# ADR 0001: Runtime-neutral Mnesis execution with an Actorpass service route

- Status: Accepted boundary; production implementation gated by the capability
  audit
- Date: 2026-08-09
- Decision owners: Mnesis and Actorpass maintainers
- Scope: command execution, runtime integration, replies, event propagation,
  modularity, testability, and performance

## Decision summary

`mnesis-bombay` is a separate integration repository. Mnesis and Actorpass do
not depend on each other.

The leading architecture is:

1. Mnesis remains the domain and durability authority.
2. A small runtime-neutral core carries command identity and typed outcomes.
3. Actorpass Behaviors remain pure. A command Behavior emits a typed
   `ExecuteRequest` in its `Sends` algebra.
4. An Actorpass `DeliveryRouter` adapts that request to a runtime-neutral
   command service. Actorpass awaits routing before reading the next mailbox
   event, preserving per-actor sequential turns.
5. Aggregate roots are cached in a bounded key-affine execution service, not
   made authoritative actor state. The Mnesis stream remains authoritative.
6. Tower interoperability is optional. Tower is not part of the domain or core
   contract.
7. Reliable event propagation reads the committed Mnesis log and uses explicit
   receipts, checkpoints, retries, poison handling, and optional consumer
   inboxes. Immediate Actorpass sends after append are not a reliable outbox.
8. No generic mediator, dynamic message bus, global registry, `Any`, `TypeId`,
   or erased universal envelope is introduced.

This decision is provisional until the failure, readiness, allocation,
durable-store, and high-cardinality performance gates below pass.

The complete production capability inventory, repository ownership decisions,
upstream requirements, activation state machine and release gates are normative
in [`../production-readiness-research.md`](../production-readiness-research.md).
This ADR alone must not be interpreted as a production-ready implementation.

## Naming

Actorpass replaces the historical Bombay runtime. This repository does not
support two competing actor runtimes. The `mnesis-bombay` name refers to the
Bombay family of focused libraries. Runtime-specific production code belongs in
the `mnesis-actorpass` crate.

## Context

Mnesis provides:

- pure aggregate decision through `Handle<C>`;
- event-sourced aggregate reconstruction;
- typed `Repository<A>` load/save;
- `CommandRepository<A>::execute` for decide then durable append;
- optimistic concurrency and typed conflicts;
- committed global positions as read-your-writes tokens;
- subscriptions, projections, snapshots, sagas, and projected intents.

Behaviorpass provides a typed transition algebra. A Behavior consumes one typed
event and returns typed actions: sends, child creations, phase transition, stop,
or error. Its architecture intends this transition to be pure, although Rust's
async trait signature cannot enforce the absence of I/O.

Actorpass supplies the runtime interpretation:

- typed mailboxes and actor references;
- sequential behavior turns;
- bounded user-lane backpressure;
- child lifecycle and supervision;
- timers and completion observation;
- statically selected `RouteSends` and `DeliveryRouter` capabilities.

The integration must not turn any one of these libraries into a framework that
owns every concern.

## Quality attributes

The decision optimizes for correctness first, then independent layering,
testability, flexibility, and measured performance.

### Correctness

- Durable acceptance means append success, not receipt, enqueue, fold, or
  decision.
- Aggregate ID, stream key, actor address, actor incarnation, command ID,
  correlation ID, and subscription position are distinct types and facts.
- A version conflict means another append won. It does not prove a duplicate of
  the same command was applied.
- A lost reply after append is ambiguous unless a durable command inbox records
  command identity and outcome atomically with the append.
- Actor mailbox publication proves enqueue only.
- Reliable external effects require committed-log delivery plus an explicitly
  named receipt/checkpoint boundary.
- Actor memory and service caches are disposable accelerators.

### Independent modularity

- Domain crates import Mnesis kernel contracts only.
- `mnesis-bombay-core` imports no runtime, transport, executor, or concrete
  storage adapter.
- Durability execution imports no Actorpass type.
- `mnesis-actorpass` is removable without changing domain commands or aggregate
  decisions.
- Tower, HTTP, CLI, NATS, and Zenoh are outer adapters.
- A second host must use an abstraction before that abstraction is promoted to
  the core public API.

### Testability

- Domain decisions run synchronously without a runtime.
- Application execution runs against in-memory and fault-injecting repositories
  without Actorpass.
- Pure Behavior transcripts can assert command-to-request mapping without I/O.
- The Actorpass route can be tested with a deterministic fake service.
- Readiness can be held pending to test mailbox backpressure.
- Conflict, cancellation, panic, lost reply, duplicate delivery, and poison
  events are independently injectable.
- End-to-end tests use actual `System::spawn`, `ActorRef`, Mnesis repository,
  and current sibling source revisions.

### Flexibility

The same application execution semantics must be usable by:

- an Actorpass actor;
- a direct Tokio function;
- HTTP or gRPC;
- CLI and batch processing;
- NATS or Zenoh ingress;
- tests and simulations.

Supporting another host must not require copying domain decision or retry
semantics. It may require a transport-specific mapping and acknowledgement
policy.

### Performance

Performance comparisons must name their acknowledgement boundary. Pipelined
mailbox publication is not comparable to a durable request/reply round-trip.
Measure:

- cold load and hot cached execution;
- pipeline throughput and durable response latency;
- p50, p95, and p99;
- allocations per command;
- bytes retained per idle aggregate;
- actor-per-key and key-affine shard cardinality;
- conflict rates and concurrent writers;
- mailbox saturation and service readiness;
- in-memory floor and real durable adapters.

## Chosen layer boundaries

### 1. Domain layer

Owns aggregate, state, command, event, decision error, and business idempotency.
It does not know command transport, actor identity, repository implementation,
or retries.

### 2. Runtime-neutral application protocol

Owns:

- aggregate ID;
- command ID;
- typed command payload;
- typed durable outcome vocabulary.

It does not own reply channels, actors, middleware frameworks, or storage
connections.

### 3. Durability execution

Owns:

- repository selection;
- load and cache-miss reconstruction;
- `CommandRepository::execute`;
- conflict classification;
- bounded reload/re-decision policy;
- command-inbox transaction where available;
- cache invalidation after uncertainty;
- returned committed position.

The first implementation may be application-specific and generic. We will not
introduce a new handler trait that merely duplicates `CommandRepository`.

### 4. Actorpass adapter

Owns:

- mapping aggregate identity to actor address or shard;
- mailbox admission and sequential turns;
- conversion from actor command protocol to `ExecuteRequest`;
- service readiness and backpressure integration;
- typed reply capability;
- lifecycle, shutdown, and cache eviction hooks;
- tracing actor address/incarnation alongside domain identities.

It does not own aggregate decision semantics or durable event authority.

### 5. Optional Tower adapter

Tower `Service<Request>` is an existing Rust-native asynchronous
request/response abstraction with `poll_ready` backpressure. `Layer` supports
protocol-independent middleware. A monomorphized `service_fn` benchmark shows
no measurable overhead over the bespoke typed router in the current in-memory
floor test.

Tower remains optional because:

- it is a `std` ecosystem choice;
- readiness ownership and clone behavior must be respected;
- generic retry middleware is unsafe for non-idempotent commands;
- Mnesis already owns the durable execution primitive;
- non-Tower users should not pay the dependency or conceptual cost.

The Actorpass adapter should accept a statically typed service without requiring
dynamic dispatch. A Tower implementation can satisfy that seat.

### 6. Committed-event relay

Owns:

- `$all` subscription position;
- decoding and typed routing;
- bounded delivery attempts;
- poison quarantine/halt/recorded-skip policy;
- application receipt;
- checkpoint persistence;
- optional consumer inbox.

Checkpoint modes are explicit:

- after enqueue: may lose processing after mailbox loss;
- after receipt: at-least-once handling;
- inbox protected: effectively once at the named inbox/effect transaction.

The default is after receipt. No mode is called globally exactly-once.

## Command sequence

```text
caller
  │ CommandRequest(aggregate_id, command_id, command, reply)
  ▼
Actorpass mailbox
  │ sequential Behavior turn
  ▼
pure Behavior
  │ ExecuteRequest in typed Sends algebra
  ▼
DeliveryRouter / application Service
  │ await readiness
  │ locate key-affine cache slot
  │ load on cache miss
  │ decide + append through CommandRepository
  │ invalidate on uncertain failure
  ▼
typed CommandOutcome
  │ reply after durable fact
  ▼
caller
```

Actorpass awaits `DeliveryRouter::deliver` while interpreting the turn's sends.
It does not consume the next mailbox event until routing returns. This gives
per-actor command serialization without placing repository I/O in Behavior.

## Cache and sharding

A single global mutex is rejected because unrelated aggregates would
head-of-line block. The initial probe uses 64 key-affine mutex shards.

Production cache requirements:

- configurable shard count;
- stable hash from aggregate ID, not actor incarnation;
- one mutable root owner per key in a process;
- bounded entries or explicit unbounded opt-in;
- idle eviction;
- reload after eviction, conflict, panic, or ambiguous cancellation;
- metrics for hits, misses, entries, retained bytes, contention, and eviction;
- no lock held across unrelated-key work;
- no assumption that local affinity prevents another process writing.

Store optimistic concurrency remains the distributed correctness guard.

## Backpressure and readiness

There are two bounded resources:

1. the Actorpass mailbox;
2. the durability service/shard.

The adapter must not drain the mailbox into an unbounded internal queue. If the
selected key/shard cannot accept work, service readiness remains pending and
the current Actorpass turn remains in interpretation. This naturally stops that
actor from consuming more commands. Cross-key fairness must be measured: global
readiness over all shards can introduce head-of-line blocking, as documented by
Tower `Steer`.

Timeout is not cancellation safety. If an append future is dropped when commit
status is unknown, the cached root is invalidated and the caller receives an
ambiguous outcome unless a durable inbox resolves the command ID.

## Error model

Do not flatten errors into strings or one boxed error. Preserve:

- domain rejection;
- retryable optimistic conflict;
- retry exhaustion with attempt count;
- non-conflict storage failure;
- mailbox closed;
- service unavailable/not ready;
- ambiguous completion;
- actor termination/panic;
- reply receiver cancellation.

Transport adapters may serialize these errors but cannot erase their semantic
distinctions internally.

## Retry policy

No blanket Tower or runtime retry is enabled.

Conflict retry is eligible only when:

- the store classifies the failure as a conflict;
- the retry budget remains;
- the aggregate is reloaded;
- the command is re-decided against new state;
- the command contract permits replay.

Transport timeout, lost reply, and actor restart are not sufficient evidence
that the command did not commit. Transparent retry across those boundaries
requires a durable command ID/inbox or explicit domain idempotency.

## Alternatives considered

### Repository-owning Behavior

Mechanically valid and proven end to end. Rejected as the leading architecture
because it couples intended-pure Behavior code to repository types, cache
recovery, conflicts, and I/O. It remains a useful performance and complexity
baseline.

### Direct external execution only

Maximum runtime independence and simplest command path. Retained as a required
host and dissenting architecture. It does not provide Actorpass affinity,
mailbox admission, or supervision when those are product requirements.

### Expose Actorpass Environment and RuntimeEffects

Insufficient. Visibility alone does not define load, persistence intent,
append-before-send ordering, reply recovery, or checkpoint semantics.

### Add a persistence effect to Behaviorpass/Actorpass

Architecturally clean and similar to persistent-actor systems. Deferred because
it changes two foundational crates and their public algebra before the adapter
has proven exact semantics. The typed service route provides an executable
current-API experiment first.

### Persistence actor

Moves I/O to another actor but requires a request/ack state machine and
stashing while persistence is pending. It adds mailbox and lifecycle boundaries
without improving the durable transaction. Rejected unless persistence itself
needs independent actor lifecycle.

### Generic MediatR-style dispatcher

Rejected. Mnesis already has typed command decisions and durable execution.
Dynamic registration, universal envelopes, or reflection-like lookup would
weaken static guarantees without a demonstrated consumer.

### Tower as mandatory core

Rejected. Tower is valuable interoperability, not domain authority. It remains
an optional adapter until multiple hosts prove it belongs in the public surface.

### Historical Bombay runtime

Rejected as a target because Actorpass replaces it.

## Consequences

### Positive

- Behavior purity is preserved.
- Mnesis and Actorpass remain independently usable.
- Direct and Actorpass hosts share execution semantics.
- Tower/HTTP interoperability can be added without infecting core crates.
- Static typing and monomorphization remain available end to end.
- Backpressure has an explicit readiness seat.
- Repository-owning actors remain available as a comparison, not dogma.

### Negative

- Key-affine cache/shard machinery is non-trivial.
- Typed service routing introduces another generic boundary and diagnostics to
  maintain.
- Durable command inbox support may require store-specific transaction work.
- Actor supervision cannot undo a committed append.
- Application replies require an additional async hop.
- Effectful event consumers still require a separate committed-log relay.

### Risks

- A shared shard mutex can cause cross-key contention.
- Service cloning can detach `poll_ready` from the instance receiving `call`.
- Generic middleware might retry unsafe commands.
- Cache eviction might race active execution if ownership is not explicit.
- A transport might mislabel enqueue as durable acceptance.
- A relay might checkpoint before its promised effect boundary.

## Validation evidence so far

The moved comparative benchmark uses current sibling source checkouts and proves:

- direct Mnesis execution;
- repository-owning Actorpass Behavior execution;
- pure Behavior plus custom service route;
- pure Behavior plus monomorphized Tower service route;
- actual `System::spawn` and `ActorRef::send`;
- durable reload after execution;
- 1, 64, and 1,024 actor identities through a 64-shard cache.

The initial four-worker in-memory medians for 10,000 hot commands were:

| Path | Pipelined ns/op | Durable round-trip ns/op |
|---|---:|---:|
| direct Mnesis | 605 | 605 |
| repository-owning Behavior | 801 | 8,594 |
| pure Behavior + custom route | 523 | 7,772 |
| pure Behavior + Tower route | 506 | 7,812 |

These are overhead-floor measurements, not production claims.

## Mandatory gates before acceptance

- Preserve concrete domain and store error types across every candidate.
- Inject conflict, append failure, append-success/lost-reply, cancellation, and
  panic at deterministic boundaries.
- Implement command inbox behavior or explicitly prohibit transparent retries.
- Implement relay receipt, checkpoint, poison, retry, and consumer inbox tests.
- Test Tower readiness without cloning the ready instance incorrectly.
- Saturate mailboxes and shards; demonstrate bounded memory.
- Measure allocations and idle retained bytes.
- Compare actor-per-key against multiple shard counts and aggregate cardinality.
- Benchmark at least one real durable adapter.
- Record p50/p95/p99 under concurrent producers.
- Compile the core crate for a no-std target.
- Prove Actorpass/Tower are absent from the core feature graph.
- Add compile-fail tests for mixed identity/protocol types.
- Run an independent red-team review.
- Publish Actorpass or expose a remotely fetchable pinned revision so the full
  workspace and Nix checks are hermetic rather than sibling-path dependent.

Until these gates pass, this ADR remains Proposed.
