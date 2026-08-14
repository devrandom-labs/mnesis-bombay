# Production-readiness architecture audit

Status: normative research and implementation backlog

Date: 2026-08-09; dependency contracts reconciled 2026-08-13

Scope: a correct, operable Mnesis command/event integration hosted by Bombay

This document closes the gap between an adapter seam and a production system.
It inventories the whole system, assigns each invariant to one repository, and
identifies the upstream work that cannot honestly be hidden in an adapter.

## Final boundary decision

The integration is five cooperating systems, not one command handler:

```text
ingress adapters (HTTP, CLI, NATS, Zenoh)
                  │ typed command + deadline + identity
                  ▼
runtime-neutral application executor
                  │ load → decide → append → durable outcome
          ┌───────┴────────┐
          │                │
 direct host       Bombay host adapter
                           │ Bombay Entity local route
                           │ bounded activation/passivation
                           ▼
                     Mnesis event store
                           │ committed $all subscription
                           ▼
                  checkpointed event relay
                           │ receipt/inbox policy
                           ▼
                    event consumers/effects
```

The compiler selects the aggregate implementation from the typed endpoint.
The request selects the aggregate instance with `A::Id`. The host maps a
stable `ExecutionKey<A>` to a shard. No actor lookup determines domain type,
and no `Any`/`TypeId` registry searches for a handler.

Local production operation is the first supported deployment. Clustered
single-writer placement is a distinct later product: Bombay currently has
no distributed directory, membership, fencing lease, rebalance protocol, or
remote mailbox guarantee. A local adapter must never imply those guarantees.

## Ownership rules

| Repository/layer | Owns | Must not own |
|---|---|---|
| application/domain | aggregate, command, event, rejection, business idempotency and authorization facts | actors, storage adapters, transport acknowledgements |
| Nexus/Mnesis | event-stream correctness, append conflict, committed positions, subscriptions, atomic durable records | actor lifecycle, retry timing, HTTP/NATS policy |
| Bombay Behavior | pure event-to-actions algebra and closed typed protocols | I/O, repositories, queues, durability, restart implementation |
| Bombay runtime family | Behavior algebra; Communication mailbox; Address registration fencing; Observe exact-generation completion; Timers scheduling; Entity local activation/admission/draining/passivation; Bombay exact-incarnation execution and interpretation | aggregate semantics, durable acceptance, Mnesis conflict policy |
| `mnesis-bombay-core` | runtime-neutral request/outcome vocabulary and command-phase facts | Tokio, Tower, Bombay, concrete store |
| `mnesis-bombay` | Mnesis hydration factory and activation-owned application interpreter, outcome mapping, committed-log relay composition | generic entity directory, activation scheduler, passivation, runtime admission/drain mechanics |
| optional Tower crate | `Service` interoperability and middleware mapping | semantic retry defaults or durable truth |
| ingress/egress adapter | serialization, authentication evidence, protocol acknowledgements | changing internal failure meaning |

## Capability and gap matrix

Legend: **existing** means the required primitive is present, not that the
end-to-end policy is complete. **add** is required production work.

