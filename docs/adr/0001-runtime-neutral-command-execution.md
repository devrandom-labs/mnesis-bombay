# ADR 0001: Runtime-neutral Mnesis execution with Bombay entity hosting

- Status: Accepted boundary; production implementation gated by the capability
  audit
- Date: 2026-08-09; amended 2026-08-14 for the released Entity runtime,
  current Bombay contracts, and the actor-native CQRS/ES application topology
- Decision owners: Mnesis and Bombay maintainers
- Scope: command execution, runtime integration, replies, event propagation,
  modularity, testability, and performance

## Decision summary

`mnesis-bombay` is a separate integration repository. Mnesis and Bombay do
not depend on each other.

The leading architecture is:

1. Mnesis remains the domain and durability authority.
2. A small runtime-neutral core carries command identity and typed outcomes.
3. Bombay Behaviors remain pure. A command Behavior emits a typed
   `ExecuteRequest` in its `Sends` algebra.
4. Bombay and its focused Bombay sibling libraries own the reusable runtime.
   `bombay-entity` owns stable local entity identity, bounded on-demand
   activation, admission, routing, passivation, and draining; Bombay owns
   exact-incarnation execution, typed delivery, lifecycle interpretation, and
   supervision. Location-transparent remote placement is a later, separate
   capability.
5. One disposable actor activation hosts each active aggregate instance. Its
   root is a rebuildable cache; the Mnesis stream remains authoritative.
6. Tower interoperability is optional. Tower is not part of the domain or core
   contract.
7. Reliable event propagation reads the committed Mnesis log and uses explicit
   receipts, checkpoints, retries, poison handling, and optional consumer
   inboxes. Immediate Bombay sends after append are not a reliable outbox.
8. No generic mediator, dynamic message bus, global registry, `Any`, `TypeId`,
   or erased universal envelope is introduced.
9. Bombay is the default operational assembly, not merely a command adapter.
   Aggregate instances, projection partitions, saga/process-manager instances,
   committed-log relay partitions, and external-effect delivery partitions are
   independently supervised actor roles with distinct typed protocols.
10. Pure decisions and durable primitives do not become actors. Actors own
    identity, activation, serialized turns, bounded concurrency, timers and
    lifecycle; Mnesis owns streams, append facts, positions, checkpoints, saga
    state/intents, and durable inbox/outbox records.

This decision is provisional until the failure, readiness, allocation,
durable-store, and high-cardinality performance gates below pass.

The complete production capability inventory, repository ownership decisions,
upstream requirements, activation state machine and release gates are normative
in [`../production-readiness-research.md`](../production-readiness-research.md).
This ADR alone must not be interpreted as a production-ready implementation.

## Naming

The current `bombay-rs` core replaces the historical Bombay implementation.
This repository does not support two competing actor runtimes. The
`mnesis-bombay` name refers to the Bombay family of focused libraries.
Runtime-specific production code belongs in the `mnesis-bombay` crate.

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
or error. The synchronous fold keeps I/O outside the Behavior transition.

The current Bombay runtime family supplies:

- Behavior's pure typed folds, semantic `SendProduct` composition, typed
  recipients, shutdown/deadline protocols, and replacement-resolution facts;
- Communication's sole two-lane mailbox, bounded user admission, per-lane
  FIFO, closure, and rejected-payload recovery;
- Address's exact registration identities and generation fencing;
- Observe's retained terminal publication for one exact generation;
- Timers' keyed, queue-branded, generation-safe monotonic schedules;
- Entity's stable local identity, single-flight activation, bounded waiters,
  typed refusal, exact-incarnation routing, processing fence, draining, and
  passivation;
- Bombay's exact-incarnation spawn/delivery, effect interpretation,
  supervision, lifecycle reporting, and application-facing composition.

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
- Mnesis durability primitives and the runtime-neutral protocol import no
  Bombay type. The Bombay-hosted application adapter may compose both.
- `mnesis-bombay` is removable without changing domain commands or aggregate
  decisions.
- Tower, HTTP, CLI, NATS, and Zenoh are outer adapters.
- A second host must use an abstraction before that abstraction is promoted to
  the core public API.

