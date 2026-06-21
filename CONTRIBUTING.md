# Contributing to Lifthrasir

Thank you for your interest in contributing to Lifthrasir! This document outlines the development workflow using Jujutsu (jj) version control system, coding standards, and best practices.

## Using AI Tools

AI assistants (Copilot, Claude, ChatGPT, Cursor, and similar) are welcome here. They are not a shortcut around the bar every contribution has to clear, though. If you use one, **you** are the author and you are fully responsible for what you submit. The rules below are heavily inspired by [curl's stance on AI use](https://curl.se/dev/contribute.html#on-ai-use-in-curl).

### You own the contribution

- Understand every line you submit. You must be able to explain and defend it in review as if you had written it by hand.
- Do not paste raw AI output. AI tends to be verbose, adds comments that explain the obvious, and invents details. Trim it down to match our style: early returns over nested `if`s, no superfluous comments, critical paths that fail loudly rather than papering over errors.
- If a reviewer can tell a change was AI-generated and that you do not fully grasp it, you have more work to do before it is ready.

### Same quality bar

AI-assisted code must meet the exact same requirements as everything else:

- `cargo fmt` clean, `cargo clippy` with no warnings, `cargo check` passing.
- The full test suite passes (`cargo test`), with tests added for new domain logic.
- It follows the architecture and idioms already in the codebase.

### Licensing and provenance (read this one carefully)

This is the part that matters most for a Ragnarok Online client.

- Lifthrasir is **MIT licensed**. Only contribute code you actually have the right to license under MIT.
- Language models routinely reproduce code memorized from other RO clients and servers, many of them GPL or proprietary (rAthena, Hercules, OpenKore, Korangar, and others). Do not let an AI launder incompatibly-licensed code into this project.
- If you cannot establish where AI-suggested code came from or under what license, do not submit it.

### Issues, bug and security reports

- If you used an AI tool to find a problem, say so in the report.
- Reproduce and verify a finding yourself before filing it. AI tools frequently generate inaccurate or fabricated results.
- Unverified "AI slop" reports waste maintainer time and will be closed. Deliberately fabricated reports will get you banned.

## Development Workflow with Jujutsu

Lifthrasir uses [Jujutsu](https://jj-vcs.github.io/jj/latest/) (jj) backed by Git for version control. Jujutsu provides a more intuitive and powerful version control experience compared to traditional Git workflows.

Its not mandatory though, since jj is backed by Git, you can keep using Git if you prefer.

### Initial Setup

1. **Install Jujutsu**: Follow the installation instructions at https://jj-vcs.github.io/jj/latest/install/

2. **Clone the repository**:
   ```bash
   jj git clone git@github.com:EndurnyrProject/lifthrasir.git
   cd lifthrasir
   ```

3. **Set up the development environment**:
   ```bash
   cargo build
   ```

4. **Generate derived assets**: After placing your GRF files (`data.grf`, `en.grf`) under `assets/`, run the converter once before the first launch:
   ```bash
   cargo run -p ro-to-lifthrasir-cli -- convert
   ```
   This generates `assets/data/ron/job_data.ron`, which the client loads for job and sprite data. Re-run it whenever the source `.lub` files in your GRFs change.

### Feature Development Workflow

#### 1. Starting a New Feature

Before starting work on a new feature or bug fix:

```bash
# Create a new change for your feature
jj new -m "feat: implement new feature description"

# Alternatively, create a bookmark for easier tracking
jj bookmark create feature/your-feature-name
```

#### 2. Development Cycle

Jujutsu treats the working directory as a commit, making development more fluid:

```bash
# View current status
jj log

# Make your code changes...

# View what you've changed
jj diff

# Add a meaningful commit message to your change
jj describe -m "feat: detailed description of what you implemented

- Added new packet handler for login flow
- Implemented session validation
- Updated tests for edge cases"

# Continue making changes in the same commit or create a new one
jj new -m "fix: address review feedback"
```

#### 3. Keeping Changes Clean

Use Jujutsu's powerful editing features to maintain clean history:

```bash
# Combine multiple commits into one
jj squash -r <commit-id>

# Edit a previous commit
jj edit <commit-id>
# Make changes, then return to working copy
jj edit @

# Rebase changes interactively
jj rebase -i -d master

# View operation history (useful for undoing mistakes)
jj op log

# Undo the last operation if needed
jj undo
```

#### 4. Code Quality Checks

Before submitting your changes, ensure code quality:

```bash
# Format code
cargo fmt

# Run linting
cargo clippy
cargo check

```

#### 5. Preparing for Review

When your feature is ready:

```bash
# Ensure your changes are on top of latest master
jj rebase -d master

# Create a clean, descriptive commit message
jj describe -m "feat: implement character movement validation

This commit adds comprehensive validation for character movement
packets to prevent cheating and ensure proper game mechanics:

- Added position validation against map boundaries
- Implemented speed checks to prevent teleporting
- Added unit tests with 95% coverage
- Updated documentation for packet handlers

Closes #123"

# Push to your fork
jj git push --remote origin --branch your-feature-name
```

## Commit Message Guidelines

Use conventional commit format:

```
type(scope): subject

body

footer
```

### Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Build process, tooling changes

### Examples
```
feat(zone): implement player movement validation

Add comprehensive validation for character movement packets to prevent
speed hacking and teleportation exploits.

- Validate movement speed against character stats
- Check path validity on server side  
- Add integration tests for movement system

Closes #123
```

## Getting Help

- **Documentation**: Check the project's README and code documentation
- **Issues**: Search existing GitHub issues before creating new ones
- **Discussions**: Use GitHub Discussions for questions and general discussion
- **Code Review**: Be open to feedback and iterate on your changes

## Review Process

1. **Self-review**: Review your own code before requesting review
2. **Quality checks**: Ensure all tests pass and code is properly formatted
3. **Documentation**: Update relevant documentation if needed
4. **Small changes**: Keep pull requests focused and reasonably sized
5. **Responsiveness**: Respond to review feedback promptly

## Resources

- [Jujutsu Tutorial](https://jj-vcs.github.io/jj/latest/tutorial/)

Thank you for contributing to Lifthrasir!
