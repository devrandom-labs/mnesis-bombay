use core::fmt;
use core::marker::PhantomData;

use mnesis::{Aggregate, Handle};

use crate::ValidatedCommandContext;

/// Application-owned command, causation, and correlation identities.
///
/// The three representations are independent generic parameters. The
/// integration neither chooses their representation nor permits their roles to
/// be exchanged accidentally.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[allow(
    clippy::struct_field_names,
    reason = "the suffix names three distinct identity roles and is the semantic distinction this type preserves"
)]
pub struct CommandIdentity<CommandId, CausationId, CorrelationId> {
    command_id: CommandId,
    causation_id: CausationId,
    correlation_id: CorrelationId,
}

impl<CommandId, CausationId, CorrelationId> CommandIdentity<CommandId, CausationId, CorrelationId> {
    /// Creates an identity set without changing any application representation.
    pub const fn new(
        command_id: CommandId,
        causation_id: CausationId,
        correlation_id: CorrelationId,
    ) -> Self {
        Self {
            command_id,
            causation_id,
            correlation_id,
        }
    }

    /// Borrows the stable command identity.
    pub const fn command_id(&self) -> &CommandId {
        &self.command_id
    }

    /// Borrows the causation identity.
    pub const fn causation_id(&self) -> &CausationId {
        &self.causation_id
    }

    /// Borrows the correlation identity.
    pub const fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    /// Returns all application-owned identities in their typed roles.
    pub fn into_parts(self) -> (CommandId, CausationId, CorrelationId) {
        (self.command_id, self.causation_id, self.correlation_id)
    }
}

impl<CommandId, CausationId, CorrelationId> fmt::Debug
    for CommandIdentity<CommandId, CausationId, CorrelationId>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandIdentity")
            .field("command_id", &"<redacted>")
            .field("causation_id", &"<redacted>")
            .field("correlation_id", &"<redacted>")
            .finish()
    }
}

/// A command for an aggregate instance that has already been selected.
///
/// `A: Handle<C, N>` makes the aggregate/command relationship part of the
/// request's type. The aggregate ID is intentionally absent: direct hosts pair
/// this request with [`crate::Addressed<A::Id, _>`], while the Bombay adapter
/// routes it with `bombay_entity::EntityId<A::Id>`. Keeping routing outside the
/// payload prevents two aggregate identities from disagreeing.
pub struct CommandRequest<A, C, Identity, Context, Deadline, const N: usize = 0>
where
    A: Aggregate + Handle<C, N>,
    Context: ValidatedCommandContext,
{
    identity: Identity,
    command: C,
    context: Context,
    deadline: Option<Deadline>,
    aggregate: PhantomData<fn() -> A>,
}

impl<A, C, Identity, Context, Deadline, const N: usize>
    CommandRequest<A, C, Identity, Context, Deadline, N>
where
    A: Aggregate + Handle<C, N>,
    Context: ValidatedCommandContext,
{
    /// Creates a request for the statically selected aggregate and command.
    pub const fn new(
        identity: Identity,
        command: C,
        context: Context,
        deadline: Option<Deadline>,
    ) -> Self {
        Self {
            identity,
            command,
            context,
            deadline,
            aggregate: PhantomData,
        }
    }

    /// Borrows the request identities.
    pub const fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Borrows the typed domain command.
    pub const fn command(&self) -> &C {
        &self.command
    }

    /// Borrows the validated application context.
    pub const fn context(&self) -> &Context {
        &self.context
    }

    /// Borrows the application-defined absolute deadline, when present.
    pub const fn deadline(&self) -> Option<&Deadline> {
        self.deadline.as_ref()
    }

    /// Separates the request into the exact values needed by an interpreter.
    pub fn into_parts(self) -> (Identity, C, Context, Option<Deadline>) {
        (self.identity, self.command, self.context, self.deadline)
    }
}

impl<A, C, Identity, Context, Deadline, const N: usize> fmt::Debug
    for CommandRequest<A, C, Identity, Context, Deadline, N>
where
    A: Aggregate + Handle<C, N>,
    Context: ValidatedCommandContext,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandRequest")
            .field("aggregate", &core::any::type_name::<A>())
            .field("command", &core::any::type_name::<C>())
            .field("identity", &"<redacted>")
            .field("context", &"<redacted>")
            .field("deadline", &self.deadline.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}