### Testability

- Domain decisions run synchronously without a runtime.
- Application execution runs against in-memory and fault-injecting repositories
  without Bombay.
- Pure Behavior transcripts can assert command-to-request mapping without I/O.
- The Bombay route can be tested with a deterministic fake service.
- Readiness can be held pending to test mailbox backpressure.
- Conflict, cancellation, panic, lost reply, duplicate delivery, and poison
  events are independently injectable.
- End-to-end tests use actual `System::spawn`, `ActorRef`, Mnesis repositories,
  and the locked released dependency graph.

### Flexibility

The same application execution semantics must be usable by:

- a Bombay actor;
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

- preservation of the Mnesis-owned `A::Id` and `A: Handle<C, N>` relationship;
- application-owned command, causation and correlation identities in distinct
  generic roles;
- a typed command payload for an already-selected aggregate instance;
- validated, bounded and redacted application context;
- direct-host addressing as `Addressed<A::Id, CommandRequest<..>>`;
- typed durable outcome vocabulary.

It does not own reply channels, actors, middleware frameworks, or storage
connections.

### 3. Mnesis command execution

Owns:

- repository selection;
- load and cache-miss reconstruction;
- `CommandRepository::execute`;
- conflict classification;
- bounded reload/re-decision policy;
- command-inbox transaction where available;
- cache invalidation after uncertainty;
- returned committed position.

This policy is packaged as `mnesis-bombay-execution`, which depends on
`mnesis-bombay-core` and `mnesis-store` but not Bombay, Tokio, Tower, a
transport, or a concrete store. A transparent repository decorator observes
the exact `save` call made by Mnesis's own `CommandRepository::execute`, so the
adapter can distinguish pure decision from append-in-flight without copying
Mnesis's decide-then-save implementation.

The direct implementation remains usable without Bombay. The Bombay host
may place the same Mnesis operations inside an entity activation, but it must
not duplicate generally useful activation, routing, passivation, admission, or
draining machinery in this repository.

### 4. Bombay runtime facilities

Bombay and focused Bombay sibling crates own:

- stable typed local entity lookup by `bombay_entity::EntityId`;
- bounded, race-free local activation and passivation in `bombay-entity`;
- mailbox admission in Communication/Bombay and entity admission in Entity;
- concurrency across unrelated entities;
- typed request/reply and timeout composition;
- lifecycle, supervision, overload, draining, and shutdown mechanics;
- tracing actor address/incarnation alongside domain identities.

The `mnesis-bombay` adapter supplies the opaque hydration factory and Mnesis
command behavior. Bombay does not own aggregate decisions, Mnesis conflict
policy, durable outcomes, or event authority.

For local hosting, the adapter is the sole translation from an application
`A::Id` into `bombay_entity::EntityId<A::Id>`. It then delivers the command
payload unchanged. The payload does not repeat the aggregate ID, preventing a
routed Entity identity and an embedded application identity from disagreeing.

The local binding uses Bombay #307's `System::activate`. Entity commits an
activation only after this actor-runtime port reports successful transactional
activation; Bombay completes the Behavior initialization fold before
registering the endpoint and returns separately nameable cloneable delivery
and affine retirement capabilities. Ordinary actors that do not require this
visibility law continue to use `System::spawn`.

`bombay-entity` already composes `bombay-transition` and
`bombay-machine-executor` for its deterministic lifecycle and serialized turn
policies. The adapter consumes Entity's resulting typed facts rather than
building a second lifecycle machine. Mnesis load/append is external async I/O,
so a machine-executor turn receipt cannot serve as durable-command evidence;
only the Mnesis repository result advances the command durability phase.

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

The Bombay adapter should accept a statically typed service without requiring
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

### 7. Actor-native CQRS/ES topology

The default Bombay-facing application is a statically composed actor topology,
not a collection of unrelated async services:

```text
typed ingress
    │
    ▼
aggregate Entity<A::Id> ──append──▶ Mnesis committed log
                                         │
                         supervised relay partition actors
                              ┌──────────┼──────────┐
                              ▼          ▼          ▼
                     projection     saga Entity    effect-delivery
                     partition      <Saga::Id>     partition actor
                       actor           actor
                              │          │          │
                              └──── durable receipts/checkpoints ────┘
```

