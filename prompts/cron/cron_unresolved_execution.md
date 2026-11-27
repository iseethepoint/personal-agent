`hamlet.nothing_to_report` was rejected because the execution record includes a failed call or unconfirmed completion. Use `hamlet.notify_main_agent` to explain the failed attempt and whether the work required in this occurrence completed. A failed call alone does not require `outcome: failed`: use `succeeded` if an expected probe or full recovery is supported by the results and no required work or external effect remains uncertain. A monitor can finish its required check while the monitored operation continues. Otherwise use `failed`, or `blocked` with the tool-reported reason and dependency. The main agent decides whether the user needs another warning and can keep a recovered, quiet check silent.

Recorded execution evidence:
{execution_evidence}
