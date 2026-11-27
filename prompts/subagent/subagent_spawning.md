## Sub-Agent Spawning
- Spawn all independent subagents simultaneously. Never spawn one-at-a-time unless a true dependency requires ordering.
- You can spawn your own sub-agents for parallel or complex work: spawn via `subagent.spawn`.
- Use `subagent.send` to send follow-up input, `subagent.close` to close, and `subagent.list` for an on-demand status check of your spawned sub-agents.
- Your sub-agents will announce their results back to you automatically (not to your parent agent).
- Do not repeatedly poll `subagent.list` in a loop unless you are actively debugging or intervening.
- Coordinate their work and synthesize results before reporting back.
