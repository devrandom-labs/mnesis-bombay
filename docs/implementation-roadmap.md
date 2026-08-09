# Production implementation roadmap

This is dependency order, not calendar order. A later slice must not be pulled
forward by hiding an unresolved earlier guarantee in an adapter.

## Phase 0 — Freeze contracts with executable models

Repository: `mnesis-bombay`

- Replace placeholder request/outcome types with the protocol described in the
  production-readiness audit.
- Add identity newtypes and compile-fail tests for aggregate/actor/command/
  position confusion.
- Write independent state models for activation, per-key queues, commit phases,
  shutdown and relay checkpointing.
- Build fault-injecting repository, clock, router, receipt and checkpoint
  doubles before implementing the runtime.
- Turn every failure-table row into a named contract test.

Exit: direct, Tower and Actorpass hosts can be tested against the same executor
contract and outcome vocabulary without a persistent store.

## Phase 1 — Runtime-neutral executor

Repositories: `mnesis-bombay`; no Actorpass changes

- Implement `AggregateExecutor<A, C, R>` over Mnesis repository capabilities.
- Preserve typed domain, conflict, storage and ambiguous errors.
- Implement explicit conflict reload/re-decision with replay eligibility and a
  bounded budget.
- Add direct-host integration tests against in-memory, Fjall and Postgres.
- Add decorators for authorization facts, tracing/metadata and metrics without
  introducing a mediator registry.

Exit: durable command semantics work without Actorpass and produce a committed
position that can be used for read-your-writes.

## Phase 2 — Local Actorpass host

Repository: `mnesis-bombay`; use current Actorpass/Behaviorpass APIs

- Add typed `AggregateRuntime` builder and `AggregateHandle<A>`.
- Spawn a supervised fixed shard pool under the root guardian.
- Implement the bounded per-key scheduler and activation state machine.
- Add cache bounds, idle eviction, poison invalidation and telemetry.
- Add typed Actorpass reply envelopes, overload behavior and phase-aware
  deadlines locally.
- Compose restart budgets, lifecycle observation, receive timeout/timers and
  coordinated shutdown from existing sibling primitives.

Exit: local crashes restore availability, uncertain roots are never reused,
all queues/caches have measured bounds, and saturation preserves per-key order
and cross-key progress.

## Phase 3 — Durable command identity

Repositories: Nexus first, then `mnesis-bombay`

- Specify an optional atomic command-inbox capability and storage-neutral
  conformance suite in Nexus.
- Implement it for adapters whose transaction model can satisfy it; expose
  unsupported capability honestly for the others.
- Define outcome encoding/versioning, request fingerprint mismatch, retention,
  expiry and privacy semantics.
- Integrate executor lookup/recording and replace resolvable ambiguity with
  duplicate outcome recovery.

Exit: retry with the same command ID cannot apply a command twice at the named
inbox+event-stream transaction boundary.

## Phase 4 — Committed-event relay

Repositories: `mnesis-bombay`; optional Nexus consumer-inbox capability

- Build one relay per consumer group over Mnesis `$all` subscription.
- Persist relay state and checkpoint atomically through `SnapshotStore`.
- Implement typed receipt levels, bounded retry with jitter/backoff, poison
  policy and quarantine inspection/replay.
- Propagate relay backpressure without spawning an unbounded task per event.
- Add optional consumer inbox integration for effects sharing the transaction;
  document external effects as at-least-once unless their own idempotency
  boundary says otherwise.

Exit: crash tests prove no silent checkpoint skip and explicitly characterize
duplicates for every receipt mode.

## Phase 5 — Upstream distillation

Repositories: Actorpass and possibly Behaviorpass, only from evidence

- Reconcile the concrete host against Actorpass F3, L1, L3, L4, L5 and L6.
- Extract only Mnesis-free invariants with another consumer or compelling
  cross-runtime proof.
- Keep aggregate cache, event-store outcomes, inbox and relay semantics in the
  integration even if generic scheduling/admission primitives move upstream.
- Add no Behaviorpass feature unless a new pure action has two interpreters and
  cannot be expressed by its existing closed protocols.

Exit: upstream APIs are smaller than the integration policy they support and
have independent conformance tests.

## Phase 6 — Operations and release

Repositories: `mnesis-bombay`, reference application and deployment assets

- Health/readiness/degraded endpoints and dashboards.
- Alert thresholds for queue depth, age, cache pressure, conflict, ambiguity,
  crash loops, relay lag, poison and checkpoint failures.
- Configuration schema with safe finite defaults and startup validation.
- Rolling-upgrade compatibility suite and event replay corpus.
- Backup/restore and full projection-rebuild drills.
- Security threat model, tenant isolation, metadata redaction, key rotation and
  dependency/supply-chain gates.
- Soak, chaos, cardinality, allocation and durable-store performance reports.

Exit: every gate in `production-readiness-research.md` links to reproducible
evidence and an operator can diagnose/recover the reference deployment.

## Phase 7 — Optional clustered execution

This is not an extension flag on the local scheduler. It requires a separate
architecture decision covering membership, placement, ownership transfer,
fencing, network partitions, rebalance, remote authentication, delivery
acknowledgements and rolling protocol compatibility.

Optimistic stream conflicts alone are not a placement protocol. Until a
cluster design passes split-brain and failover tests, run multiple stateless
ingress instances against direct optimistic execution or route each aggregate
to one local host through external infrastructure with explicitly documented
failure semantics.
