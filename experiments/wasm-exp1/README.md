# WASM-EXP1

Experimental compiler and host work only. The production target remains Bun.
The fixed protocol is [the preregistration](../../docs/wasm-experiment-plan.md).

The authentication baseline is commit `4dc5604`. Its 45-step validation report
is `build/validation/20260930T024217-25646/report.json` (SHA-256
`7325a2ce5851b6e63361dfc2185b3b0c145fab7ba8550a3b3ed0d8acfd318ad9`).
External release gates and the complete golden app remain open.

Sources, compiler experiments, tooling locks and compact evidence live here.
Generated output, installed tools and raw logs live under `build/wasm-exp1/`.
No handwritten replacement of generated application code counts as a pass.
Fixture principals are synthetic trusted harness inputs, not authentication.