| Capability | Required invariant | Existing evidence | Owner and action |
|---|---|---|---|
| aggregate selection | command type maps statically to exactly one aggregate type | Mnesis `Handle<C>` and aggregate traits | **compose Mnesis:** typed endpoint; no registry or duplicate handler abstraction |
| instance selection | direct routing carries exactly `A::Id`; an Entity-hosted payload cannot carry a second disagreeing aggregate ID | Mnesis `Aggregate::Id`; Entity `EntityId<T>` | **compose:** `Addressed<A::Id, CommandRequest<A, C, ..>>` for direct hosts; the Bombay adapter alone maps it to `EntityId<A::Id>` plus the unchanged payload |
| stable execution key | application ID, Entity stable identity, activation generation and actor incarnation remain distinct | generic Mnesis IDs and typed Entity/Bombay identities exist | **compose existing types:** do not replace them with integration-owned ID representations |
| startup registration | host explicitly installs each supported aggregate family | `bombay-entity` 0.1.0 `EntityDefinition`/`LocalEntityRuntime` seats | **adapter:** typed Mnesis definition and Bombay runtime binding |
| activation | first command lazily reconstructs a missing root | Mnesis `Repository::load`; Entity single-flight activation and opaque preparation | **compose existing:** prepare from Mnesis, activate through Entity/Bombay |
| passivation | idle roots are evicted without losing truth | Entity exact-incarnation drain fence and passivation | **compose existing:** disposable Mnesis activation; separately verify idle/global-capacity policy |
| per-key ordering | one process never executes two commands for one key concurrently | actor turn serialization exists | **upstream:** one activation/turn stream per entity key |
| cross-key concurrency | slow key does not block unrelated keys | independent actors provide the intended topology | **upstream:** bounded independent activations; benchmark shared-resource contention |
| hot-key fairness | one key cannot monopolize shared runtime/store capacity | generic runtime concern | **upstream:** dispatcher/admission fairness and starvation telemetry |
| durable execution | success means append confirmed | Mnesis `CommandRepository::execute` | **use existing**, wrap with typed outcome |
| conflict recovery | discard root, reload, re-decide, bounded retry | Mnesis exposes conflict predicate, policy consumer-owned | **add core/adapter:** explicit replay eligibility and retry budget |
| command deduplication | same command ID cannot apply twice when outcome is uncertain | no command inbox in Nexus | **propose Nexus:** atomic command-inbox record plus event append capability |
| lost reply | never translate unknown commit into safe retry | no durable inbox | **add core:** `AmbiguousCompletion`; inbox later resolves it |
| cancellation | dropping a future after commit begins invalidates cached state | Tokio/Bombay cancellation exists; no integration policy | **adapter:** commit-phase guard; **upstream:** retire activation mechanism |
| panic recovery | panic retires incarnation and never reuses uncertain root | Bombay classifies `TaskOutcome::Panicked` and supports restart | **compose:** restart/revive empty activation; fail/resolve outstanding receipts |
| restart budget | crash loops terminate or degrade predictably | Behaviorpass supervision budget/restart strategy exists | **use existing**, configure and observe it |
| child ownership | supervisor retirement shuts down owned workers | Bombay child leases/scopes and guardian mechanics exist | **use existing**, contract-test ordering |
| mailbox pressure | user admission is bounded | Bombay Communication/Bombay bounded user lane | **use existing**, never bypass with unbounded queue |
| execution pressure | store and activation capacity propagate to callers | Entity bounds per-activation waiters and returns typed refusals; mailbox is bounded | **adapter:** map distinct Entity, mailbox, and store facts; do not infer a global entity bound |
| overload policy | wait, reject, and deadline have distinct typed outcomes | Bombay L3 is feature-complete; Entity exposes typed refusal | **compose existing:** preserve boundary-specific facts and command ownership |
| readiness | capacity reservation is consumed or released correctly | Tower has an optional interoperability contract | **upstream:** actor admission law; **Tower adapter:** faithful mapping |
| deadlines | deadline covers queue and execution but does not lie about commit | timeout primitives exist, semantic phase absent | **add core:** absolute deadline and phase-aware outcome |
| graceful shutdown | stop ingress → drain/reject → settle commits → checkpoint → stop children | Bombay L4 is feature-complete; Entity owns reservation/fence/retirement drain facts | **adapter:** compose Mnesis/relay phase ordering and map forced-drain ambiguity honestly |
| immediate command reply | reply only after durable fact | Bombay F3 typed recipient/timeout composition is feature-complete | **adapter:** send only a factual Mnesis outcome through the existing typed recipient |
| committed event source | reliable notifications originate after append | Mnesis `$all` subscription and positions | **use existing** |
| relay checkpoint | resume strictly after last acknowledged policy boundary | positions exist; generic consumer runner intentionally absent | **add adapter relay host** |
| checkpoint storage | checkpoint survives restart atomically with relay state | Mnesis `SnapshotStore<S, P>` can store state+position | **use existing** for relay state |
| consumer receipt | enqueue, fold, effect, and inbox commit are different facts | Bombay send acknowledges publication only | **add adapter protocol:** typed receipt level |
| consumer dedup | duplicate relay delivery cannot repeat protected effect | no general consumer inbox | **application storage or propose Nexus optional inbox capability** |
| poison event | checkpoint never silently skips failed decode/handling | subscriptions surface errors | **add relay:** halt/quarantine/recorded-skip policy |
| retry scheduling | bounded attempts, jitter and backoff; no mailbox-blocking sleep | Bombay L5 is feature-complete over keyed generation-safe Bombay Timers | **compose existing timers in adapter**; never reinterpret an uncertain commit as retryable |
| projection lag | expose head, checkpoint and distance/age | positions exist but distance may be adapter-specific | **add telemetry adapter**, never assume dense sequence |
| read-your-writes | caller can await projection reaching append position | Mnesis append returns last committed position | **add optional consistency waiter** with timeout/ambiguity |
| snapshots | optimization failure never changes correctness | Mnesis snapshot decorator is best effort | **configure application**, expose hit/stale/failure metrics |
| schema evolution | old events remain decodable or explicitly upcast | schema version/envelope support exists; policy app-owned | **application:** versioned codecs/upcasters and replay test corpus |
| multi-aggregate work | no implied transaction across streams | Mnesis has `AtomicAppend` capability for supported stores | **application service:** require capability explicitly or use saga |
| sagas/process managers | durable state and intents resume after crash | Mnesis saga/projected-intent primitives exist; runner consumer-owned | **add separate host**, not command executor magic |
| external effects | no dual-write claim | committed-log relay available | **relay/outbox consumer** with named inbox/effect boundary |
| local discovery | address resolves exact incarnation and retirement | Bombay Address/Observe integration | **use existing** |
| cluster placement | one active owner or safe concurrent writers | absent; optimistic concurrency only catches conflicting append | **new future distributed adapter/runtime**, not local v1 |
| membership/rebalance | moves ownership without loss or split brain | absent | **future distributed project** |
| fencing | stale owner cannot write after lease loss | absent from generic store contract | **future store/coordination capability** |
| remote delivery | protocol defines authentication, dedup, order and ack | deliberately downstream of KERI/Zenoh in Bombay roadmap | **separate integration repositories** |
| authentication | ingress proves caller identity | outside all current core crates | **transport/application** |
| authorization | decision uses authenticated principal/tenant facts | metadata can carry bytes but policy absent | **application decorator/domain**, typed context |
| tenant isolation | routing, cache, stream and metrics include tenant safely | absent as universal policy | **application/core key type**, deployment quotas |
| secrets/encryption | keys rotate; logs and snapshots protect sensitive data | signed-event example exists, encryption policy absent | **store/deployment adapter**, never Bombay Behavior |
| audit trail | command/causation/correlation metadata reaches committed events | Mnesis metadata provider hooks exist | **application decorator**, schema and redaction policy |
| observability | command ID, aggregate ID, actor address/incarnation and position correlate | tracing/lifecycle primitives exist separately | **add adapter spans/metrics**, bounded-cardinality labels |
| health | distinguish live, ready, degraded, relay-lagged and store-unavailable | absent integration endpoint | **add host health model** |
| configuration | capacities/retry/shutdown settings validated at startup | absent integration builder | **add adapter config** with no silent unlimited defaults |
| rolling deploy | old/new event and command protocols coexist safely | no integration contract | **document compatibility window**, versioned messages/codecs |
| disaster recovery | restore verifies ordering, checksums, positions and rebuilds projections | Mnesis export/import exists | **deployment runbook and restore drills** |
| retention/deletion | legal deletion does not corrupt replay guarantees | policy not universal | **application/store operations**, explicit tombstone/crypto-shred decision |
| deterministic tests | domain and Behavior work without runtime/I/O | Behavior testkit and Mnesis test domains exist | **add layered contract suites** |
| fault injection | every crash window has an oracle | current probe is performance-focused | **add adapter fault store/router/clock** |
| concurrency models | ordering, cancellation and wakeup races are explored | sibling projects use model/property/Miri tests | **add state model + loom where implementation permits** |
| compatibility tests | all supported Mnesis stores obey required capability subset | Nexus conformance kit exists | **add integration profile per store** |
| capacity tests | saturation remains bounded and fair | absent | **add soak/load tests and memory ceilings** |

