# Module seed

This two-file application is the first P10.5 module/import acceptance example.
It demonstrates explicit module identity, selective imports, module-public
declarations, private-by-default declarations, and cross-file nominal field
references without an ambient project-wide namespace.

```text
cargo run -p jadpo-cli -- check ../examples/module-seed
cargo run -p jadpo-cli -- inspect ../examples/module-seed
```

The bounded first slice deliberately excludes import aliases, re-exports,
same-name declarations in separate modules, relative module paths, and external
packages. Those features require separate fixtures and language decisions.
