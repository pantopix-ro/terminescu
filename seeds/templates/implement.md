---
key: implement
name: Implement the plan
variables: [threshold]
---
Proceed with implementation.

If the plan is multi-phase: after each phase review the implemented code. Check for issues,
risks, problems, and fix them. Assess confidence to merge the code in percentage and report it
with the `report_confidence` tool. If the percentage is >= {{threshold}}, proceed to the next
phase until completed; otherwise fix and reassess.
