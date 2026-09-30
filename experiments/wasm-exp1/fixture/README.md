# Frozen WASM-EXP1 source

`app.jadpo` is the common source for the Bun baseline and both compilation
probes. The adjacent `../acceptance.json` records exact seed data, probe cases,
the A01–A18 full-slice mapping, source digest, and required checked facts.
Cases are preregistered assertions, not completed runtime validation.

Normal compiler commands, from the repository root:

```sh
jadpo/target/debug/jadpo check experiments/wasm-exp1/fixture
jadpo/target/debug/jadpo build experiments/wasm-exp1/fixture
```

At checkpoint `4dc5604`, both pass: one source file, 15 declarations, 551
semantic nodes, and 18 generated files. Generated output remains under the
ignored `build/` directory and must not be repaired by hand.

The shared probe reaches `probe → Item.read_title → Item.read`. It validates
nominal constrained text and an optional nullable input, suspends at a named
authoritative read, returns the stored title on success, and recovers only
declared `ItemMissing` by returning the input fallback title. The optional
note does not affect the probe result. Full-slice `Item.change` separately
proves omission preserves a stored note and explicit null clears it.

`update_pair` performs two sequential renames in one checked atomic boundary.
A missing second row or duplicate second title must roll back the first
write. Nested handled mutation/savepoint recovery is outside this slice.

`User` is a nonpersistent principal type. Only Item rows are stored. The owner
role derives from authoritative `Item.owner_id`; fixture principals enter
through the trusted test harness, never an HTTP body or header. There are no
routes or authentication adapters. Generated Bun callables are internal to
`target/app.ts`, so baseline access needs a recorded trusted test adapter
that exposes their existing behavior without rewriting application logic.

Canonical `identity: id` clauses are essential here: the initially tried
legacy field modifier checked, but left entity/transaction audit models
empty. The frozen source uses canonical clauses, and both models are now
populated. Lowering must consume the complete checked project and preserve
nominal refinements, presence/nullability, failure sets, authoritative query
plans, propagated policy obligations, atomic plans, suspension facts, source
ranges and semantic operation identities.
