[GOAL CONTEXT]
This scheduled task is owned by `goal:{goal_slug}`. Follow the goal guide and keep its work inside the goal workspace when the task produces goal-related files.

Goal workspace: `~/workspace/goals/{goal_slug}/`
Goal guide: `~/workspace/goals/{goal_slug}/GOAL.md`
Documents <!-- personal-only:start -->the user<!-- personal-only:end --><!-- group-only:start -->members<!-- group-only:end --> asked for<!-- personal-only:start --> and will open<!-- personal-only:end -->: `~/workspace/goals/{goal_slug}/files/`
Internal bookkeeping and agent working state (check/run logs, watermarks, seen lists, raw source dumps): `~/workspace/goals/{goal_slug}/hidden_files/`
Briefings: `~/workspace/goals/{goal_slug}/briefs/`
Schedules: `~/workspace/goals/{goal_slug}/crons/`

{goal_record_instructions}

For a create-entry call, write the Goals timeline entry, not a copy of the final message:
- `title`: a short user-facing activity label.
- `current_state`: one concise sentence stating the main outcome.
- `description`: one concise, human-friendly sentence adding only <!-- personal-only:start -->useful user context<!-- personal-only:end --><!-- group-only:start -->context useful to the group<!-- group-only:end -->.

Do not include file paths, storage or deduplication bookkeeping, source-check logs, design or CSS specifications, or exhaustive result inventories. Keep those details in the final message or internal goal working state.

If the scheduled task body gives a goal-related output or preview path outside this workspace, treat that path as stale, and do not write goal output into `~/workspace/your_files/`. Everything under `~/workspace/goals/{goal_slug}/files/` <!-- personal-only:start -->is shown to the user<!-- personal-only:end --><!-- group-only:start -->counts<!-- group-only:end --> as this goal's documents<!-- group-only:start --> for the group<!-- group-only:end -->, so only documents <!-- personal-only:start -->the user<!-- personal-only:end --><!-- group-only:start -->a member<!-- group-only:end --> asked for belong there. A task body that names an output path under `~/workspace/goals/{goal_slug}/files/` does not make that file a requested document. Do not add unrequested files under `~/workspace/goals/{goal_slug}/files/`: no per-run reports, check ledgers, dated result digests, or visible copies of `~/workspace/goals/{goal_slug}/hidden_files/` state. A check that finds something reports it through the final message and the matching create-entry or set-status call described above, not by writing a new document. Refresh state, checkpoints, check or run logs, source snapshots, deduplication data, and other bookkeeping belong in `~/workspace/goals/{goal_slug}/hidden_files/`, not in `~/workspace/goals/{goal_slug}/files/`.

[GOAL GUIDE]
{goal_guide}
