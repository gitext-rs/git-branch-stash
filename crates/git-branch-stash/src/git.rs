use bstr::ByteSlice;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Branch {
    pub(crate) name: String,
    pub(crate) id: git2::Oid,
    pub(crate) push_id: Option<git2::Oid>,
    pub(crate) pull_id: Option<git2::Oid>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Commit {
    pub(crate) id: git2::Oid,
    pub(crate) tree_id: git2::Oid,
    pub(crate) summary: bstr::BString,
    pub(crate) time: std::time::SystemTime,
    pub(crate) author: Option<std::rc::Rc<str>>,
    pub(crate) committer: Option<std::rc::Rc<str>>,
}

pub struct GitRepo {
    repo: git2::Repository,
    push_remote: Option<String>,
    pull_remote: Option<String>,
    commits: std::cell::RefCell<std::collections::HashMap<git2::Oid, std::rc::Rc<Commit>>>,
    interned_strings: std::cell::RefCell<std::collections::HashSet<std::rc::Rc<str>>>,
}

impl GitRepo {
    pub fn new(repo: git2::Repository) -> Self {
        Self {
            repo,
            push_remote: None,
            pull_remote: None,
            commits: Default::default(),
            interned_strings: Default::default(),
        }
    }

    pub(crate) fn push_remote(&self) -> &str {
        self.push_remote.as_deref().unwrap_or("origin")
    }

    pub(crate) fn pull_remote(&self) -> &str {
        self.pull_remote.as_deref().unwrap_or("origin")
    }

    pub fn raw(&self) -> &git2::Repository {
        &self.repo
    }

    pub fn raw_mut(&mut self) -> &mut git2::Repository {
        &mut self.repo
    }

    pub(crate) fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>> {
        let mut commits = self.commits.borrow_mut();
        if let Some(commit) = commits.get(&id) {
            Some(std::rc::Rc::clone(commit))
        } else {
            let commit = self.repo.find_commit(id).ok()?;
            let summary: bstr::BString = commit.summary_bytes().unwrap().into();
            let time = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(commit.time().seconds().max(0) as u64);

            let author = commit.author().name().map(|n| self.intern_string(n));
            let committer = commit.author().name().map(|n| self.intern_string(n));
            let commit = std::rc::Rc::new(Commit {
                id: commit.id(),
                tree_id: commit.tree_id(),
                summary,
                time,
                author,
                committer,
            });
            commits.insert(id, std::rc::Rc::clone(&commit));
            Some(commit)
        }
    }

    pub(crate) fn head_branch(&self) -> Option<Branch> {
        let resolved = self.repo.head().unwrap().resolve().unwrap();
        let name = resolved.shorthand()?;
        let id = resolved.target()?;

        let push_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.push_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());
        let pull_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.pull_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());

        Some(Branch {
            name: name.to_owned(),
            id,
            push_id,
            pull_id,
        })
    }

    pub(crate) fn branch(&mut self, name: &str, id: git2::Oid) -> Result<(), git2::Error> {
        let commit = self.repo.find_commit(id)?;
        self.repo.branch(name, &commit, true)?;
        Ok(())
    }

    pub(crate) fn find_local_branch(&self, name: &str) -> Option<Branch> {
        let branch = self.repo.find_branch(name, git2::BranchType::Local).ok()?;
        let id = branch.get().target().unwrap();

        let push_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.push_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());
        let pull_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.pull_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());

        Some(Branch {
            name: name.to_owned(),
            id,
            push_id,
            pull_id,
        })
    }

    pub(crate) fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        log::trace!("Loading branches");
        self.repo
            .branches(Some(git2::BranchType::Local))
            .into_iter()
            .flatten()
            .flat_map(move |branch| {
                let (branch, _) = branch.ok()?;
                let name = if let Some(name) = branch.name().ok().flatten() {
                    name
                } else {
                    log::debug!(
                        "Ignoring non-UTF8 branch {:?}",
                        branch.name_bytes().unwrap().as_bstr()
                    );
                    return None;
                };
                let id = branch.get().target().unwrap();

                let push_id = self
                    .repo
                    .find_branch(
                        &format!("{}/{}", self.push_remote(), name),
                        git2::BranchType::Remote,
                    )
                    .ok()
                    .and_then(|b| b.get().target());
                let pull_id = self
                    .repo
                    .find_branch(
                        &format!("{}/{}", self.pull_remote(), name),
                        git2::BranchType::Remote,
                    )
                    .ok()
                    .and_then(|b| b.get().target());

                Some(Branch {
                    name: name.to_owned(),
                    id,
                    push_id,
                    pull_id,
                })
            })
    }

    pub(crate) fn detach(&mut self) -> Result<(), git2::Error> {
        let head_id = self
            .repo
            .head()
            .unwrap()
            .resolve()
            .unwrap()
            .target()
            .unwrap();
        self.repo.set_head_detached(head_id)?;
        Ok(())
    }

    pub(crate) fn switch(&mut self, name: &str) -> Result<(), git2::Error> {
        // HACK: We shouldn't limit ourselves to `Local`
        let branch = self.repo.find_branch(name, git2::BranchType::Local)?;
        self.repo.set_head(branch.get().name().unwrap())?;
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();
        self.repo.checkout_head(Some(&mut builder))?;
        Ok(())
    }

    fn intern_string(&self, data: &str) -> std::rc::Rc<str> {
        let mut interned_strings = self.interned_strings.borrow_mut();
        if let Some(interned) = interned_strings.get(data) {
            std::rc::Rc::clone(interned)
        } else {
            let interned = std::rc::Rc::from(data);
            interned_strings.insert(std::rc::Rc::clone(&interned));
            interned
        }
    }
}

impl std::fmt::Debug for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("GitRepo")
            .field("repo", &self.repo.workdir())
            .field("push_remote", &self.push_remote.as_deref())
            .field("pull_remote", &self.pull_remote.as_deref())
            .finish()
    }
}
