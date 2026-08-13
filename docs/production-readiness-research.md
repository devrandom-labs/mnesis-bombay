# Production-readiness architecture audit

Status: normative research and implementation backlog

Date: 2026-08-09

Scope: a correct, operable Mnesis command/event integration hosted by Actorpass

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
 direct host       Actorpass host adapter
                           │ locationpass entity route
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
single-writer placement is a distinct later product: Actorpass currently has
no distributed directory, membership, fencing lease, rebalance protocol, or
remote mailbox guarantee. A local adapter must never imply those guarantees.

## Ownership rules

| Repository/layer | Owns | Must not own |
|---|---|---|
| application/domain | aggregate, command, event, rejection, business idempotency and authorization facts | actors, storage adapters, transport acknowledgements |
| Nexus/Mnesis | event-stream correctness, append conflict, committed positions, subscriptions, atomic durable records | actor lifecycle, retry timing, HTTP/NATS policy |
| Behaviorpass | pure event-to-actions algebra and closed typed protocols | I/O, repositories, queues, durability, restart implementation |
| Actorpass/Bombay runtime family | actor incarnation, mailbox, typed request/reply, entity routing, bounded activation/passivation, effect order, lifecycle, supervision, admission and draining mechanics | aggregate semantics, durable acceptance, Mnesis conflict policy |
| `mnesis-bombay-core` | runtime-neutral request/outcome vocabulary and command-phase facts | Tokio, Tower, Actorpass, concrete store |
| `mnesis-actorpass` | Mnesis hydration factory and activation-owned application interpreter, outcome mapping, committed-log relay composition | generic entity directory, activation scheduler, passivation, runtime admission/drain mechanics |
| optional Tower crate | `Service` interoperability and middleware mapping | semantic retry defaults or durable truth |
| ingress/egress adapter | serialization, authentication evidence, protocol acknowledgements | changing internal failure meaning |

## Capability and gap matrix

Legend: **existing** means the required primitive is present, not that the
end-to-end policy is complete. **add** is required production work.

