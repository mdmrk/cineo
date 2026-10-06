# Contributing

Thanks for helping! Start with [AGENTS.md](AGENTS.md). It is the engineering
contract for humans and AI agents alike. Then read
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Ground rules

- **One topic per PR.** Base on current `main`, keep it small, rebase to stay
  current. Check for overlapping PRs first.
- **Tests prove behavior.** A bug fix needs a regression test, and a protocol
  behavior needs a fixture and a test. A COMPATIBILITY.md status only goes
  up with evidence.
- **Docs move with code.** Update the affected docs in the same PR. They
  describe verified behavior, not plans.
- **Architecture changes need an ADR** ([docs/DECISIONS.md](docs/DECISIONS.md)).
- **Fail loudly.** No silent stubs. Unsupported input produces an error or a
  logged warning.
- **Dependencies** are justified in
  [docs/dev/DEPENDENCIES.md](docs/dev/DEPENDENCIES.md).
- **Provenance.** Do not copy code from projects without a compatible license
  (notably stremio-web and stremio-shell-ng). Record adapted code in
  [docs/LEGAL.md](docs/LEGAL.md).
- **AI assistance** must be disclosed in the PR description. You are
  responsible for every line you submit.
- Investigation notes and logs belong in the PR discussion, not the
  repository.

## Before opening a PR

```sh
scripts/check.sh
```

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org).

By contributing, you agree that your contributions are licensed under the
project's MIT license.
