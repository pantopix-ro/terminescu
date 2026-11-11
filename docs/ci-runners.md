# CI runners

Every job reads its runner from the repository variable `CI_RUNS_ON`.

| `CI_RUNS_ON` | Jobs run on |
|---|---|
| `local` | the local Mac runner (labels `self-hosted`, `terminescu-runner`) |
| unset, or any other value | GitHub: `macos-15` for CI and releases, `ubuntu-latest` for the license check |

Switch with `gh variable set CI_RUNS_ON --body local`, and back with
`gh variable delete CI_RUNS_ON`. A run reads the workflow file of its own commit, so an open
pull request follows the switch only once its branch carries these workflows.

## The local runner

The runner lives in `~/.actions-runners/terminescu` as a launchd service, registered to this
repository with the labels `self-hosted`, `macOS`, `ARM64` and `terminescu-runner`. It needs Node
(the version in `.nvmrc`), pnpm and the stable Rust toolchain on the `PATH` it was configured
with. One runner takes one job at a time, so pull requests queue.

```
cd ~/.actions-runners/terminescu
./svc.sh status        # or stop, start
```

Timing tests (`crates/cli/tests/hook.rs`) are tuned for an idle machine. A Mac that also runs
other runners or a build can push them over budget; rerun the job before reading it as a bug.
A Mac that sleeps drops the job: rerun it with `gh run rerun <id> --failed`.
