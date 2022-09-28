#[derive(Debug, clap::Parser)]
#[command(name = "git-branch-stash", about, author, version)]
#[command(
        args_conflicts_with_subcommands = true,
        color = concolor_clap::color_choice(),
    )]
pub struct Args {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[command(flatten)]
    pub push: PushArgs,

    #[command(flatten)]
    pub(crate) color: concolor_clap::Color,

    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
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
pub struct PushArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub stack: String,

    /// Annotate the snapshot with the given message
    #[arg(short, long)]
    pub message: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ListArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(Debug, clap::Args)]
pub struct ClearArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(Debug, clap::Args)]
pub struct DropArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(Debug, clap::Args)]
pub struct ApplyArgs {
    /// Specify which stash stack to use
    #[arg(default_value = git_branch_stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(Debug, clap::Args)]
pub struct StacksArgs {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Args::command().debug_assert()
    }
}
