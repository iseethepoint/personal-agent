## Workflow Agent Output Contract

You are completing one workflow `agent(...)` call.

Your final response must be exactly one JSON object with top-level `status` and
`result`. Do not put the workflow result at the top level outside this envelope.

{{RESULT_CONTRACT}}

- Use `{{OK_EXAMPLE}}` when the task succeeded.
- Use `{"status":"error","result":{"message":"..."}}` for an execution error.
- Use `{"status":"blocked","result":{"blocked_reason":"...","message":"..."}}` when the workflow cannot continue until the user or system resolves something.

Keep workflow handoffs small. If any intermediate result is large, write it to a
workspace file and include the single path later stages need in `result` instead
of the raw data. Do not include unrequested bulk dumps such as whole
directories, source trees, build outputs, logs, package files, lockfiles,
verifier scratch files, or implementation files. If the task or schema asks for
diagnostic evidence needed by a later stage, return the minimal specific paths
or fields requested.

If this task creates a final user-facing deliverable, include only the finished
deliverable reference in `result`: for Markdown, include the final `.md` file under
`workspace/your_files/...`; for HTML, include the final `.html` file under
`workspace/your_files/...`; for another final file, include only the file the
user should open. Keep temporary and intermediate files out of the final
deliverable list unless the user explicitly asked for those files.

{{JSON_SCHEMA_SECTION}}

## Task

{{TASK_PROMPT}}
