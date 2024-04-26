#[derive(Debug, clap::Parser)]
#[command(name = "git-branch-stash", about, author, version)]
#[command(
        args_conflicts_with_subcommands = true,
        color = concolor_clap::color_choice(),
    )]
pub(crate) struct Args {
    #[command(subcommand)]
    pub(crate) subcommand: Option<Subcommand>,

    #[command(flatten)]
    pub(crate) push: PushArgs,

    #[command(flatten)]
    pub(crate) color: concolor_clap::Color,

    #[command(flatten)]
    pub(crate) verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Subcommand {
    /// Stash all branches
    Push(PushArgs),
    /// List all stashed snapshots
    List(ListArgs),
    /// Clear all snapshots
    Clear(ClearArgs),
    /// Delete the last snapshot
    Drop(DropArgs),
    /// Apply the last snapshot, deleting it
    Pop(ApplyArgs),
    /// Apply the last snapshot
    Apply(ApplyArgs),
    /// List all snapshot stacks
    Stacks(StacksArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct PushArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub(crate) stack: String,

    /// Annotate the snapshot with the given message
    #[arg(short, long)]
    pub(crate) message: Option<String>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ListArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub(crate) stack: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ClearArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub(crate) stack: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct DropArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub(crate) stack: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ApplyArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub(crate) stack: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct StacksArgs {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
