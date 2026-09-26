# Diagnostic conformance suite

The DX2 presentation contract is tested as one semantic diagnostic projected
to agents, terminals, and IDEs. The suite deliberately does not approve a
snapshot merely because it is stable.

Run the active conformance suite with:

```text
cd jadpo
cargo test --workspace
cargo build -p jadpo-cli
cd ..
ruby tests/compile/verify.rb
cd editors/vscode
npm test
```

Coverage is split by responsibility:

- `jadpo-diagnostics` enumerates every compiler-owned public code and validates
  the complete version-2 agent schema, bounds, repair objects, impact, aliases,
  JSON validity, and secret/ANSI exclusion;
- `jadpo-cli::terminal` renders every catalogue entry through plain and rich
  modes and proves that neither projection loses the shared explanation,
  choices, impact, rule, or help identity;
- `jadpo-cli::lsp` renders every entry through the IDE hover projection and
  exercises Problems, UTF-16 ranges, Quick Fix ordering, revision checks, and
  edit previews through scenario tests;
- `editors/vscode/test` verifies that Problems uses only the human summary and
  that the details view preserves and safely escapes the complete shared repair
  protocol; and
- compile fixtures prove exact root-diagnostic sequences and byte ranges. A
  fixture may additionally assert the exact summary, reason fragment, repair
  classification, owner, context, alternatives, replacements, and impact.

`ROUTE_AUTH_VALUE_INVALID` is the first complete golden scenario. Its fixture
proves one root error over `nonke`, continued parsing of subsequent route items,
two non-preferred human-owned security choices, and parity across JSON,
terminal, Problems, hover, details, and Quick Fix.

The catalogue currently contains explicitly classified copy debt inherited
from the original identifier-derived fallback. The normal suite rejects any
entry falsely labelled as authored and prevents format regressions for every
entry. The stricter completion gate is intentionally visible as the ignored
Rust test
`strict_public_catalogue_has_no_placeholders_and_every_code_has_a_real_fixture`.
Run it explicitly with:

```text
cd jadpo
cargo test -p jadpo-diagnostics strict_public_catalogue_has_no_placeholders_and_every_code_has_a_real_fixture -- --ignored
```

It must be made non-ignored, and must pass, before DX2 can be called complete.
This red gate is the work queue for replacing every remaining generic message
with rule-specific copy and a real triggering fixture.

The `SYN_*` catalogue family is fully authored. Its public copy now describes
the concrete grammar rule and a usable next step; the general parser expectation
also carries the bounded `expected` and `found` facts supplied by the parser.
Real trigger-fixture coverage is still tracked independently by the strict gate.

The `SEM_*` name-resolution family is also fully authored: duplicate names,
unresolved names and callees, wrong declaration kinds, and non-callable names
now explain resolution scope and the concrete declaration change required.

The `EFFECT_*` and `FAIL_*` families are fully authored and mostly backed by
existing compiler fixtures. Their copy explains the function/action purity
boundary, exact failure-set propagation, failure-kind mappings, and flat
public/internal context rules.

The `ROUTE_*` family is fully authored. It covers authentication ownership,
exactly one behavior form, consistent item separators, duplicate items, and the
one-to-one typed relationship between URL placeholders and `path:` fields.
