# Developer tooling and LLM presentation workstream

**Status:** DX0.5 in progress; DX1 compiler-backed LSP and VS Code client implemented  
**Timing:** thin highlighting may begin during P10–P11; DX0.5 is required
during P10.5 before P11 application authoring; the minimum tooling baseline
must be frozen before the P12 comparison

The language is intended for agent-primary authorship, but weak human tooling
would still distort review, debugging, adoption, and the eventual comparison
with TypeScript. Editor and LLM presentation support are therefore part of the
product hypothesis rather than optional polish.

## 1. One compiler-backed language service

The semantic compiler remains authoritative. Editor integrations must not grow
a second parser, type system, or diagnostic catalogue that can disagree with
`jadpo check`.

The language server uses standard LSP over stdio and reuses the
compiler's source ranges, syntax tree, semantic graph, inferred expression
types, and structured diagnostics. Its delivery order is:

1. live parse/type/failure/effect diagnostics with exact CLI parity;
2. document symbols, go-to-definition, references, and workspace symbols;
3. hover information for types, fields, actions, routes, failures, constraints,
   effects, and generated behaviour;
4. context-aware completion and signature help;
5. semantic tokens, rename, formatting, and conservative code actions;
6. links from source declarations to relevant generated audit or contract
   artifacts without presenting generated target code as the normal review
   surface.

Hover documentation should be derived from the language reference and semantic
metadata where possible. Hand-copying descriptions into each editor extension
would create another source of truth.

## 2. Editor packaging

The first packaging target is VS Code and compatible editors because a TextMate
grammar, file association, snippets, and an LSP client provide a small portable
base. The same server should remain usable from Neovim, Helix, Emacs, and
editors with generic LSP support. JetBrains-specific packaging is justified
only after the protocol behaviour is stable.

The baseline editor package includes:

- a canonical source-file association;
- bracket, comment, indentation, and folding rules;
- a TextMate grammar for immediate lexical highlighting;
- LSP semantic-token refinement once available;
- formatter and `jadpo check` commands;
- links to the canonical language documentation; and
- extension tests using the Jadpo seed, persistence seed, and later the
  golden todo application.

The TextMate grammar is presentation-only. Its scopes must not be treated as
semantic evidence.

The package now lives in `editors/vscode`. It supplies `.jadpo` file
association, TextMate highlighting, brackets/comments/indentation, and snippets.
Its persistent client starts `jadpo lsp` and receives live diagnostics for
unsaved text, document/workspace symbols, cross-file definitions and references,
inferred-type hover, contextual member completion, callable signature help,
semantic tokens, conservative compiler-indexed rename, and the canonical
formatter. Source declarations also link to the relevant generated validation,
callable, failure-audit, or OpenAPI artifact. The extension contains no parser
or type checker.

## 3. Documentation in the editing loop

Documentation support has three layers:

- concise hover text for the declaration currently under the cursor;
- a searchable local language reference with canonical examples and diagnostic
  explanations; and
- stable documentation URLs or identifiers that diagnostics and generated
  audits can reference.

Every public diagnostic code should eventually have a short explanation,
likely causes, a safe correction, and links to relevant rules. Examples must be
compile fixtures or extracted from them so documentation cannot silently drift
from accepted syntax.

## 4. LLM and chat presentation

LLMs can already emit source in fenced Markdown blocks. Presentation quality is
controlled by the client renderer, not by the model alone. The project should
define a stable machine language identifier, separate from any eventual product
name, and use it consistently as the Markdown fence tag, file association, LSP
language ID, and grammar package ID.

The portable rendering deliverables are:

- TextMate grammar for editors and clients that consume TextMate scopes;
- adapters or generated grammars for Shiki, Prism, Highlight.js, and Monaco;
- fenced-snippet fixtures covering declarations, constraints, routes,
  persistence, failures, comments, strings, and malformed source; and
- a plain-text fallback that preserves code exactly when a host does not know
  the language identifier.

Inline backtick spans are normally rendered as undifferentiated code. Rich
syntax colouring should be expected only for fenced blocks or file views whose
host has installed the grammar. An existing third-party or hosted chat client
cannot be forced to recognize the language merely by prompting the LLM.

