#![allow(clippy::collapsible_else_if)]

use std::io::Write;

use clap::Parser;
use itertools::Itertools;
use proc_exit::WithCodeResultExt;

mod args;
mod logger;

fn main() {
    human_panic::setup_panic!();
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    // clap's `get_matches` uses Failure rather than Usage, so bypass it for `get_matches_safe`.
    let args = match args::Args::try_parse() {
        Ok(args) => args,
        Err(e) if e.use_stderr() => {
            let _ = e.print();
            return proc_exit::Code::USAGE_ERR.ok();
        }
        Err(e) => {
            let _ = e.print();
            return proc_exit::Code::SUCCESS.ok();
        }
    };

    args.color.apply();
    let colored_stdout = concolor::get(concolor::Stream::Stdout).ansi_color();
    let colored_stderr = concolor::get(concolor::Stream::Stderr).ansi_color();

    logger::init_logging(args.verbose.clone(), colored_stderr);

    let subcommand = args.subcommand;
    let push_args = args.push;
    match subcommand.unwrap_or(args::Subcommand::Push(push_args)) {
        args::Subcommand::Push(sub_args) => push(sub_args),
        args::Subcommand::List(sub_args) => list(sub_args, colored_stdout),
        args::Subcommand::Clear(sub_args) => clear(sub_args),
        args::Subcommand::Drop(sub_args) => drop(sub_args),
        args::Subcommand::Pop(sub_args) => apply(sub_args, true),
        args::Subcommand::Apply(sub_args) => apply(sub_args, false),
        args::Subcommand::Stacks(sub_args) => stacks(sub_args),
    }
}

fn push(args: args::PushArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_branch_stash::GitRepo::new(repo);
    let mut stack = git_branch_stash::Stack::new(&args.stack, &repo);

    let repo_config = git_branch_stash::config::RepoConfig::from_all(repo.raw())
        .with_code(proc_exit::Code::CONFIG_ERR)?;

    stack.capacity(repo_config.capacity());

    if is_dirty(&repo) {
        log::warn!("Working tree is dirty, only capturing committed changes");
    }

    let mut snapshot =
        git_branch_stash::Snapshot::from_repo(&repo).with_code(proc_exit::Code::FAILURE)?;
    if let Some(message) = args.message.as_deref() {
        snapshot.insert_message(message);
    }
    stack.push(snapshot)?;

    Ok(())
}

fn list(args: args::ListArgs, colored: bool) -> proc_exit::ExitResult {
    let palette = if colored {
        Palette::colored()
    } else {
        Palette::plain()
    };

    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_branch_stash::GitRepo::new(repo);
    let stack = git_branch_stash::Stack::new(&args.stack, &repo);

    let snapshots: Vec<_> = stack.iter().collect();
    for (i, snapshot_path) in snapshots.iter().enumerate() {
        let style = if i < snapshots.len() - 1 {
            palette.info
        } else {
            palette.good
        };
        let snapshot = match git_branch_stash::Snapshot::load(snapshot_path) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                log::error!(
                    "Failed to load snapshot {}: {}",
                    snapshot_path.display(),
                    err
                );
                continue;
            }
        };
        match snapshot.metadata.get("message") {
            Some(message) => {
                writeln!(
                    std::io::stdout(),
                    "{}",
                    style.paint(format_args!("Message: {}", message))
                )?;
            }
            None => {
                writeln!(
                    std::io::stdout(),
                    "{}",
                    style.paint(format_args!("Path: {}", snapshot_path.display()))
                )?;
            }
        }
        for branch in snapshot.branches.iter() {
            let summary = if let Some(summary) = branch.metadata.get("summary") {
                summary.to_string()
            } else {
                branch.id.to_string()
            };
            let name =
                if let Some(serde_json::Value::String(parent)) = branch.metadata.get("parent") {
                    format!("{}..{}", parent, branch.name)
                } else {
                    branch.name.clone()
                };
            writeln!(
                std::io::stdout(),
                "{}",
                style.paint(format_args!("- {}: {}", name, summary))
            )?;
        }
        writeln!(std::io::stdout())?;
    }

    Ok(())
}

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
struct Palette {
    error: yansi::Style,
    warn: yansi::Style,
    info: yansi::Style,
    good: yansi::Style,
    hint: yansi::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: yansi::Style::new(yansi::Color::Red),
            warn: yansi::Style::new(yansi::Color::Yellow),
            info: yansi::Style::new(yansi::Color::Blue),
            good: yansi::Style::new(yansi::Color::Green),
            hint: yansi::Style::new(yansi::Color::Blue).dimmed(),
        }
    }

    pub fn plain() -> Self {
        Self {
            error: yansi::Style::default(),
            warn: yansi::Style::default(),
            info: yansi::Style::default(),
            good: yansi::Style::default(),
            hint: yansi::Style::default(),
        }
    }
}

