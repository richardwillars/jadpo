# Jadpo repository workflow

For roadmap work, read [the project workflow](docs/roadmap-workflow.md).
Use the personal `adaptive-delivery` skill for planning/execution and
`roadmap-tidy` for roadmap restructuring when available in the skill catalog.
Use `roadmap-add` to capture new ideas in the existing roadmap without starting them.
For sustained measurement-driven improvement, use `verify-loop` with its target
and execution-model checks. Recommend a bounded Goal for unattended delivery;
create one only when explicitly requested and reuse a compatible active Goal.
The project workflow supplies Jadpo's priorities, paths and acceptance gates;
the skills own the reusable procedure. Do not copy their full rules here.

## Request routing

- **“Plan the next batch”**: prepare 3–5 useful tasks with proportional planning
  and model recommendations; stop before implementation.
- **“Plan tasks in a loop until you need me”**: repeatedly save and check plans
  within scope; replenish the working window without starting implementation.
- **“Implement tasks in a loop until you need me”**: complete eligible tasks,
  verify and record each outcome, then continue without per-task approval.
- **“Work through the roadmap” / “continue the roadmap”**: select eligible work,
  plan as needed, implement, verify, record evidence, then continue within scope.
- **“Implement RM-xxx”**: focus on its acceptance conditions and prerequisites.
- **“Review RM-xxx”**: review requirements, changes and evidence; fix only if asked.
- **“Tidy the roadmap”**: reorganise remaining work and history without starting it.
- **“Add this to the roadmap” / “queue this idea”**: use the current conversation,
  check overlap, add or update the appropriate task and record planning needs.
- **“Create a Goal to…”**: establish the requested outcome and evidence, then use
  supported Goal controls; do not mistake discussion of Goals for activation.
- **Status/workflow questions**: report or explain; do not start a work loop.

These are examples, not exact commands. “Continue” retains current authorised
scope. While RM-213 remains open, start unscoped roadmap work with its documentation audit,
then prioritise the golden application's dependencies. Preserve user-selected
models and effort. Pin the lowest settings sufficient for the hardest expected
work in a bounded batch. Adaptive-delivery checks that pin at batch entry,
including unnecessary capability or effort, then retains it through ordinary
task and stage transitions. Reassess only at a batch boundary, material scope
change or evidence that the pin is insufficient.
Recommendations do not themselves change settings. Persistent
Goals, delegation, other-chat messages and publishing require their own existing
authorisation; invoking a skill does not supply it.

For either loop, surface concrete questions once and continue independent work
unless asked to stop at the first question. Stop at scope completion, a specified
limit or when no eligible work remains; follow host Goal lifecycle rules. Keep
planning status separate from execution readiness and completion.

For task timing use [the local recorder](docs/task-timing/README.md), including
actual model/effort when known. Preserve missing values and original estimates.
Keep explicit planning states in the work-plan table and add the completed-task
marker only when closing a fully evidenced task, following the
[daily reporting rules](docs/progress/README.md). Reports read these records;
do not count a partial session or an unassessed plan as completion.
If a skill is unavailable, follow the project workflow's fallback and report the
limitation; do not halt otherwise actionable work merely to install a skill.

For documentation, follow [ownership and maintenance](docs/README.md#document-ownership-and-maintenance).
Update the owning document and link it; keep transient narration in chat and raw
results in their evidence location. Preserve frozen contracts, decision rationale
and historical links. Do not duplicate skills or create a new document per turn.