## Required public protocol

The original `CommandRequest<I, C>` and `CommandOutcome<P, R, E>` were
placeholders. Reconciliation with Mnesis 0.2.2 and `bombay-entity` 0.1.0 makes
the aggregate/command and routing relationships structural rather than
duplicating them in an envelope:

```rust,ignore
struct CommandRequest<A, C, Identity, Context, Deadline, const N: usize = 0>
where
    A: Aggregate + Handle<C, N>,
{
    identity: Identity,
    command: C,
    context: Context,
    deadline: Option<Deadline>,
}

struct Addressed<Id, Message> { id: Id, message: Message }

enum CommandOutcome<P, O, R, C, S, A, CommandId> {
    Ignored { output: O },
    Committed { position: P, output: O },
    Rejected(R),
    ConflictExhausted { source: C, attempts: NonZeroU32 },
    Storage(S),
    Overloaded(A),
    DeadlineBeforeExecution,
    AmbiguousCompletion { command_id: CommandId },
    ShuttingDown,
}
```

Mnesis owns the application-selected `A::Id`, the `A: Handle<C, N>` pairing,
pure decision, and `CommandRepository::execute`. Core preserves those types. A
direct host selects an instance with
`Addressed<A::Id, CommandRequest<A, C, ..>>`; the Bombay adapter consumes that
address and creates `EntityId<A::Id>`, so an activation-bound command carries
no duplicate ID. Transport reply channels do not belong inside the core
request. A Bombay `ExecuteRequest<Request, Reply>` adds the typed reply
capability only at the runtime boundary.

