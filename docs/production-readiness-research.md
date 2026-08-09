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
                           │ bounded keyed scheduler
                           │ activation/cache/supervision
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
| Actorpass | local actor incarnation, mailbox, routing, effect order, child lifecycle, supervision mechanics | aggregate semantics, durable acceptance, distributed placement |
| `mnesis-bombay-core` | runtime-neutral request/outcome vocabulary and executor contracts | Tokio, Tower, Actorpass, concrete store |
| `mnesis-actorpass` | Actorpass composition, activation, key scheduling, cache and relay host | domain decision rules or event-store internals |
| optional Tower crate | `Service` interoperability and middleware mapping | semantic retry defaults or durable truth |
| ingress/egress adapter | serialization, authentication evidence, protocol acknowledgements | changing internal failure meaning |

## Capability and gap matrix

Legend: **existing** means the required primitive is present, not that the
end-to-end policy is complete. **add** is required production work.

| Capability | Required invariant | Existing evidence | Owner and action |
|---|---|---|---|
| aggregate selection | command type maps statically to exactly one aggregate type | Mnesis `Handle<C>` and aggregate traits | **add core:** `AggregateExecutor<A, C>`/typed endpoint; no registry |
| instance selection | request always carries validated `A::Id` | domain IDs and Mnesis stream mapping exist | **add core:** `CommandRequest<A::Id, C>` |
| stable execution key | tenant/kind/id cannot be confused with actor incarnation | identity distinctions documented only | **add core:** newtypes and explicit mapping |
| startup registration | host explicitly installs each supported aggregate family | Actorpass `System::spawn` exists | **add adapter:** typed builder returning `AggregateHandle<A>` |
| activation | first command lazily reconstructs a missing root | Mnesis `Repository::load` | **add adapter:** activation state machine |
| passivation | idle roots are evicted without losing truth | no integration policy | **add adapter:** bounded cache, idle/capacity eviction |
| per-key ordering | one process never executes two commands for one key concurrently | actor turn serialization exists, benchmark shard mutex is too broad | **add adapter:** per-key FIFO/single-flight scheduler |
| cross-key concurrency | slow key does not block unrelated keys | not supplied by one shard mutex | **add adapter:** ready-key queue plus bounded active-key permits |
| hot-key fairness | one key cannot monopolize a shard indefinitely | not implemented | **add adapter:** quantum/one-command rotation and starvation metric |
| durable execution | success means append confirmed | Mnesis `CommandRepository::execute` | **use existing**, wrap with typed outcome |
| conflict recovery | discard root, reload, re-decide, bounded retry | Mnesis exposes conflict predicate, policy consumer-owned | **add core/adapter:** explicit replay eligibility and retry budget |
| command deduplication | same command ID cannot apply twice when outcome is uncertain | no command inbox in Nexus | **propose Nexus:** atomic command-inbox record plus event append capability |
| lost reply | never translate unknown commit into safe retry | no durable inbox | **add core:** `AmbiguousCompletion`; inbox later resolves it |
| cancellation | dropping a future after commit begins invalidates cached state | Tokio/Actorpass cancellation exists; no integration policy | **add adapter:** commit-phase guard and cache poisoning |
| panic recovery | panic retires incarnation and never reuses uncertain root | Actorpass classifies `TaskOutcome::Panicked` and supports restart | **compose adapter:** restart empty shard state; fail/resolve outstanding receipts |
| restart budget | crash loops terminate or degrade predictably | Behaviorpass supervision budget/restart strategy exists | **use existing**, configure and observe it |
| child ownership | supervisor retirement shuts down owned workers | Actorpass child leases/scopes and guardian mechanics exist | **use existing**, contract-test ordering |
| mailbox pressure | user admission is bounded | Bombay Communication/Actorpass bounded user lane | **use existing**, never bypass with unbounded queue |
| execution pressure | store and key capacity propagate to callers | no adapter readiness protocol | **add adapter:** bounded admission and permits |
| overload policy | wait, reject, and deadline have distinct typed outcomes | Actorpass roadmap L3 remains pending | **add adapter first; propose Actorpass optional primitive after proven general** |
| readiness | capacity reservation is consumed or released correctly | Tower `poll_ready` has this contract; Actorpass router is async delivery | **add Tower adapter:** correct reservation; **adapter:** direct equivalent |
| deadlines | deadline covers queue and execution but does not lie about commit | timeout primitives exist, semantic phase absent | **add core:** absolute deadline and phase-aware outcome |
| graceful shutdown | stop ingress → drain/reject → settle commits → checkpoint → stop children | Actorpass guardian/shutdown exists; job drain roadmap L4 pending | **add adapter protocol; upstream only when generic** |
| immediate command reply | reply only after durable fact | no framework ask/reply contract; Actorpass F3 pending | **add typed adapter reply capability**, then feed concrete requirements to F3 |
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

## Activation and scheduling state machine

Each aggregate key is in exactly one local state:

```text
Absent → Loading → Ready → Executing → Ready
   ▲         │        │         │          │
   └─────────┴─Failed─┴─Poisoned┴─Evicted──┘
```

- `Absent`: no memory retained; commands may create a bounded pending entry.
- `Loading`: one load is active; later commands join the bounded per-key FIFO.
- `Ready`: root is trusted and the key can be scheduled.
- `Executing`: exactly one command for the key is active.
- `Poisoned`: append status, panic, or cancellation made the root untrustworthy;
  discard it before further execution.
- `Evicted`: pending queue must be empty; the next command reloads.

The scheduler has a bounded number of key entries, a bounded queue per key, a
bounded global admitted-command count, and a bounded active-key count. After
one command, a still-ready hot key returns to the ready queue behind peers.
No production mode has an implicit unbounded queue or cache.

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

### Actorpass

Use existing bounded mailboxes, effect ordering, child scopes, guardian,
keep-address restart, lifecycle reporting, timers and peer observation.

The integration provides concrete evidence for currently pending roadmap work:

- F3 typed reply/timeout composition;
- L1 worker-pool and key-persistent routing;
- L3 typed admission refusal;
- L4 graceful in-flight draining;
- L5 retry scheduling/backoff;
- L6 queryable application reporting.

Implement Mnesis-specific versions in `mnesis-actorpass` first. Promote only
the invariant that remains after removing Mnesis vocabulary and after another
consumer proves it. Actorpass must not absorb aggregate caching or durable
command semantics.

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

Add the aggregate descriptor/executor, typed handles, Actorpass supervisor,
bounded keyed scheduler, activation cache, receipt protocol, event relay,
health/telemetry, configuration validation, failure doubles, contract tests
and operational runbooks. This is the composition owner.

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
