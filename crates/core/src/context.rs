use core::fmt;

mod sealed {
    pub trait Sealed {}
}

/// Proof that application context crossed the validation, size, and redaction
/// boundary.
///
/// This trait is sealed: only [`ValidatedContext`] can satisfy it. A command
/// request therefore cannot accidentally carry raw ingress context.
pub trait ValidatedCommandContext: sealed::Sealed {}

/// Application policy for measuring, validating, and safely formatting command
/// context.
///
/// The integration owns no tenant, principal, trace, or protocol
/// representation. Applications keep those fields in `Self` and define the
/// validation and redaction policy appropriate to their ingress protocol.
pub trait Context: Sized {
    /// Application-specific validation failure.
    type Error;

    /// Size of the context at its ingress encoding boundary.
    fn encoded_len(&self) -> usize;

    /// Validates semantic invariants beyond the universal size bound.
    ///
    /// # Errors
    ///
    /// Returns the application's validation fact when any required context
    /// invariant is absent or invalid.
    fn validate(&self) -> Result<(), Self::Error>;

    /// Formats only fields approved by the application's redaction policy.
    ///
    /// # Errors
    ///
    /// Returns the formatter's error when the redacted representation cannot
    /// be written.
    fn fmt_redacted(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// Failure to establish a validated, bounded context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextError<E> {
    /// The encoded context is larger than the protocol permits.
    TooLarge {
        /// Measured encoded size.
        actual: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// Application semantic validation failed.
    Invalid(E),
}

/// A context proven to satisfy application validation and a protocol size
/// bound.
///
/// Construction is fallible and the wrapped value is private, so downstream
/// interpreters can require this type instead of repeatedly trusting raw
/// ingress context.
pub struct ValidatedContext<C, const MAX_BYTES: usize> {
    inner: C,
}

impl<C, const MAX_BYTES: usize> sealed::Sealed for ValidatedContext<C, MAX_BYTES> {}

impl<C, const MAX_BYTES: usize> ValidatedCommandContext for ValidatedContext<C, MAX_BYTES> {}

impl<C: Context, const MAX_BYTES: usize> ValidatedContext<C, MAX_BYTES> {
    /// Validates the application context and establishes the byte bound.
    ///
    /// # Errors
    ///
    /// Returns [`ContextError::TooLarge`] before semantic validation when the
    /// ingress encoding exceeds `MAX_BYTES`, or [`ContextError::Invalid`] when
    /// application validation rejects the value.
    pub fn try_new(inner: C) -> Result<Self, ContextError<C::Error>> {
        let actual = inner.encoded_len();
        if actual > MAX_BYTES {
            return Err(ContextError::TooLarge {
                actual,
                maximum: MAX_BYTES,
            });
        }
        inner.validate().map_err(ContextError::Invalid)?;
        Ok(Self { inner })
    }

    /// Borrows the validated application context.
    pub const fn get(&self) -> &C {
        &self.inner
    }

    /// Returns the validated application context.
    pub fn into_inner(self) -> C {
        self.inner
    }
}

impl<C: Context, const MAX_BYTES: usize> fmt::Debug for ValidatedContext<C, MAX_BYTES> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt_redacted(formatter)
    }
}
