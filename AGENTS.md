# Contributor and agent guide

`mnesis-bombay` integrates Mnesis with Actorpass without making either system
the application's architectural centre. Read
`docs/adr/0001-runtime-neutral-command-execution.md` before changing a public
boundary.

## Work authority and workflow

Use these sources in this order:

1. The Bombay-Nexus GitHub project, epic, and selected card define executable
   scope, ownership, dependencies, acceptance criteria, and current status.
2. The linked ADR, production-readiness audit, and implementation roadmap
   define system-wide boundaries, invariants, failure semantics, and rationale.
3. Current sibling source and executable tests are the factual API and behavior
   baseline. Never implement from a card or document without reconciling it
   against the exact checked-out revisions.
4. New tests, models, benchmarks, and implementation evidence determine whether
   the card is complete.

Begin at the production epic and select the first unblocked `Todo` child. Read
the card and only its relevant linked research completely, inspect all owning
and consuming repositories, then move it to `In Progress`. Work on a signed
`feat/*` branch. Record discoveries, decisions, verification commands,
benchmark context, and the PR on the card. Mark it `Done` only when every
acceptance criterion links to executable evidence and the protected PR is
merged.

If sources disagree, stop implementation long enough to reconcile them:

- a card conflicting with an ADR must be corrected or the ADR superseded;
- documentation conflicting with code requires a recorded contract decision;
- a newly discovered capability or materially expanded scope becomes a linked
  issue with an explicit owner rather than an unrecorded addition;
- a card cannot override current behavior merely by claiming it.

The current entry point is the production epic
`devrandom-labs/mnesis-bombay#2`. Follow its GitHub sub-issue dependency graph;
do not infer ordering from issue numbers alone.

## Design and modularization discipline

Always begin with the invariant, owner, lifecycle, failure boundary, and
required evidence. Select established design patterns when they solve those
specific forces; do not introduce a pattern merely because its name fits.

- Apply dependency inversion: domain and runtime-neutral application contracts
  do not import runtime, transport, executor, or concrete storage concerns.
- Prefer ports and adapters at actual variability boundaries. Require a second
  real implementation before promoting a one-consumer helper to a public
  abstraction.
- Use composition and typed decorators for orthogonal policy. Avoid a central
  mediator, service locator, global registry, `Any`, `TypeId`, universal
  envelope, or dynamic dispatch on the typed hot path unless a proven plugin
  boundary requires erasure.
- Keep command request/reply, committed-event fan-out, projection, saga, and
  external-effect delivery as distinct protocols with distinct acknowledgement
  and failure semantics.
- Model lifecycle and concurrency explicitly with state machines, bounded
  queues, ownership, cancellation, readiness, and shutdown laws. Supervision
  restores availability; it does not prove transaction outcome or justify
  retry.
- Make caches, actors, routes, and transports replaceable accelerators or
  adapters. Mnesis's committed log remains the durable authority.
- Keep optional interoperability such as Tower in a separate crate so users do
  not pay its dependency or conceptual cost.
- Preserve type safety with domain newtypes and closed protocols. Encode illegal
  states as unrepresentable where practical, and use compile-fail tests for
  architectural boundaries.
- Treat GoF, CQRS, event sourcing, actor, mediator, inbox/outbox, repository,
  unit-of-work, strategy, decorator, adapter, and supervisor patterns as tools
  with trade-offs. Document why the selected pattern fits, alternatives
  rejected, and what guarantee it does not provide.
- Optimize only after correctness and independence are established. Performance
  evidence must compare equivalent durability/acknowledgement boundaries and
  include latency distributions, allocations, retained memory, saturation,
  cardinality, and real persistent stores.

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
