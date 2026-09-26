# Jadpo for VS Code

This local extension provides `.jadpo` file association, syntax highlighting,
comments/brackets/indentation, snippets, and a persistent compiler-backed
language server. Unsaved edits receive live parse, semantic, type, failure, and
effect diagnostics. Document/workspace symbols, go-to-definition, references,
hover types, completion, conservative rename, semantic tokens, and deterministic
formatting all use the same compiler syntax tree, semantic graph, and inferred
types as the CLI. Declarations link to their generated validation, callable,
failure-audit, or OpenAPI artifact when a build exists.

The `Jadpo: Check Project` command requests a fresh compiler diagnostic pass.
The server is also available to any standard LSP client through `jadpo lsp`.

For local development, open this directory in VS Code and run the extension
host. Set `jadpo.executable` to the built `jadpo` binary if it is not on `PATH`.
