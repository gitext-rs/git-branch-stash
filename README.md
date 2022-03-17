# git-branch-stash

> **Manage snapshots of your working directory**

[![codecov](https://codecov.io/gh/gitext-rs/git-branch-stash/branch/master/graph/badge.svg)](https://codecov.io/gh/gitext-rs/git-branch-stash)
[![Documentation](https://img.shields.io/badge/docs-master-blue.svg)][Documentation]
![License](https://img.shields.io/crates/l/git-branch-stash.svg)
[![Crates Status](https://img.shields.io/crates/v/git-branch-stash.svg)](https://crates.io/crates/git-branch-stash)

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE)

## Documentation

- [About](#about)
- [Install](#install)
- [Getting Started](#getting-started)
- [FAQ](#faq)
- [Contribute](CONTRIBUTING.md)
- [CHANGELOG](CHANGELOG.md)

## About

Backup and restore what your branches, including what they point at.

## Example

## Install

[Download](https://github.com/gitext-rs/git-branch-stash/releases) a pre-built binary
(installable via [gh-install](https://github.com/crate-ci/gh-install)).

Or use rust to install:
```bash
cargo install git-branch-stash-cli
```

### Uninstall

See the uninstall method for your installer.

Once removed, `git-branch-stash` leaves behind:
- `.git/branch-stash`

Removing this is safe and will have no effect.

## Getting Started

### Configuring `git-branch-stash`

**Protected branches:** These are branches like `main` or `v3` that `git-branch-stash`
must not modify.  `git-branch-stash` will also rebase local protected branches against
their remote counter parts.

Run `git-branch-stash --protected -v` to test your config
- To locally protect additional branches, run `git-branch-stash --protect <glob>`.
- When adopting `git-branch-stash` as a team, you can move the protected branches from
  `$REPO/.git/config` to `$REPO/.gitconfig` and commit it.

**Pull remote** when working from a fork, where upstream is a different remote than
`origin`, run `git config --add stack.pull-remote <REMOTE>` to set your remote in `$REPO/.git/config`.

To see the config, run `git-branch-stash --dump-config -`.

### Using

## FAQ

### Why don't you just ...?

Have an idea, we'd love to [hear it](https://github.com/gitext-rs/git-branch-stash/discussions)!
There are probably `git` operations or workflows we haven't heard of and would
welcome the opportunity to learn more.

[Crates.io]: https://crates.io/crates/git-branch-stash
[Documentation]: https://docs.rs/git-branch-stash