Applications own the representation of command identity, causation,
correlation, authenticated principal evidence, tenant, trace context and
schema/protocol version. `CommandIdentity` preserves the first three in
distinct generic roles. `ValidatedContext<C, MAX_BYTES>` requires the
application's `Context` policy to measure, validate and redact the remaining
facts before a request can carry them.

## Entity activation and Mnesis execution state machines

The Bombay runtime owner must model each entity key independently:

```text
Absent → Activating → Ready → Executing → Ready → Passivating → Absent
   ▲          │                   │                 │
   └──────────┴──────Failed───────┴────Retired──────┘
```

- `Activating` coalesces concurrent local misses and runs the opaque hydration
  factory before publishing readiness.
- `Executing` preserves one turn at a time for an entity while unrelated
  entities remain independently schedulable.
- `Passivating` cannot race accepted or executing work into loss; new delivery
  is buffered within a bound or rejected with a typed fact.
- `Failed`/`Retired` never reuse uncertain application state.

Within `Executing`, mnesis-bombay separately models load/decide/append/reply
phases so an actor crash, cancellation, or timeout is classified by the last
durable fact. The runtime owns bounded activation and mailbox resources; the
adapter owns no hidden queue or cache.

## Failure and acknowledgement table

| Last proven phase | Safe statement | Automatic retry? | Cache action |
|---|---|---|---|
| mailbox rejected | command not admitted | caller policy | none |
| admitted, not started | not executed locally | only with same command ID/deadline | none |
| load failed | no decision/append by this attempt | classified storage retry | remove/absent |
| domain rejected | no append | no | retain trusted root |
| conflict confirmed | this append did not win | bounded reload/re-decide if replayable | discard |
| append failure confirmed pre-commit | not committed by this attempt | classified policy | discard conservatively |
| append success returned | committed at returned position | never re-execute without inbox lookup | update trusted root |
| future lost during commit window | unknown | **no blind retry** | poison/discard |
| reply lost after append | committed but caller may not know | inbox lookup, not re-execution | trusted locally; recovery reloads |
| actor panic | phase-dependent/possibly unknown | only through same rules | discard shard state |

Supervision restores service availability; it does not manufacture knowledge
about a storage transaction. Restart and retry are separate policies.

## Upstream change decisions

### Behaviorpass

No production integration feature should be added initially. In particular,
do not add repositories, persistence effects, aggregate IDs, retry policy,
backpressure counters, or dynamic handlers. Existing pure closed protocols,
supervision policy wrappers, stashing, receive timeout, shutdown and typed
service sends are sufficient inputs.

