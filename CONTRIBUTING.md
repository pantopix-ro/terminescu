# Contributing

Thank you for helping. Issues and pull requests are welcome on GitHub.

## Contributor License Agreement

Before we can merge a pull request, its author signs the Contributor License Agreement. The CLA
Assistant bot asks for the signature in the pull request, once per person. The CLA lets
PANTOPIX RO ship your contribution under the project's license, the Functional Source License
1.1 with the Apache 2.0 future license, and under later licenses. You keep the copyright in
your work.

## Before you open a pull request

- Start your branch from `develop`.
- Run `pnpm check:rust` and `pnpm -r test`, and keep Rust code rustfmt-clean.
- Keep commit messages to 9 lines at most, in the present tense.
- Use invented data in tests, seeds and docs (`acme/api`, `example.com`). Never commit tokens,
  client data or real names.
- For a change in behavior, propose it first as an OpenSpec change under `openspec/changes/`.

## Reporting a problem

Use Help › Report an Issue in the app: it fills in the app and macOS versions. Or write to
salut@pantopix.com with a subject that starts with `[Terminescu]`.
