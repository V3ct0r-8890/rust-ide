# Token usage report

`tools/usage_report.py` shows where a Claude Code session's tokens went, as a text bar chart.
It only reads the session transcript, uses only the Python standard library, and works on
Windows, Linux and macOS.

## Use it on another machine

1. Copy `tools/usage_report.py` (this one file) to the machine.
2. Run it with Python 3.8 or newer:

```bash
python -I usage_report.py                      # newest session of any project
python -I usage_report.py --project Rust-IDE   # newest session whose folder name contains this
python -I usage_report.py path/to/session.jsonl
```

Transcripts are in `~/.claude/projects/<project-folder>/<session-id>.jsonl`
(Windows: `C:\Users\<you>\.claude\projects\...`). The Claude desktop app may keep them under its
own profile folder; pass the file path in that case.

## How to read it

Cost is weighted, not raw: a cache read counts 0.1, a cache write 2, an output token 5, compared
with 1 for a normal input token. The totals are exact. The split between groups is an estimate:
each request's cost is shared among the parts of the conversation it had to read or write.

Groups: instructions (system prompt and tool list), your messages, Claude's thinking and replies,
writing/editing files, running commands, reading files, screenshots, web research, subagents,
Claude in Chrome, connectors (one line per connector), task-list tools, system reminders.

The last line, "By kind", is the most useful number:
- **re-reading history** high (70% or more): the session is too long. Start a fresh one.
- **output** high: Claude writes a lot (big file rewrites, long thinking).
- **screenshots** high: ask for text checks instead of looking at the screen.

## Example (the Rust-IDE build session, 503 requests)

```
Writing and editing files      ##############################  26.1%
Running commands               ######################          19.9%
System notes and reminders     #####################           18.9%
Claude's thinking and replies  ############                    11.2%
Instructions                   ##########                       9.0%
Screenshots (images)           #########                        8.0%
By kind: re-reading history 73.3%, output 10.5%, new context written 16.2%
```

## Ways to use fewer tokens

- One backlog item per session; keep `CLAUDE.md` and `docs/ROADMAP.md` current so a new session
  starts quickly.
- Prefer tests and text output over screenshots.
- Ask for small edits instead of whole-file rewrites.

## As a Claude skill

The built-in `explain-usage` skill does the same analysis on request ("show me where this
session's tokens went"). This script is the same idea as a standalone file you can keep and run
anywhere, including outside Claude.
