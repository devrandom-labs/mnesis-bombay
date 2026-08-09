# Local sibling development

Actorpass is not yet published and its declared GitHub repository is not
remotely accessible. The development workspace therefore uses the sibling
`../actorpass` path. Local patches also force current sibling Mnesis and
Behaviorpass sources during architecture validation.

This means the full workspace is not yet a standalone hermetic Cargo/Nix build.
Publishing Actorpass or exposing a pinned remote revision is a release blocker.
Once available, replace the Actorpass path with an exact version or Git
revision and remove the committed local patches below the root manifest.

The intended local override shape is:

```toml
[patch.crates-io]
mnesis = { path = "../nexus/crates/mnesis" }
mnesis-store = { path = "../nexus/crates/store" }
mnesis-inmemory = { path = "../nexus/adapters/inmemory" }
bombay-behavior = { path = "../behaviorpass/crates/behavior" }
bombay-behavior-macros = { path = "../behaviorpass/crates/behavior-macros" }

[patch."https://github.com/devrandom-labs/actorpass"]
actorpass = { path = "../actorpass/crates/actorpass" }
```

Always run the full test and benchmark gates both with the hermetic committed
dependency graph and with local sibling overrides before proposing a cross-repo
API change.