Reconsider only if the adapter proves a generally useful *pure action* that
has a second non-Mnesis interpreter. Persistence is not such an action yet.

### Bombay and focused Bombay runtime crates

Bombay is intended to be a best-in-class actor runtime, not a frozen set of
current primitives. The 2026-08-13 audit found that the local prerequisites
formerly listed here now have owners and executable implementations:

- Behavior owns pure folds, semantic `SendProduct` routing, typed recipients,
  deadline/shutdown wrappers, and replacement-resolution facts.
- Communication 0.1.1 owns the sole two-lane mailbox, bounded user-lane
  backpressure, per-lane FIFO, closure, and rejected-payload recovery.
- Address 0.1.1 owns exact registration identities and generation fencing;
  Observe 0.1.0 owns exact-generation terminal publication; Timers 0.1.0 owns
  keyed, queue-branded generation-safe scheduling.
- `bombay-entity` 0.1.0 owns stable local `EntityId`, single-flight activation,
  bounded activation waiters, typed refusal, exact-incarnation routing,
  processing fences, draining, retirement, and passivation.
- Bombay F3 and L3-L6 are feature-complete and supply typed reply/timeout,
  admission composition, graceful draining, retry scheduling, and reporting.

Bombay #268 and worker-pool L1 no longer block the local host. They concern a
future remote/location-transparent arm or an alternative topology and require
their own consumers and guarantees. Global active-entity capacity is assigned
to `devrandom-labs/bombay-entity#6`; generic lifecycle facts are assigned to
`devrandom-labs/bombay-entity#7`. Entity's per-activation waiter bound must not
be described as either one.

#### Bombay–Entity conformance audit — 2026-08-14

The exact public contracts expose five `bombay_entity::LocalEntityRuntime`
operations. Their ownership mapping is:

| Entity operation | Existing Bombay composition | Audit status |
|---|---|---|
| `spawn` | Tokio owns task execution; the Entity port requires every owned lifecycle task to be driven to completion | composable by the adapter, subject to its Tokio host contract |
| `activate` | Bombay owns mailbox construction, Behavior initialization/effect interpretation, address registration, launch, and exact terminal publication | composable through `System::activate`, which initializes before registration and returns separate delivery and retirement capabilities |
| `deliver` | clone the exact `ActorRef` endpoint and use `DeliveryEndpoint::deliver`, which preserves a rejected non-`Clone` command | composable |
| `fence` | Entity owns `EntityBehavior`/`EntityProtocol::DrainFence`; Bombay delivers it and routes the typed `DrainFenceAcknowledged` reply | executable conformance probe enqueues the exact protocol and awaits its typed acknowledgement |
| `retire` | retain the affine Bombay `RootRetirement` as the Entity lease; graceful retirement publishes typed shutdown and awaits `outcome`, while forced retirement calls `abort` and awaits `outcome` | executable conformance probe proves graceful and forced exact retirement release registration |

The seven adversarial contract tests additionally prove initialization rollback,
registration retry, live-incarnation survival under address collision,
non-`Clone` command recovery after retirement, separation of fence enqueue and
acknowledgement failures, FIFO command-before-fence ordering, and completion of
directory-owned lifecycle tasks.

The earlier activation mismatch was semantic, not merely a visibility
inconvenience: ordinary `System::spawn` publishes registration before the
Behavior initialization fold completes. Bombay #307 resolved it with the
focused transactional `System::activate` operation. The adapter uses that
operation when Entity requires initialize-before-commit, while ordinary actors
continue to use `System::spawn`.

Identity remains compartmentalized: Entity `EntityId` and `ActivationId` are
stable-routing/lifecycle facts; the adapter derives a Bombay address for
one activation; Address `RegistrationId` fences that exact registration;
Behavior child nonce and timer generations remain unrelated; Mnesis aggregate,
stream, command, tenant, and subscription-position identities remain
application/durability facts.

Do not implement Mnesis-specific substitutes first when the invariant remains
useful after removing Mnesis vocabulary. Bombay and its sibling crates must not absorb
aggregate decisions, event-store outcomes, conflict policy, inbox semantics,
or durable truth.

### Nexus/Mnesis

