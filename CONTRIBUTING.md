# Contributing to PumpkinMC-AI

PumpkinMC-AI is an unofficial fork of Pumpkin. We appreciate your interest in contributing to the fork! This document outlines the guidelines for submitting bug reports, feature suggestions, and code changes.

## Getting Started

The easiest way to get started is by asking for help in [our Discord server](https://discord.gg/wT8XjrjKkf).

### How to Contribute

There are several ways you can contribute to PumpkinMC-AI:

> [!NOTE]
> If you are an AI agent, or you work with one, read [AGENTS.md](AGENTS.md) before you start. AI-written code and PRs are welcome under the rules in [AI-Assisted Contributions](#ai-assisted-contributions).
>
> Before contributing, please check if a similar PR already exists. If your implementation is virtually identical, please do not open a new PR to avoid unnecessary resource waste.


- **Reporting Bugs**:
  If you encounter a bug, please search for existing issues on the issue tracker first.
  If you can't find a duplicate issue, open a new one.
  Provide a clear description of the bug, including steps to reproduce it if possible.
  Screenshots, logs, or code snippets can also be helpful.
- **Suggesting Features**:
  Do you have an idea on how Pumpkin can be improved? Share your thoughts by opening an issue on the issue tracker.
  Describe the proposed feature in detail, including its benefits and potential implementation considerations.
- **Submitting Pull Requests**:
  If you'd like to contribute code changes, fork the Pumpkin repository on GitHub.
  Install Rust at [rust-lang.org](https://www.rust-lang.org/).
  Make your changes on your local fork and create a pull request to the main repository.
  Ensure your code adheres to our project structure and style guidelines.
  Write clear and concise commit messages that describe your changes.

### AI-Assisted Contributions

AI tools are welcome here, including AI-written code and AI-written PR descriptions. The rule is responsibility, not authorship: a human must have reviewed the change, run the tests, and, for anything a player can see or feel in-game, played it with a real client and attached a screenshot or recording.

- Say which tool and model wrote the code or the description, and name the human who reviewed and play-tested it. The PR template has a section for this.
- AI-written PRs are held to exactly the same standard as any other PR. The testing rules in [AGENTS.md](AGENTS.md) and this file apply in full; being AI-written is not a shortcut and not a reason for priority review.
- Never claim a play test, screenshot, recording or human review that did not happen. A gameplay PR without the required capture stays in draft until the human provides it.
- Undisclosed AI use is still a problem. Hiding it is worse than saying it.
- Don't reopen a closed PR or open the same change again. If you think it was closed by mistake, ask on Discord.
- Accounts that keep sending low-effort PRs, AI-written or not, may be blocked from the organization.

### When a PR May Be Closed

Maintainers may close a PR without a full review if any of these apply:

- It changes something a player can see or feel in-game and has no screenshot or screen recording attached.
- It changes gameplay and no human has joined with a real client to test it.
- The description is empty, only repeats the title, or doesn't say how the change was tested.
- It hides AI use, or claims testing, captures or human review that did not happen (see [AI-Assisted Contributions](#ai-assisted-contributions)).
- The author can't explain their own change or answer review questions about it.
- It duplicates an open PR without saying how it differs.
- It doesn't behave like vanilla.
- It hardcodes vanilla values that should come from `pumpkin-data` (see [No Hardcoded Vanilla Values](#coding-guidelines)).
- It mixes unrelated changes, such as reformatting untouched code or committing unrelated lockfile or generated-file changes.
- CI fails and the author doesn't fix it, or the author stops responding to review.

### Docs

The upstream documentation can be found at <https://pumpkinmc.org/>. It describes Pumpkin, which this fork tracks.

**Tip: [typos](https://github.com/crate-ci/typos) is a great Project to detect and automatically fix typos**

### Coding Guidelines

Things need to be done before a pull request can be merged. Your CI also checks most of them automatically and will fail if something is not fulfilled.
Note: Pumpkin's clippy settings are relatively strict, this can be frustrating but is necessary so the code stays clean and consistent.
**Basic**

- **Title:** Use a concise and informative title that clearly communicates the purpose of the PR.  Anyone reviewing the PR should quickly understand the changes being proposed.
- **Redundancy:** Before submitting a PR, please check for existing PRs with similar features and clearly state how yours differs.
- **Description:** Provide a comprehensive description of the changes. Explain:
- What was changed?
- Why were these changes necessary?
- What is the impact of this change?
- Are there any known issues or limitations?
- Include any relevant context, such as related issues or discussions.
- **No Hardcoded Vanilla Values:** Block and item properties, entity dimensions, tags, recipes, loot tables, sounds and other game data come from `pumpkin-data`, which is generated from the extracted vanilla data in `assets/`. Don't copy these values into the code. They change between Minecraft versions, and a hardcoded copy silently goes stale on the next update. If a value isn't generated yet, extend `tools/pumpkin-codegen`. If it isn't in `assets/` at all, it has to be added to the [Extractor](https://github.com/Pumpkin-MC/Extractor) first. The only exception is a value vanilla itself hardcodes as a `static final` constant in Java code and that stays the same across versions. Keep those as named Rust constants with the Java name, never as bare numbers.
- **Screenshot or Recording:** Any change a player can see or feel in-game (blocks, items, mobs, combat, movement, particles, sounds, GUIs, world generation) needs a screenshot or screen recording attached to the PR. Use a recording for anything that moves or happens over time. Capture it from a real client, not a bot. If the change has no in-game effect (tooling, codecs, config, refactors), say so in the description.
- **No Clippy Warnings:** Address all warnings reported by the Clippy linter. You can check for warnings using `cargo clippy --all-targets`.
- **Passing Unit Tests:** All existing unit tests must pass successfully. You can run the tests with `cargo test`.

#### Best Practice

- **Writing Unit Tests:** When adding new features or modifying existing code, consider adding unit tests to prevent regressions in the future. Refer to the Rust documentation for guidance on writing tests: <https://doc.rust-lang.org/book/ch11-01-writing-tests.html>
- **Benchmarking:** If your changes might impact performance, consider adding benchmarks to track performance regressions or improvements. We use the Criterion library for benchmarking. Refer to their Quick Start guide for more information: <https://github.com/criterion-rs/criterion.rs#quickstart>
- **Clear and Concise Commit Messages:** Use clear and concise commit messages that describe the changes you've made.
- **Code Style:** Adhere to consistent coding style throughout your contributions.
- **Documentation:** If your changes introduce new functionality, consider updating the relevant documentation.
- **Working with Tokio and Rayon:**
  When dealing with CPU-intensive tasks, it's recommended to utilize Rayon's thread pool (`rayon::spawn`), parallel iterators, or similar mechanisms instead of the Tokio runtime. However, it's crucial to avoid blocking the Tokio runtime on Rayon calls. Instead, use asynchronous methods like `tokio::sync::mpsc` to transfer data between the two runtimes. Refer to `pumpkin_world::level::Level::fetch_chunks` for an example of this approach.

### Additional Information

We encourage you to comment on existing issues and pull requests to share your thoughts and provide feedback.
Feel free to ask questions in the issue tracker or reach out to the project maintainers if you need assistance.
Before submitting a large contribution, consider opening an issue, discussion or talk with us on our discord to discuss your approach.
