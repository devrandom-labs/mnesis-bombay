use mnesis_bombay_core::CommandIdentity;

struct CommandId(u64);
struct CausationId(u64);
struct CorrelationId(u64);

fn requires_roles(_: CommandIdentity<CommandId, CausationId, CorrelationId>) {}

fn main() {
    requires_roles(CommandIdentity::new(
        CorrelationId(1),
        CausationId(2),
        CommandId(3),
    ));
}