| Capability | Required invariant | Existing evidence | Owner and action |
|---|---|---|---|
| aggregate selection | command type maps statically to exactly one aggregate type | Mnesis `Handle<C>` and aggregate traits | **compose Mnesis:** typed endpoint; no registry or duplicate handler abstraction |
| instance selection | request always carries validated `A::Id` | domain IDs and Mnesis stream mapping exist | **add core:** `CommandRequest<A::Id, C>` |
| stable execution key | tenant/kind/id cannot be confused with actor incarnation | identity distinctions documented only | **add core:** newtypes and explicit mapping |
| startup registration | host explicitly installs each supported aggregate family | Actorpass spawn and Bombay #268 factory seam | **add upstream + adapter:** locationpass registration plus typed Mnesis factory |
| activation | first command lazily reconstructs a missing root | Mnesis `Repository::load`; Bombay #268 specifies get-or-activate | **upstream:** locationpass lifecycle; **adapter:** hydrate from Mnesis |
| passivation | idle roots are evicted without losing truth | Bombay #268 specifies passivation | **upstream:** bounded safe passivation; **adapter:** disposable Mnesis activation |
| per-key ordering | one process never executes two commands for one key concurrently | actor turn serialization exists | **upstream:** one activation/turn stream per entity key |
| cross-key concurrency | slow key does not block unrelated keys | independent actors provide the intended topology | **upstream:** bounded independent activations; benchmark shared-resource contention |
| hot-key fairness | one key cannot monopolize shared runtime/store capacity | generic runtime concern | **upstream:** dispatcher/admission fairness and starvation telemetry |
| durable execution | success means append confirmed | Mnesis `CommandRepository::execute` | **use existing**, wrap with typed outcome |
| conflict recovery | discard root, reload, re-decide, bounded retry | Mnesis exposes conflict predicate, policy consumer-owned | **add core/adapter:** explicit replay eligibility and retry budget |
| command deduplication | same command ID cannot apply twice when outcome is uncertain | no command inbox in Nexus | **propose Nexus:** atomic command-inbox record plus event append capability |
| lost reply | never translate unknown commit into safe retry | no durable inbox | **add core:** `AmbiguousCompletion`; inbox later resolves it |
| cancellation | dropping a future after commit begins invalidates cached state | Tokio/Actorpass cancellation exists; no integration policy | **adapter:** commit-phase guard; **upstream:** retire activation mechanism |
| panic recovery | panic retires incarnation and never reuses uncertain root | Actorpass classifies `TaskOutcome::Panicked` and supports restart | **compose:** restart/revive empty activation; fail/resolve outstanding receipts |
| restart budget | crash loops terminate or degrade predictably | Behaviorpass supervision budget/restart strategy exists | **use existing**, configure and observe it |
| child ownership | supervisor retirement shuts down owned workers | Actorpass child leases/scopes and guardian mechanics exist | **use existing**, contract-test ordering |
| mailbox pressure | user admission is bounded | Bombay Communication/Actorpass bounded user lane | **use existing**, never bypass with unbounded queue |
| execution pressure | store and activation capacity propagate to callers | generic Actorpass roadmap work remains | **upstream:** bounded admission; **adapter:** map runtime facts and store limits |
| overload policy | wait, reject, and deadline have distinct typed outcomes | Actorpass roadmap L3 remains pending | **implement upstream before adapter consumption** |
| readiness | capacity reservation is consumed or released correctly | Tower has an optional interoperability contract | **upstream:** actor admission law; **Tower adapter:** faithful mapping |
| deadlines | deadline covers queue and execution but does not lie about commit | timeout primitives exist, semantic phase absent | **add core:** absolute deadline and phase-aware outcome |
| graceful shutdown | stop ingress → drain/reject → settle commits → checkpoint → stop children | Actorpass guardian/shutdown exists; job drain roadmap L4 pending | **upstream:** generic drain; **adapter:** Mnesis/relay phase ordering |
| immediate command reply | reply only after durable fact | Actorpass F3 pending | **upstream:** typed reply/timeout; **adapter:** send only factual Mnesis outcome |
| committed event source | reliable notifications originate after append | Mnesis `$all` subscription and positions | **use existing** |
| relay checkpoint | resume strictly after last acknowledged policy boundary | positions exist; generic consumer runner intentionally absent | **add adapter relay host** |
| checkpoint storage | checkpoint survives restart atomically with relay state | Mnesis `SnapshotStore<S, P>` can store state+position | **use existing** for relay state |
| consumer receipt | enqueue, fold, effect, and inbox commit are different facts | Actorpass send acknowledges publication only | **add adapter protocol:** typed receipt level |
| consumer dedup | duplicate relay delivery cannot repeat protected effect | no general consumer inbox | **application storage or propose Nexus optional inbox capability** |
| poison event | checkpoint never silently skips failed decode/handling | subscriptions surface errors | **add relay:** halt/quarantine/recorded-skip policy |
| retry scheduling | bounded attempts, jitter and backoff; no mailbox-blocking sleep | Actorpass timers exist; roadmap L5 pending | **compose timers in adapter**, upstream after second consumer |
| projection lag | expose head, checkpoint and distance/age | positions exist but distance may be adapter-specific | **add telemetry adapter**, never assume dense sequence |
| read-your-writes | caller can await projection reaching append position | Mnesis append returns last committed position | **add optional consistency waiter** with timeout/ambiguity |
| snapshots | optimization failure never changes correctness | Mnesis snapshot decorator is best effort | **configure application**, expose hit/stale/failure metrics |
| schema evolution | old events remain decodable or explicitly upcast | schema version/envelope support exists; policy app-owned | **application:** versioned codecs/upcasters and replay test corpus |
| multi-aggregate work | no implied transaction across streams | Mnesis has `AtomicAppend` capability for supported stores | **application service:** require capability explicitly or use saga |
| sagas/process managers | durable state and intents resume after crash | Mnesis saga/projected-intent primitives exist; runner consumer-owned | **add separate host**, not command executor magic |
| external effects | no dual-write claim | committed-log relay available | **relay/outbox consumer** with named inbox/effect boundary |
| local discovery | address resolves exact incarnation and retirement | Actorpass Address/Observe integration | **use existing** |
| cluster placement | one active owner or safe concurrent writers | absent; optimistic concurrency only catches conflicting append | **new future distributed adapter/runtime**, not local v1 |
| membership/rebalance | moves ownership without loss or split brain | absent | **future distributed project** |
| fencing | stale owner cannot write after lease loss | absent from generic store contract | **future store/coordination capability** |
| remote delivery | protocol defines authentication, dedup, order and ack | deliberately downstream of KERI/Zenoh in Actorpass roadmap | **separate integration repositories** |
| authentication | ingress proves caller identity | outside all current core crates | **transport/application** |
| authorization | decision uses authenticated principal/tenant facts | metadata can carry bytes but policy absent | **application decorator/domain**, typed context |
| tenant isolation | routing, cache, stream and metrics include tenant safely | absent as universal policy | **application/core key type**, deployment quotas |
| secrets/encryption | keys rotate; logs and snapshots protect sensitive data | signed-event example exists, encryption policy absent | **store/deployment adapter**, never Actorpass Behavior |
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

