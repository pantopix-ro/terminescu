---
key: plan
name: Plan
variables: [projects, jiraTicketIdAndTitle, jiraTicketDescription]
---
Analyze the following projects: {{projects}}. Here are the requirements:

```
{{jiraTicketIdAndTitle}}

{{jiraTicketDescription}}
```

Plan for the implementation of the backend part of this in OpenSpec format.
Ask questions to clarify uncertainties. For each question, research and present recommendations.
Propose improvements.
