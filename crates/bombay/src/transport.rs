/// Bombay transport envelope around a runtime-neutral command request.
///
/// The reply capability belongs to Bombay delivery and never enters
/// `mnesis-bombay-core` or the Mnesis domain decision.
#[derive(Debug)]
pub struct ExecuteRequest<Request, Reply> {
    request: Request,
    reply: Reply,
}

impl<Request, Reply> ExecuteRequest<Request, Reply> {
    /// Adds a Bombay-owned typed reply capability to a core request.
    pub const fn new(request: Request, reply: Reply) -> Self {
        Self { request, reply }
    }

    /// Borrows the runtime-neutral request.
    pub const fn request(&self) -> &Request {
        &self.request
    }

    /// Borrows the Bombay reply capability.
    pub const fn reply(&self) -> &Reply {
        &self.reply
    }

    /// Separates application execution input from runtime reply transport.
    pub fn into_parts(self) -> (Request, Reply) {
        (self.request, self.reply)
    }
}
