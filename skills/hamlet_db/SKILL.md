# Hamlet database

Use `hamlet.db` to read Hamlet's own records with one PostgreSQL `SELECT` when a purpose-built tool does not expose what you need: missing or orphaned records, execution history, inconsistencies, or relationships across memories, scheduled work, goals and workflow runs.

1. Read `references/schema.md` for the tables, columns and keys. Use schema-qualified names exactly as listed.
2. Read `references/querying.md` for the accepted functions and cast types and the query rules.
3. Start narrow: filter by the id, key or time range you are tracing, select only the columns you need, and alias every column to a unique name.
4. If the tool rejects a function, cast or clause, rewrite the query with the listed operations instead of concluding the records are missing.
5. Prefer the purpose-built tools for ordinary reads and every change; `hamlet.db` only reads.
