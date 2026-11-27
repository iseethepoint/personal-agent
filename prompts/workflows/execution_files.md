## Workflow execution files

Shared workflow working directory: `{{WORKFLOW_WORK_DIR}}`
- Exec commands start in this directory unless you specify another working
  directory. File tool paths resolve relative to `~`; use an explicit
  `workspace/...` path for files in the real workspace.
- Prefer the shared working directory for helper scripts, source files, raw
  data, drafts, intermediate notes, downloaded inputs, generated media,
  renders, logs, tests, verifier files, and other non-final output, especially
  files needed by later commands or workflow stages. `/tmp` is also acceptable
  for ephemeral files that do not need to persist or be handed to another
  stage.
- This guidance is authoritative for workflow support files even when the task
  below or a loaded skill suggests `.src/`, build, media, or validation paths
  beside a final deliverable. Put those support files in the shared working
  directory instead.
- Write only finished file deliverables that the user should keep, open,
  or download under `workspace/your_files/<short-slug>/`.
  That final deliverable directory should contain only those finished
  user-facing files.
- A file is final only when the user outcome explicitly asks
  for that file, or the user must directly open, read, use, download, or share
  it. A task instruction that names a support-file path under
  `workspace/your_files/` does not make that file final. Helper code,
  generated source, raw data, configs, cron or job definitions, operational
  notes, README files, tests, and verification evidence stay in the shared
  workflow working directory unless the user explicitly asked to receive that
  exact file.
- If the task is satisfied by chat, a schedule, or a connector action, leave
  `workspace/your_files/` untouched.
- In the final structured result, refer to a finished file with its canonical
  `workspace/your_files/...` path.
- You may write or edit the real workspace when completing the task genuinely
  requires it. Do not treat ordinary workflow support files as workspace
  deliverables.
- Native product tools may manage their own durable directories, such as
  scheduler storage. Do not place unrelated support files in those
  directories. Hidden implementation files that must persist after
  this workflow may remain in the shared workflow working directory.
- You may return the minimum path inside this directory required by the result
  schema or needed by a later workflow stage. Such paths are internal workflow
  handoffs, not user-facing deliverables.
- Never expose the workflow working directory or anything inside it in the
  workflow's final user-facing handback.
