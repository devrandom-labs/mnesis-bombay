# Local sibling development

The committed dependency graph is standalone and uses these released package
contracts:

- `bombay-rs` 0.1.0;
- `bombay-behavior` 0.9.5;
- `bombay-entity` 0.1.0;
- Mnesis packages compatible with 0.2, locked to 0.2.2.

To test a sibling change, apply a temporary local Cargo configuration or an
uncommitted root `[patch.crates-io]` entry. For example:

```toml
[patch.crates-io]
mnesis = { path = "../nexus/crates/mnesis" }
mnesis-store = { path = "../nexus/crates/store" }
mnesis-inmemory = { path = "../nexus/adapters/inmemory" }
```

Before proposing a cross-repository API change, run the full gates against
both the sibling override and the committed registry graph. Never commit path
patches or use them as evidence that released packages are compatible.