fn clear(args: args::ClearArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_branch_stash::GitRepo::new(repo);
    let mut stack = git_branch_stash::Stack::new(&args.stack, &repo);

    stack.clear();

    Ok(())
}

fn drop(args: args::DropArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_branch_stash::GitRepo::new(repo);
    let mut stack = git_branch_stash::Stack::new(&args.stack, &repo);

    stack.pop();

    Ok(())
}

fn apply(args: args::ApplyArgs, pop: bool) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let mut repo = git_branch_stash::GitRepo::new(repo);
    let mut stack = git_branch_stash::Stack::new(&args.stack, &repo);

    match stack.peek() {
        Some(last) => {
            let snapshot =
                git_branch_stash::Snapshot::load(&last).with_code(proc_exit::Code::FAILURE)?;

            let stash_id = stash_push(&mut repo, "branch-stash");
            if is_dirty(&repo) {
                stash_pop(&mut repo, stash_id);
                return Err(
                    proc_exit::Code::USAGE_ERR.with_message("Working tree is dirty, aborting")
                );
            }

            snapshot
                .apply(&mut repo)
                .with_code(proc_exit::Code::FAILURE)?;

            stash_pop(&mut repo, stash_id);
            if pop {
                let _ = std::fs::remove_file(&last);
            }
        }
        None => {
            log::warn!("Nothing to apply");
        }
    }

    Ok(())
}

fn stacks(_args: args::StacksArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_branch_stash::GitRepo::new(repo);

    for stack in git_branch_stash::Stack::all(&repo) {
        writeln!(std::io::stdout(), "{}", stack.name)?;
    }

    Ok(())
}

fn is_dirty(repo: &git_branch_stash::GitRepo) -> bool {
    if repo.raw().state() != git2::RepositoryState::Clean {
        log::trace!("Repository status is unclean: {:?}", repo.raw().state());
        return true;
    }

    let status = repo
        .raw()
        .statuses(Some(git2::StatusOptions::new().include_ignored(false)))
        .unwrap();
    if status.is_empty() {
        false
    } else {
        log::trace!(
            "Repository is dirty: {}",
            status
                .iter()
                .flat_map(|s| s.path().map(|s| s.to_owned()))
                .join(", ")
        );
        true
    }
}

fn stash_push(repo: &mut git_branch_stash::GitRepo, context: &str) -> Option<git2::Oid> {
    let branch = repo
        .raw()
        .head()
        .and_then(|r| r.resolve())
        .ok()
        .and_then(|r| r.shorthand().map(|s| s.to_owned()));

    let stash_msg = format!(
        "WIP on {} ({})",
        branch.as_deref().unwrap_or("HEAD"),
        context
    );
    let signature = repo.raw().signature();
    let stash_id = signature.and_then(|signature| {
        repo.raw_mut()
            .stash_save2(&signature, Some(&stash_msg), None)
    });

    match stash_id {
        Ok(stash_id) => {
            log::info!(
                "Saved working directory and index state {}: {}",
                stash_msg,
                stash_id
            );
            Some(stash_id)
        }
        Err(err) => {
            log::debug!("Failed to stash: {}", err);
            None
        }
    }
}

fn stash_pop(repo: &mut git_branch_stash::GitRepo, stash_id: Option<git2::Oid>) {
    if let Some(stash_id) = stash_id {
        let mut index = None;
        let _ = repo.raw_mut().stash_foreach(|i, _, id| {
            if *id == stash_id {
                index = Some(i);
                false
            } else {
                true
            }
        });
        let index = if let Some(index) = index {
            index
        } else {
            return;
        };

        match repo.raw_mut().stash_pop(index, None) {
            Ok(()) => {
                log::info!("Dropped refs/stash {}", stash_id);
            }
            Err(err) => {
                log::error!("Failed to pop {} from stash: {}", stash_id, err);
            }
        }
    }
}
