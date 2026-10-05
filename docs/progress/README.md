# Daily project progress

Open [the latest report](latest.md). Dated Markdown reports and matching JSON
snapshots live here. The report is a short view of recorded project state, not a
second backlog. The daily refresh is scheduled in the reporting chat for 09:00
Europe/London. It reports progress; it does not implement roadmap tasks.

## Produce the report

Read the current roadmap, completion history, saved task plans and recent relevant
validation evidence. Write a temporary JSON file with four editorial fields:
`summary` (one or two simple sentences), `delivered` (up to three short bullets
with source links), `next` (the next milestone) and `input_needed` (the specific
current decisions, or none recorded). Links are relative to this directory.
Explain what users can now do, not a list of modified files. Distinguish delivered
capabilities from partial/source-only results. Refresh these sentences each run;
do not carry forward old numbers or claim a fresh test run from an old report.

Run `python3 tools/project-progress.py --notes /absolute/path/to/notes.json`.
The command computes task counts and epic estimates, reads explicit planning
states, compares with the most recent earlier daily snapshot and writes today's
report plus `latest.md`. Same-day refreshes replace today's snapshot and continue
to compare against an earlier day. Missed days remain missing; comparisons state
the actual previous date. Do not invent historical daily observations.

Review the generated Markdown, numbers, source links and narrative before sharing
it. If concurrent work changes inputs, refresh from current evidence and rerun.
Do not launch expensive test suites merely to generate a status report. If source
formats become unrecognisable, repair the reporter or report the failure instead
of emitting silently incorrect numbers. The JSON preserves source hashes, task
IDs, recorded states and comparisons for auditability.

## Maintain the source records

- The active roadmap owns open task IDs, estimates, dependencies and execution
  state. A `Ready` row is reported as marked ready, not independently certified.
- When a task's full acceptance conditions are met, its dated history entry must
  contain `**Completed task:** RM-xxx` and evidence links. Partial sessions do not
  get that marker. Existing older capabilities remain qualitative history and
  are not mixed into the new task-count denominator. No whole-project percentage
  is inferred from this incomplete historical inventory.
- Record planning state in the existing work-plan table, using `Task ID` and
  `Planning state` columns. Valid values are `planned`, `needs planning`,
  `needs input` and `unassessed`. Reconcile contradictory records when revising a
  task plan. A file mention or accepted design document alone does not establish
  task-level planning status; assess it before recording the result.
- Open IDs without a planning record are **unassessed**, not automatically missing
  a design. Planning state and implementation readiness are separate dimensions.
- Removal from the roadmap is not completion evidence. Daily comparisons separate
  newly completed, added/reopened and removed/reclassified IDs. If history and the
  active roadmap overlap, report the item as open and flag the overlap.
- Estimates are full forecasts for open tasks, not measured remaining effort on
  partial work. Show each epic's own non-conditional effort and unestimated work;
  new intake items can use `Unestimated (needs planning)` until assessed.
  Exclude conditional tasks from that effort subtotal but retain their counts.
  Do not turn summed effort into a calendar delivery date or import engineer-day
  assumptions. External waiting and shared prerequisites remain separate.

This report does not require any changes to model settings, a Goal, another agent
or another chat. Scheduled runs should deliver a short link to the updated daily
artifact; keep implementation work in its existing workflow.