- One active aggregate Entity actor owns the disposable `AggregateRoot<A>`
  cache for one application-selected aggregate identity.
- A projection worker actor owns one explicitly bounded partition and advances
  its Mnesis checkpoint only at the projection's declared durable boundary.
- A saga/process-manager Entity actor owns one active saga identity. Mnesis
  persists its state, consumed position, and projected intents; durable
  schedules may reactivate it, while an in-memory timer alone is not workflow
  durability.
- Relay and effect-delivery actors own bounded intake, retry scheduling,
  poison policy, supervision, and drain. Their checkpoints and inbox/effect
  records remain durable Mnesis or application-store facts.
- A typed read-your-writes waiter coordinates a command's committed position
  with a projection checkpoint. It does not turn a mailbox acknowledgement
  into a consistency guarantee.
- One typed composition root installs these roles and their supervision and
  shutdown relationships. It does not use a service locator, dynamic handler
  registry, `Any`, `TypeId`, or a universal envelope.

Direct execution remains a supported secondary adapter and test seam. It is
not the default product topology. Pure `Handle` decisions, event application,
codecs, validation, checkpoint arithmetic, and repository/store primitives
remain ordinary deterministic or durable components; actorizing them would
add message boundaries without adding an ownership or concurrency invariant.

## Command sequence

```text
caller
  │ Addressed<A::Id, CommandRequest<A, C, ..>>
  ▼
Bombay Entity local route
  │ adapter consumes A::Id into EntityId<A::Id>
  │ locate or activate aggregate entity by stable key
  │ hydrate from Mnesis before readiness
  ▼
aggregate activation mailbox
  │ one sequential command turn for this entity
  ▼
pure command Behavior
  │ validate and emit ExecuteRequest<CommandRequest, typed reply recipient>
  ▼
activation-owned application interpreter
  │ decide + append through CommandRepository
  │ retire/poison activation on uncertain failure
  ▼
typed CommandOutcome
  │ reply after durable fact
  ▼
caller
```

The entity activation does not consume the next command until the current
durable turn reaches a factual outcome. Different entity activations progress
concurrently. The Behavior remains pure; an application interpreter attached
to the activation owns Mnesis I/O and keeps domain decision separately
testable.

No executor trait is introduced in core. Mnesis already supplies the two
relevant capabilities: `Handle<C, N>` for pure decision and
`CommandRepository<A>::execute` for decide then durable save. Direct and Bombay
hosts are compositions around that concrete port, not two implementations of a
new general executor abstraction. Reconsider only after a second real
execution implementation proves a smaller missing variability boundary.

## Activation and passivation

A fixed shard scheduler in this integration is rejected as the default because
it duplicates a general actor-runtime capability and can head-of-line block
unrelated entities. `bombay-entity` provides one logical local activation per
active entity ID, bounded activation waiters and explicit passivation. A
global active-entity capacity remains a separately verified policy seam; it
must not be inferred from the per-activation waiter bound. The reusable bound
is assigned to `devrandom-labs/bombay-entity#6`.

The released local Entity runtime already owns race-free get-or-activate,
passivation with an ordered processing fence, concurrency across unrelated
entities, typed admission refusal, and graceful/forced draining. Global active
entity capacity and lifecycle facts remain explicit upstream seams, assigned to
`devrandom-labs/bombay-entity#6` and `#7`. This adapter supplies
hydrate-on-activation and discards or retires an activation after conflict,
panic, or ambiguous cancellation.

Store optimistic concurrency remains the distributed correctness guard.

## Backpressure and readiness

There are two principal bounded resources:

1. the Bombay mailbox;
2. active entity capacity and the Mnesis store.

The runtime must not drain a mailbox into an unbounded internal queue. Entity
activation admission, per-entity mailbox admission, and store pressure remain
distinct observable boundaries. Communication owns mailbox pressure; Entity
owns activation waiters, reservation closure, fences, and drain facts;
Bombay owns exact-incarnation delivery. The adapter maps those facts and
store readiness into the core factual outcome vocabulary.

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
host and dissenting architecture. It does not provide Bombay affinity,
mailbox admission, or supervision when those are product requirements.