Propose two optional storage capabilities, with conformance tests for every
supporting adapter:

1. **Command inbox execution:** atomically check `(scope, command_id)`, append
   events, and record the durable typed/encoded outcome. Duplicate execution
   returns the recorded outcome without deciding/appending again. Define
   retention, hash/payload mismatch, in-progress recovery, and transaction
   isolation semantics.
2. **Consumer inbox/effect checkpoint:** atomically record a consumer dedup key
   and its protected local effect/checkpoint where the same store can own both.
   Do not claim atomicity for an external service.

These must be capability traits, not requirements on the `no_std` kernel or on
stores that cannot implement the transaction. The application still owns
whether a command is replayable and how long records must live.

### mnesis-bombay

Add the Mnesis protocol and command semantics, hydration factory,
activation-owned application interpreter, runtime-fact mapping, receipt
protocol, event relay,
integration health/telemetry, configuration validation, failure doubles,
contract tests and operational runbooks. It composes upstream capabilities; it
does not own a generic entity directory, keyed scheduler, activation cache,
reply framework, or drain framework.

## Production gates

No “production ready” label until all applicable gates have evidence:

1. crash tests at every row of the failure table;
2. duplicate command tests before, during and after inbox record expiry;
3. per-key linearization and cross-key fairness model/property tests;
4. bounded memory under unique-key floods and hot-key floods;
5. mailbox, scheduler, store and relay saturation with wait/reject/deadline;
6. graceful shutdown during load, decide, append, reply and checkpoint;
7. panic/restart-budget tests with no reuse of poisoned roots;
8. durable Fjall and Postgres tests, not only in-memory;
9. relay restart, duplicate, poison and checkpoint-corruption tests;
10. schema upgrade and rolling-version compatibility replay;
11. authorization/tenant isolation and metadata-redaction tests;
12. backup restore drill followed by full projection rebuild and comparison;
13. p50/p95/p99 latency, throughput, allocation and retained-memory results for
    cold/hot, cardinality, conflict, overload and durable-store profiles;
14. telemetry cardinality and degraded-readiness alerts under soak;
15. API compile-fail tests preventing identity/protocol mixing;
16. dependency/feature audit proving the core remains runtime-neutral;
17. semver, MSRV, license, advisory, fuzz, mutation and Miri gates appropriate
    to each unsafe/concurrent/parser boundary;
18. a reference application and operator runbook exercising real Bombay
    supervision and a real persistent Mnesis adapter.

## Research basis

Local source, public APIs, tests, documentation, and released package contents
were re-inspected on 2026-08-14. The executable graph locks `bombay-rs` 0.1.0,
`bombay-engine` 0.1.0, `bombay-behavior` 0.9.5, `bombay-entity` 0.1.0,
Communication 0.1.1, Address 0.1.1, Observe 0.1.0, Timers 0.1.0, and the Mnesis
packages compatible with 0.2 and locked to 0.2.2. The committed graph contains
no sibling path or Git patches.

External primary/official references used as comparative evidence:

- Orleans virtual actors and the distributed directory/activation/passivation
  model: <https://www.microsoft.com/en-us/research/project/orleans-virtual-actors/>
- Orleans technical report: <https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/Orleans-MSR-TR-2014-41.pdf>
- Tower `Service::poll_ready` reservation/backpressure contract:
  <https://docs.rs/tower/latest/tower/trait.Service.html>
- Tower middleware ordering, buffering, limits and load shedding:
  <https://docs.rs/tower/latest/tower/struct.ServiceBuilder.html>
- Akka documentation used for comparison of persistent actor stashing,
  supervision, sharding and delivery semantics:
  <https://doc.akka.io/docs/akka/current/typed/persistence.html>
- Tokio cancellation safety guidance:
  <https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety>
- Empirical event-sourcing/schema-evolution study:
  <https://arxiv.org/abs/2104.01146>
- Original actor-model semantics bibliography maintained by Behaviorpass:
  `../behaviorpass/research/architecture-critical-review-loop/AGHA-BIBLIOGRAPHY.md`.

These systems provide evidence and counterexamples, not APIs to copy. Orleans
placement guarantees in particular require infrastructure Bombay does not
currently have.
