# ctx-sync

## What is ctx-sync?

ctx-sync is a CLI that shares only the development context other sandboxes and AI coding agents need. It stores that context in a GitHub Gist and synchronizes it through Git.

## Why does it exist?

Separate sandboxes do not know each other's conversation history. ctx-sync shares the facts they need to coordinate: decisions, current work, changes, and anything another worker should notice. It is neither an orchestrator nor a project management tool.

## Installation

```bash
cargo install --git https://github.com/ieee0824/ctx-sync ctx-sync-cli
```

`git` is required. The optional `gh` CLI is needed only to create a Gist with `init --create-gist`; attaching to an existing Gist uses Git.

## Quick start

```bash
ctx-sync init --project example --create-gist --secret
ctx-sync register sandbox-a
ctx-sync agent start

# work

ctx-sync agent finish --summary "Implemented feature A"
```

## Multiple sandbox example

**New project.** Run the Quick start in sandbox A, then commit the generated `.ctx-sync.toml` with your project so other sandboxes can find the Gist. Each worktree registers its own worker identity.

**Joining an ongoing project.** In a fresh checkout that contains `.ctx-sync.toml`, run:

```bash
ctx-sync agent start --name sandbox-b
```

`agent start` attaches this machine if needed, registers sandbox B, pulls the latest context, and prints onboarding information. If the worktree is already registered, `ctx-sync agent start` is enough.

**Adding ctx-sync to an existing repository.** Preview the context that can be collected from the repository, then initialize and review it before publishing:

```bash
ctx-sync bootstrap
ctx-sync init --project existing --create-gist --secret
ctx-sync register sandbox-a
ctx-sync bootstrap --apply
ctx-sync status
ctx-sync sync
```

`bootstrap --apply` adds candidate facts to the project document without syncing them; review that document in the context repo shown by `status` before `sync`.

For a handoff, sandbox A runs `ctx-sync agent finish --summary "Implemented feature A"`. Its worker update is synchronized to the Gist. Sandbox B then runs `ctx-sync agent start` and receives that update in its onboarding context.

## How context is divided

| Layer | Shared? | Contents |
| --- | --- | --- |
| Project | Yes | Goals, architecture, and accepted decisions. |
| Coordination | Yes | Worker status, handoffs, changes, and attention items. |
| Local | No | Conversations, reasoning, experiments, and temporary debugging notes. |

Write concise facts that another worker can act on. Keep private working context local.

## Gist storage model

The Gist is a flat Git repository:

| File | Purpose |
| --- | --- |
| `00-meta.json` | Project identity and schema version. |
| `10-project.md` | Project goals and shared project context. |
| `20-architecture.md` | Current architecture. |
| `30-decision-<date>-<number>-<slug>.md` | One decision per file. |
| `40-worker-<short-id>.md` | One worker's current state and handoff. |

Decisions are append-only: add a new decision to supersede an old one. Each worker owns its own worker file. Synchronization uses Git's optimistic concurrency; a conflicting edit stops for manual resolution rather than being silently overwritten.

Local state, including the Gist clone and worker identity, lives outside the project. Set `CTX_SYNC_HOME` to override its location. By default it is under `~/.local/state/ctx-sync` on Linux or `~/Library/Application Support/ctx-sync` on macOS. Use `ctx-sync status` to find the context repo.

## Skill usage

The [Codex skill](.codex/skills/ctx-sync/SKILL.md) gives agents the operating rules for starting, recording decisions, handing off work, and finishing. It uses the CLI to do the work; it does not implement synchronization itself.

## Security warning

A secret Gist is readable by anyone with its URL. Do not treat it as a secret store. Never write credentials, tokens, API keys, environment secrets, or private keys into shared context.

`handoff` and `decision add` warn when they detect obvious credential patterns such as `sk-`, `ghp_`, `github_pat_`, or a `-----BEGIN ... PRIVATE KEY-----` header. The warning does not replace reviewing what you share. `.ctx-sync.toml` does not store credentials, and unknown configuration keys are rejected when it is read.

## Command reference

| Command | Purpose and main arguments |
| --- | --- |
| `ctx-sync init` | Create project context with `--project`, `--create-gist` or `--gist-id`, `--secret` or `--public`, and optional `--ssh`. |
| `ctx-sync attach <gist-id-or-url>` | Attach this checkout to an existing context Gist; use `--ssh` for SSH. |
| `ctx-sync bootstrap` | Preview context candidates from an existing repository; `--apply` writes them locally for review. |
| `ctx-sync register <name>` | Register this worktree as a worker; `--force` replaces its existing identity. |
| `ctx-sync pull` | Fetch the latest shared context. |
| `ctx-sync sync` | Commit, rebase, and push local context changes. |
| `ctx-sync context` | Print the shared context; `--json` produces structured output. |
| `ctx-sync onboard` | Print a short onboarding view. |
| `ctx-sync status` | Show project, sync, conflict, and worker status. |
| `ctx-sync handoff` | Update your worker state with `--task`, `--summary`, `--status`, `--working-on`, `--changed`, `--interface-change`, `--attention`, or `--blocked-by`; use `--append` for lists and `--sync` to publish now. |
| `ctx-sync decision add <title>` | Add a decision with `--context`, `--decision`, `--reason`, `--consequences`, `--supersedes`, `--status`, and optional `--sync`. |
| `ctx-sync done` | Mark this worker done and sync; accepts `--summary`. |
| `ctx-sync agent start` | Pull and print onboarding context; use `--name` to register or `--resume` to continue a completed worker. |
| `ctx-sync agent finish` | Record a handoff and sync; accepts the `handoff` fields. |
| `ctx-sync install-agent-instructions` | Reserved for a future release; currently returns an error. |

`handoff` and `decision add` commit locally by default; pass `--sync` to publish immediately. `done` and `agent finish` always sync.

| Exit code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | General or argument error. |
| 2 | Context conflict. |
| 3 | Authentication error. |
| 4 | Invalid configuration. |
| 5 | Remote or sync retry error. |

## Troubleshooting

- **HTTPS authentication:** Run `gh auth setup-git` before pushing over HTTPS, or use `--ssh` when creating or attaching the project to use `git@gist.github.com`. Git credentials or SSH access must already be configured.
- **Git hooks:** ctx-sync does not skip hooks when committing or pushing the context repo. If a global hook (for example, a pre-push hook that blocks pushes to `main`) rejects a push to `gist.github.com`, update the hook to exclude the Gist remote.
- **Context conflict:** Run `ctx-sync status` and follow its displayed steps. Open the listed context repo, rebase onto its remote branch, resolve the listed files, continue the rebase, then run `ctx-sync sync`.

## License

MIT. See [LICENSE](LICENSE).
