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

## Update the local extension

First rebuild the compiler from the repository root:

```text
cd jadpo
cargo build -p jadpo-cli
```

For extension development, open `editors/vscode` in VS Code, press `F5` to
start an Extension Development Host, and use **Developer: Reload Window** in
that host after subsequent changes. The extension automatically prefers the
workspace binary at `jadpo/target/debug/jadpo`; set `jadpo.executable` to an
absolute binary path only when using a different checkout or build.

For a normally installed local copy, package a VSIX and reinstall it:

```text
cd editors/vscode
npx --yes @vscode/vsce package --out jadpo-language.vsix
code --install-extension jadpo-language.vsix --force
```

If the `code` shell command is unavailable, choose **Extensions: Install from
VSIX...** and select that file instead. Reload the window after installation.
Rebuild and reinstall the VSIX whenever the extension files change; rebuilding
the compiler alone is sufficient when only compiler or terminal behaviour
changes.