The current generic `CommandRequest<I, C>` and `CommandOutcome<P, R, E>` are
only placeholders. The production protocol must represent facts rather than
transport machinery:

```rust,ignore
struct CommandRequest<A, C> {
    aggregate_id: A,
    command_id: CommandId,
    command: C,
    context: CommandContext,
    deadline: Option<Deadline>,
}

enum CommandOutcome<P, O, R, S> {
    Committed { position: P, output: O, duplicate: bool },
    Rejected(R),
    ConflictExhausted { attempts: NonZeroU32 },
    Storage(S),
    Overloaded { retry_after: Option<Duration> },
    DeadlineBeforeExecution,
    AmbiguousCompletion { command_id: CommandId },
    ShuttingDown,
}
```

Transport reply channels do not belong inside this core request. An Actorpass
adapter envelope may add a typed reply capability around it.

`CommandContext` must carry typed or validated command identity, causation,
correlation, authenticated principal evidence, tenant, trace context and
schema/protocol version. It must have explicit size limits and redaction rules.

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

### Actorpass and focused Bombay runtime crates

Actorpass is intended to be a best-in-class actor runtime, not a frozen set of
current primitives. Generic runtime facilities must be implemented and proven
in their owning Bombay repository before mnesis-bombay builds private versions.
Use existing bounded mailboxes, effect ordering, child scopes, guardian,
keep-address restart, lifecycle reporting, timers and peer observation, then
complete the pending reusable capabilities needed by this integration:

- F3 typed reply/timeout composition;
- devrandom-labs/bombay#268 location-transparent entity activation and
  passivation, coordinated with L1 only where a worker-pool topology is useful;
- L3 typed admission refusal;
- L4 graceful in-flight draining;
- L5 retry scheduling/backoff;
- L6 queryable application reporting.

Do not implement Mnesis-specific substitutes first when the invariant remains
useful after removing Mnesis vocabulary. Actorpass/Bombay must not absorb
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
18. a reference application and operator runbook exercising real Actorpass
    supervision and a real persistent Mnesis adapter.

## Research basis

Local source, tests and ledgers were inspected at the sibling working-tree
revisions on 2026-08-09. Especially important findings are Actorpass's pending
F3/L1/L3/L4/L5/L6 roadmap rows and Mnesis's explicit consumer-owned runner and
retry boundaries.

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
placement guarantees in particular require infrastructure Actorpass does not
currently have.
