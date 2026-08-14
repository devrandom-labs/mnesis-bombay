/// A message paired with the application-owned identity that selects its
/// destination.
///
/// Direct hosts consume this value as-is. Runtime adapters consume it at their
/// routing boundary, translating `Id` into the runtime's own routing identity
/// while delivering `Message` unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Addressed<Id, Message> {
    id: Id,
    message: Message,
}

impl<Id, Message> Addressed<Id, Message> {
    /// Pairs an application identity with a message.
    pub const fn new(id: Id, message: Message) -> Self {
        Self { id, message }
    }

    /// Borrows the application-owned destination identity.
    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// Borrows the message without losing its routing identity.
    pub const fn message(&self) -> &Message {
        &self.message
    }

    /// Separates the routing identity from the message at an adapter boundary.
    pub fn into_parts(self) -> (Id, Message) {
        (self.id, self.message)
    }
}
