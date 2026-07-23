# Contributing

Thanks for wanting to help with rehash. This should cover the basics of how to contribute, report bugs, and what to expect from the review process.

## Getting Started

1. Fork the repo.
2. Clone your fork and build from source:

```bash
cargo build --workspace
```

3. Run the tests to make sure everything works:

```bash
cargo test --workspace
```

## What we're looking for

Pull requests that fix a bug, add a useful feature, or improve the code quality. If you want to work on something substantial, open an issue first so we can talk about it before you write a bunch of code.

Things that probably won't get merged:
- PRs that add dependencies without a clear reason
- Changes that break the build or tests
- Cosmetic changes that don't fix anything (reformatting, renaming without cause)

## Pull Request Process

1. Keep PRs focused on one thing. A PR that fixes a bug and adds a feature and refactors three modules is hard to review.
2. Write a clear description of what changed and why.
3. Make sure `cargo check --workspace` and `cargo test --workspace` pass.
4. If you're adding something new, include tests.

## Reporting Bugs

Open an issue with:
- What you were doing when it broke
- What you expected to happen
- What actually happened
- Your OS and Rust version (`rustc --version`)

## Code Style

- `cargo fmt` before committing
- No tabs, 4-space indent
- Follow patterns you see in the existing code

## Review Process

Someone will look at your PR within a few days. We might ask for changes. That's normal, not a rejection. The goal is to keep the codebase consistent and maintainable.
