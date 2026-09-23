---
name: ctx-sync
description: >
  Coordinate shared development context between multiple sandbox
  agents working on the same project. Use when starting work,
  joining ongoing work, making architecture decisions, handing work
  off, or finishing work in a project configured with ctx-sync.
---

# ctx-sync

This project uses ctx-sync to coordinate work between development agents.

## Starting work

Before modifying the project, run:

    ctx-sync agent start

Read the returned shared context before making changes.

Check:

- current architecture
- accepted decisions
- active workers
- interface changes
- attention items

Avoid overlapping another active worker's work unless necessary.

## During work

When making a durable project-level design or architecture decision, record it with:

    ctx-sync decision add ...

Do not store:

- chain-of-thought
- temporary reasoning
- debugging transcripts
- speculative notes

Only store durable facts, decisions, constraints, work state, and information needed by other workers.

## Before finishing

Run:

    ctx-sync agent finish ...

Include:

- what changed
- important files changed
- public/interface changes
- unresolved problems
- anything another worker needs to know

## Joining an existing project

`ctx-sync agent start` must retrieve the current context before work begins.

Do not assume previous sandbox conversation history is available.

## Troubleshooting

- If `ctx-sync agent start` says the worker is not registered, run `ctx-sync agent start --name <short-name>`.
- Exit code 2 is a context conflict. Do not resolve it yourself; report it to a human with the output of `ctx-sync status`.
- Exit code 3 is an authentication error. Ask a human to fix git / GitHub authentication.
- Exit code 4 means the project or worker is not configured. Follow the message, or ask a human to run `ctx-sync init` or `ctx-sync attach <gist>`.
- Exit code 5 is a remote error. Retry once; if it keeps failing, report it.
- Never edit another worker's `40-worker-*.md` file or an existing `30-decision-*.md` file.
