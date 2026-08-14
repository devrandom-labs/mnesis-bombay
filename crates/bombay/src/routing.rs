use bombay_entity::EntityId;
use mnesis::Aggregate;
use mnesis_bombay_core::Addressed;

/// Translates a directly addressed aggregate message into Bombay Entity's
/// stable local routing identity and an unchanged payload.
///
/// This is intentionally the sole aggregate-ID-to-Entity-ID mapping. The
/// message carries no duplicate aggregate identity that could disagree with
/// the selected entity.
pub fn into_entity_delivery<A, Message>(
    addressed: Addressed<A::Id, Message>,
) -> (EntityId<A::Id>, Message)
where
    A: Aggregate,
{
    let (aggregate_id, message) = addressed.into_parts();
    (EntityId::new(aggregate_id), message)
}