### Expose Bombay Environment and RuntimeEffects

Insufficient. Visibility alone does not define load, persistence intent,
append-before-send ordering, reply recovery, or checkpoint semantics.

### Persistence actor selected as the Bombay host

Selected for the Bombay topology. One disposable activation per active
aggregate gives per-entity sequential turns and concurrency across unrelated
entities. Generic local route/create/passivate mechanics belong in
`bombay-entity`;
Mnesis hydration, decision, append, and factual outcomes remain in an
application interpreter attached to the activation, outside the pure Behavior.
The mailbox and supervision do not improve the durable transaction or make an
interrupted command retryable.

### Fixed keyed scheduler in the integration

Rejected as the default. It would duplicate reusable activation, routing,
passivation, admission, and draining facilities that belong in the Bombay
actor-runtime family. A measured worker-pool implementation may remain an
upstream runtime topology option, but mnesis-bombay does not own its machinery.

### Generic MediatR-style dispatcher

Rejected. Mnesis already has typed command decisions and durable execution.
Dynamic registration, universal envelopes, or reflection-like lookup would
weaken static guarantees without a demonstrated consumer.

### Tower as mandatory core

Rejected. Tower is valuable interoperability, not domain authority. It remains
an optional adapter until multiple hosts prove it belongs in the public surface.

### Historical Bombay runtime

Rejected as a target because Bombay replaces it.

## Consequences

### Positive

- Domain decision purity is preserved.
- Mnesis and Bombay remain independently usable.
- Direct and Bombay hosts share execution semantics.
- Tower/HTTP interoperability can be added without infecting core crates.
- Static typing and monomorphization remain available end to end.
- Backpressure has an explicit readiness seat.
- Repository-owning actors remain available as a comparison, not dogma.
- The default application surface makes actor identity, supervision,
  concurrency, timers, passivation and bounded queues visible across the full
  CQRS/ES lifecycle, not only aggregate command handling.

### Negative

- The integration depends on upstream entity activation and passivation
  capabilities rather than carrying a private scheduler fallback.
- Durable command inbox support may require store-specific transaction work.
- Actor supervision cannot undo a committed append.
- Application replies require an additional async hop.
- Effectful event consumers still require a separate committed-log relay.
- More actor roles require explicit partitioning, supervision ownership,
  capacity budgets and shutdown ordering in the public composition API.

### Risks

- Activation/passivation races can lose routing continuity unless the upstream
  entity directory buffers or rejects commands with explicit facts.
- Generic middleware might retry unsafe commands.
- Cache eviction might race active execution if ownership is not explicit.
- A transport might mislabel enqueue as durable acceptance.
- A relay might checkpoint before its promised effect boundary.

## Validation evidence so far

The comparative benchmark now uses the current pure Behavior contract and
exercises:

- direct Mnesis execution;
- pure Behavior plus custom service route;
- pure Behavior plus monomorphized Tower service route;
- actual `System::spawn` and `ActorRef::send`;
- durable reload after execution;
- 1, 64, and 1,024 actor identities through a 64-shard cache.

The initial 2026-08-09 four-worker in-memory medians for 10,000 hot commands
are retained as historical experiment evidence:

| Path | Pipelined ns/op | Durable round-trip ns/op |
|---|---:|---:|
| direct Mnesis | 605 | 605 |
| repository-owning Behavior (historical Behavior 0.8 experiment) | 801 | 8,594 |
| pure Behavior + custom route | 523 | 7,772 |
| pure Behavior + Tower route | 506 | 7,812 |

These are overhead-floor measurements, not current 0.9 benchmark results or
production claims. Repository-owning Behavior is no longer compiled because
the current Behavior algebra is a synchronous pure fold; Mnesis I/O belongs in
the application interpreter.

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
- Prove Bombay/Tower are absent from the core feature graph.
- Add compile-fail tests for mixed identity/protocol types.
- Run an independent red-team review.
- Keep the standalone registry graph and Nix evaluation hermetic.

Until these gates pass, this accepted architecture is not production-ready.
