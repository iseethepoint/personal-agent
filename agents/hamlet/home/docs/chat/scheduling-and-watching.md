# Scheduling, monitoring, and services

Use this guide from main or side chat when creating or changing background work.

## Choose the mechanism

- Crons run agent tasks on a schedule, including reminders, periodic checks, and work that needs tools or judgment. Use runonce for a single future action and an interval for bounded repeated checks.
- Hooks run lightweight polling scripts and wake an agent on a relevant condition. Use them when the detector can check local or public data without an agent turn each time.
- systemd services supervise programs that need to remain running, such as stream listeners, collectors, or local servers. Use the runtime's guest systemd when a sequence of independent checks cannot do the job.

Browser tasks and subagents already deliver completion handoffs. Use those instead of adding another monitor.

## Saved jobs and hooks

Record the user's scope, target, cadence, completion condition, owner, and delivery destination. A request to monitor one operation covers the observations needed to finish, not repeating the operation. Reuse an existing goal as owner when applicable. Other reminders and follow-ups need a tracked owner.

Keep goal-owned working files in `~/workspace/goals/<goal-slug>/hidden_files/`, including checkpoints, run logs, watermarks, and source snapshots. Hook scripts and state use the hook paths below.

Check existing jobs first. Change a cron through `cron.view` and `cron.update` on the same id, supplying the complete revised body and preserving unrelated settings. Read it back before confirming. Projected schedule files are read-only copies; editing them does not change the job. Removing and recreating a job is not a repair for its permissions.

Each check should finish, have request and execution timeouts, and avoid overlap. The body describes the work and reporting intent, not calls to terminal handoff tools. Stop finite monitoring when its outcome is resolved, cancelled, or past its deadline. Keep any still-owed delivery separate from further source polling.

Hook scripts belong under `~/hooks/scripts/`, with state under `~/hooks/state/`. Follow the tool's script protocol, including one terminal `silent` or `wake` decision. Use the provided state helpers so dry runs do not consume detections. New hooks start disabled. Inspect a `hooks.dry_run` result before `hooks.enable`. Use `disable_after_run` for a terminal condition. Hooks have no connector credentials. Do not add secrets to a detector.

Both mechanisms poll. Changes can be missed between checks, and schedules can run late. They do not provide continuous observation or block source events.

## Verification and delivery

Check saved configuration, actual execution, useful progress, and delivery separately. For crons, use `cron.status` and `cron.runs`, then verify that any owed result reached the conversation. For services, inspect the manager and unit, dependencies, logs, and recent output. An active unit or existing PID alone does not establish health.

Test the failure mode you claim to handle. Restarting a worker with its manager and bus intact tests process supervision. Recovery after replacement also requires restoring those prerequisites and starting the work again. Do not infer the latter from the former. Use observed deployment evidence or an authorized recovery test, and name what remains unverified.

Report component states separately when recovery partially succeeds. A failed bootstrap does not prove that every service is down. A quiet listener does not prove a crash. Preserve rate-limit stops, denied access, and uncertain external outcomes rather than retrying them as setup failures.

Deliver results to the originating chat unless the user chose another allowed destination. Provider-connected side chats retain their exact-chat boundary. Keep unchanged observations quiet unless requested, and report failures or gaps that affect the user's outcome. A chat result or push notification is not an arbitrary SMS or a message to another person.
