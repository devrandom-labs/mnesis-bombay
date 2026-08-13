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

Exit: direct and Bombay hosts share the same factual request/outcome
vocabulary without forcing one runtime executor abstraction into core.

## Phase 1 — Mnesis command execution

Repositories: `mnesis-bombay`; no Bombay changes

- Implement the Mnesis-owned load, decide, append, conflict, and ambiguity
  semantics over repository capabilities, callable directly and reusable by an
  Bombay-hosted aggregate activation.
- Preserve typed domain, conflict, storage and ambiguous errors.
- Implement explicit conflict reload/re-decision with replay eligibility and a
  bounded budget.
- Add direct-host integration tests against in-memory, Fjall and Postgres.
- Add decorators for authorization facts, tracing/metadata and metrics without
  introducing a mediator registry or a parallel general-purpose service stack.

Exit: durable command semantics work without Bombay and produce a committed
position that can be used for read-your-writes.

## Phase 2 — Verify and integrate Bombay runtime prerequisites

Repositories: owning Bombay runtime and focused sibling crates first

- Consume `bombay-entity` 0.1.0 for stable typed `EntityId`, race-free local
  activation, bounded activation waiters, exact-incarnation routing,
  passivation fences, typed refusal, and graceful/forced draining. Do not
  rebuild these facilities in this repository.
- Verify the exact locked Bombay, Behavior, Communication, Address, Observe,
  Timers, Transition, Machine Executor, and Entity source, tests, and
  documentation before changing the adapter. Bombay F3 and L3-L6 are
  feature-complete inputs, not open implementation work for this repository.
- Keep Bombay #268 and distributed directory work out of the local critical
  path. They concern the future location-transparent remote arm; local entity
  correctness is owned by `bombay-entity`.
- Consume Bombay #307's transactional `System::activate` seam. It completes
  initialization before registration and returns separately nameable cloneable
  delivery and affine retirement capabilities, satisfying Entity's
  activate-before-commit contract.
- Keep total active-entity capacity assigned to
  `devrandom-labs/bombay-entity#6` and generic lifecycle facts assigned to
  `devrandom-labs/bombay-entity#7`; do not substitute adapter-local machinery.
- Keep hydration opaque to the runtime: an application factory may load from
  Mnesis, a KV store, or memory without `bombay-entity` importing any of them.
- Prove local activation/passivation races, capacity bounds, shutdown, retained
  memory, and high-cardinality behavior before calling the prerequisites ready.

Exit: the adapter has executable conformance evidence against `bombay-rs`
0.1.0, `bombay-behavior` 0.9.5, and `bombay-entity` 0.1.0, including activation,
delivery, ordered fence acknowledgement, and exact retirement, without
introducing Mnesis vocabulary upstream.

## Phase 3 — Mnesis Bombay host

Repository: `mnesis-bombay`; depends on the Phase 2 upstream cards

- Supply the typed Mnesis hydration factory and activation-owned application
  interpreter while keeping Behavior pure; do not implement a private entity
  directory or keyed scheduler.
- Map aggregate ID to entity key without confusing actor incarnation, stream
  identity, tenant, or command identity.
- Retire/poison an activation after conflict, panic, cancellation, or ambiguous
  completion according to the factual command-phase model.
- Map upstream admission, deadline, reply, passivation, and drain facts into
  the runtime-neutral command outcome vocabulary.

Exit: local crashes restore availability, uncertain roots are never reused,
all queues/activations have measured upstream bounds, and unrelated aggregate
entities make concurrent progress.

## Phase 4 — Durable command identity

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

## Phase 5 — Committed-event relay

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

## Phase 7 — Distributed entity hosting and fencing

Locationpass deliberately gives the adapter one location-transparent interface,
but local activation ownership is not distributed single-writer proof. The
remote arm requires explicit membership, placement, ownership transfer,
fencing, network partitions, rebalance, authentication, delivery
acknowledgements, and rolling protocol compatibility owned by the appropriate
Bombay, Zenoh, KERI, and Mnesis capabilities.

Optimistic stream conflicts alone are not a placement protocol. Until a
cluster design passes split-brain and failover tests, run multiple stateless
ingress instances against direct optimistic execution or route each aggregate
to one local host through external infrastructure with explicitly documented
failure semantics.