Official Codex documentation currently states that the CLI highlights fenced
Markdown code blocks and file diffs, and documents custom themes. It does not
document custom language-grammar registration. Treat Codex/ChatGPT-specific
grammar support as an external integration request, not a compiler guarantee:
[Codex CLI customization](https://learn.chatgpt.com/docs/cli-customization).

## 5. Giving agents language-native context

Rendering is separate from comprehension. Before considering model training or
fine-tuning, provide agents with a compact, versioned language bundle:

- core grammar and accepted examples;
- semantic rules and diagnostic catalogue;
- the current compiler version and capability manifest;
- non-interactive `check`, `inspect`, `build`, and formatting commands; and
- small machine-readable queries for symbol information, hover documentation,
  definitions, references, and diagnostics.

This bundle may later be packaged as a Codex skill/plugin or exposed through an
MCP server, but both must call the same compiler-backed service. They should
help an agent retrieve the right context on demand, not inject the entire
language manual into every prompt.

Agent-facing evaluation should measure whether a fresh model can:

1. identify the language and use its canonical fenced-block identifier;
2. make a requested edit using only retrieved relevant documentation;
3. use compiler diagnostics to repair invalid source; and
4. avoid editing compiler-owned generated files.

## 6. Delivery stages and gates

### DX0 — presentation baseline

- choose the stable machine language/file identifiers;
- ship file association, comments/brackets/folding, TextMate highlighting, and
  fenced-snippet fixtures;
- prove one editor and one renderer adapter display the canonical examples; and
- document plain-text fallback behaviour.

### DX0.5 — checked local development loop

DX0.5 turns the existing manual `check`, `build`, and Bun commands into the
canonical edit-feedback-run loop without adding a second semantic path:

- `jadpo check <project> --diagnostic-format=json` emits a versioned
  machine-readable envelope containing severity, stable code, message, source
  path, byte range, and notes; the existing human-readable format remains the
  default;
- `jadpo watch <project>` watches authored `.jadpo` files and other checked
  project inputs, coalesces filesystem events, and reruns the same authoritative
  check/build pipeline used by one-shot commands;
- watch output distinguishes checking, failure, successful generation, runtime
  restart, readiness, and shutdown. Lifecycle events use the same versioned
  structured envelope when JSON output is selected, so editors and agents do
  not need to infer state from prose;
- `jadpo dev <project>` performs an initial checked build, launches the
  generated Bun server, and automatically restarts or reloads it after each
  successful rebuild;
- generated output is staged and promoted atomically. A failed check or build
  never exposes a partial target and never replaces the last successful build;
- while a new edit is invalid, the last-known-good Bun server may continue
  serving, but the tool must visibly report that the running revision is stale;
- a successful revision becomes live only after Bun starts and reaches its
  configured readiness boundary. Failed startup preserves or restores the
  last-known-good process when possible and reports a structured runtime event;
- initial implementation may use a graceful Bun process restart. In-process
  hot module replacement is not required for DX0.5; observable server state
  must nevertheless advance automatically after a successful compile; and
- shutdown cleans up the watcher and child process without leaving an orphaned
  listener.

The first two DX0.5 slices are implemented. Passing `--diagnostic-format=json` to
`jadpo check <project>` writes exactly one version-1 `diagnostic_report`
object to standard output and uses the ordinary success/failure exit code. The
envelope records `command`, `status`, the supplied project path, optional source/
declaration/node counts, and an ordered `diagnostics` array. Every diagnostic
contains `severity`, stable `code`, `message`, nullable `source` and byte
`range`, and `notes`. Failed frontend checks retain the same phase ordering as
the human CLI and include the terminal `JADPO_*_FAILED` diagnostic. Successful
checks may contain warnings. The default human format and compiler pipeline are
unchanged semantically; human output now resolves byte ranges to line/column,
prints the relevant source line and caret, and retains bounded repair notes.

`jadpo watch <project>` now performs an immediate checked build and then
fingerprints authored `.jadpo` files plus `schema.identities.json`. It ignores
`build/` and compiler staging/backup directories, polls every 100 ms, and waits
for a 75 ms quiet window before treating rapid writes as one revision. Each
revision uses the same check, target derivation, and artifact-writing functions
as one-shot commands. Output is completed in a sibling staging directory before
directory promotion; a staging or promotion failure preserves or restores the
last complete `build/` revision. Human events and version-1 JSON
`lifecycle_event` objects distinguish checking, build success, build failure,
watch-input failure, monotonic sequence/revision IDs, and whether the retained
build is stale. Source creation/deletion and registry edits participate in the
snapshot. Runtime launch/readiness and `jadpo dev` are now implemented for
the initial process loop. `jadpo dev <project>` layers Bun execution on
the same loop. `PORT` defaults to 3000 and is validated before the initial
build. Every generated target has a compiler-owned `GET /health` endpoint unless
an authored public route replaces it. Bun starts with package installation
disabled; readiness requires HTTP 200 from that endpoint within three seconds,
and unexpected runtime exit is detected while waiting for edits. Invalid source revisions leave an already-ready process
running and report the retained revision as stale. A successful revision emits
build success, stops the prior process, launches the new target, and emits
runtime starting/readiness or failure. JSON mode keeps lifecycle objects on
standard output and diverts Bun standard output to standard error. A real
generated server passed readiness and an HTTP request on Bun 1.2.20, and
terminal interruption left no listener. Generated runtime and startup faults
now use compact versioned JSON diagnostics; raw generated-target stacks are
available only through the explicit `JADPO_DEBUG_TARGET_STACKS=1` debugging
escape hatch. Startup-failure rollback to the prior generated/runtime revision,
an explicit portable shutdown event, and the full edit/recovery protocol suite
remain.

Protocol-level acceptance tests must cover a valid edit, an invalid edit,
recovery to valid source, rapid/coalesced writes, source creation/deletion, a
generated-target failure, Bun startup failure, last-known-good continuity, and
clean shutdown. The watcher must ignore compiler-owned `build/` output so its
own writes cannot create a rebuild loop.

DX0.5 is a prerequisite for P11 application authoring. The later VS Code client
and DX1 language server must consume the same structured diagnostics and state
model rather than scrape terminal prose or create an editor-only watcher.

### DX1 — compiler-backed LSP

- diagnostics have byte/range/code/message parity with the CLI;
- definitions, references, symbols, hover, and completion work across files;
- semantic tokens refine rather than contradict lexical scopes; and
- the seed applications are covered by protocol-level tests.

**Status: complete for the initial language surface.** `jadpo lsp` implements
standard Content-Length-framed JSON-RPC over stdio with full-document sync.
Compiler byte ranges are converted to LSP UTF-16 positions, including non-ASCII
source. Protocol tests cover live malformed unsaved text, tagged variants and
payload symbols, contextual enum/record completion, callable signature help,
rename/reference sets, semantic tokens, authored tests, and imported definitions
across files. Schema-identity and index-advice diagnostics reuse the same
compiler checks after a valid frontend pass. Initial declaration-to-artifact
links cover validation plans, callable inventories, failure audits, and OpenAPI.
Diagnostic documentation remains DX2 work rather than editor-owned semantics.

### DX2 — agent and documentation integration

- diagnostic documentation is searchable and versioned;
- a compact skill/plugin or MCP interface retrieves compiler-backed language
  facts on demand;
- fenced code output and source-file links are evaluated in supported clients;
  and
- failures in unsupported clients degrade to exact readable plain text.

### P12 tooling freeze gate

Before freezing the language-versus-TypeScript experiment, record exactly which
editor, LSP, formatter, grammar, agent-context bundle, and renderer support each
side receives. Missing language tooling is a real product cost and must not be
hidden, but the comparison must not change tooling halfway through a run.

## 7. Explicit non-goals for the first tooling slice

- maintaining editor-specific semantic implementations;
- claiming highlighting support in clients that cannot install the grammar;
- using a vaguely similar existing language tag and presenting the result as
  correct highlighting;
- model fine-tuning before retrieval plus compiler feedback is measured; or
- allowing IDE convenience actions to bypass compiler checks or policy review.
