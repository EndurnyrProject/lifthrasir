# Contributing to Lifthrasir

Thank you for your interest in contributing to Lifthrasir! This document outlines the development workflow using Jujutsu (jj) version control system, coding standards, and best practices.

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
