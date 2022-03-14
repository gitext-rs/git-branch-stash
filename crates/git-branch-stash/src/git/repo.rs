use bstr::ByteSlice;
use itertools::Itertools;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Branch {
    pub name: String,
    pub id: git2::Oid,
    pub push_id: Option<git2::Oid>,
    pub pull_id: Option<git2::Oid>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Commit {
    pub id: git2::Oid,
    pub tree_id: git2::Oid,
    pub summary: bstr::BString,
    pub time: std::time::SystemTime,
    pub author: Option<std::rc::Rc<str>>,
    pub committer: Option<std::rc::Rc<str>>,
}

impl Commit {
    pub fn fixup_summary(&self) -> Option<&bstr::BStr> {
        self.summary
            .strip_prefix(b"fixup! ")
            .map(ByteSlice::as_bstr)
    }

    pub fn wip_summary(&self) -> Option<&bstr::BStr> {
        // Gitlab MRs only: b"[Draft]", b"(Draft)",
        static WIP_PREFIXES: &[&[u8]] = &[
            b"WIP:", b"draft:", b"Draft:", // Gitlab commits
            b"wip ", b"WIP ", // Less formal
        ];

        if self.summary == b"WIP".as_bstr() || self.summary == b"wip".as_bstr() {
            // Very informal
            Some(b"".as_bstr())
        } else {
            WIP_PREFIXES
                .iter()
                .filter_map(|prefix| {
                    self.summary
                        .strip_prefix(*prefix)
                        .map(ByteSlice::trim)
                        .map(ByteSlice::as_bstr)
                })
                .next()
        }
    }

    pub fn revert_summary(&self) -> Option<&bstr::BStr> {
        self.summary
            .strip_prefix(b"Revert ")
            .and_then(|s| s.strip_suffix(b"\""))
            .map(ByteSlice::as_bstr)
    }
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

    pub fn set_push_remote(&mut self, remote: &str) {
        self.push_remote = Some(remote.to_owned());
    }

    pub fn set_pull_remote(&mut self, remote: &str) {
        self.pull_remote = Some(remote.to_owned());
    }

    pub fn push_remote(&self) -> &str {
        self.push_remote.as_deref().unwrap_or("origin")
    }

    pub fn pull_remote(&self) -> &str {
        self.pull_remote.as_deref().unwrap_or("origin")
    }

    pub fn raw(&self) -> &git2::Repository {
        &self.repo
    }

    pub fn user(&self) -> Option<std::rc::Rc<str>> {
        self.repo
            .signature()
            .ok()
            .and_then(|s| s.name().map(|n| self.intern_string(n)))
    }

    pub fn is_dirty(&self) -> bool {
        if self.repo.state() != git2::RepositoryState::Clean {
            log::trace!("Repository status is unclean: {:?}", self.repo.state());
            return true;
        }

        let status = self
            .repo
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

    pub fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid> {
        self.repo.merge_base(one, two).ok()
    }

    pub fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>> {
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

    pub fn head_commit(&self) -> std::rc::Rc<Commit> {
        let head_id = self
            .repo
            .head()
            .unwrap()
            .resolve()
            .unwrap()
            .target()
            .unwrap();
        self.find_commit(head_id).unwrap()
    }

    pub fn head_branch(&self) -> Option<Branch> {
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

    pub fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>> {
        let id = self.repo.revparse_single(revspec).ok()?.id();
        self.find_commit(id)
    }

    pub fn commits_from(
        &self,
        head_id: git2::Oid,
    ) -> impl Iterator<Item = std::rc::Rc<Commit>> + '_ {
        let mut revwalk = self.repo.revwalk().unwrap();
        revwalk.push(head_id).unwrap();
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL).unwrap();

        revwalk
            .filter_map(Result::ok)
            .filter_map(move |oid| self.find_commit(oid))
    }

    pub fn contains_commit(
        &self,
        haystack_id: git2::Oid,
        needle_id: git2::Oid,
    ) -> Result<bool, git2::Error> {
        let needle_commit = self.repo.find_commit(needle_id)?;
        let needle_ann_commit = self.repo.find_annotated_commit(needle_id)?;
        let haystack_ann_commit = self.repo.find_annotated_commit(haystack_id)?;

        let parent_ann_commit = if 0 < needle_commit.parent_count() {
            let parent_commit = needle_commit.parent(0)?;
            Some(self.repo.find_annotated_commit(parent_commit.id())?)
        } else {
            None
        };

        let mut rebase = self.repo.rebase(
            Some(&needle_ann_commit),
            parent_ann_commit.as_ref(),
            Some(&haystack_ann_commit),
            Some(git2::RebaseOptions::new().inmemory(true)),
        )?;

        if let Some(op) = rebase.next() {
            op.map_err(|e| {
                let _ = rebase.abort();
                e
            })?;
            let inmemory_index = rebase.inmemory_index().unwrap();
            if inmemory_index.has_conflicts() {
                return Ok(false);
            }

            let sig = self.repo.signature().unwrap();
            match rebase.commit(None, &sig, None).map_err(|e| {
                let _ = rebase.abort();
                e
            }) {
                // Created commit, must be unique
                Ok(_) => Ok(false),
                Err(err) => {
                    if err.class() == git2::ErrorClass::Rebase
                        && err.code() == git2::ErrorCode::Applied
                    {
                        return Ok(true);
                    }
                    Err(err)
                }
            }
        } else {
            // No commit created, must exist somehow
            rebase.finish(None)?;
            Ok(true)
        }
    }

    pub fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid, git2::Error> {
        let signature = self.repo.signature()?;
        self.repo.stash_save2(&signature, message, None)
    }

    pub fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<(), git2::Error> {
        let mut index = None;
        self.repo.stash_foreach(|i, _, id| {
            if *id == stash_id {
                index = Some(i);
                false
            } else {
                true
            }
        })?;
        let index = index.ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "stash ID not found",
            )
        })?;
        self.repo.stash_pop(index, None)
    }

    pub fn branch(&mut self, name: &str, id: git2::Oid) -> Result<(), git2::Error> {
        let commit = self.repo.find_commit(id)?;
        self.repo.branch(name, &commit, true)?;
        Ok(())
    }

    pub fn delete_branch(&mut self, name: &str) -> Result<(), git2::Error> {
        // HACK: We shouldn't limit ourselves to `Local`
        let mut branch = self.repo.find_branch(name, git2::BranchType::Local)?;
        branch.delete()
    }

    pub fn find_local_branch(&self, name: &str) -> Option<Branch> {
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

    pub fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
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

    pub fn detach(&mut self) -> Result<(), git2::Error> {
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

    pub fn switch(&mut self, name: &str) -> Result<(), git2::Error> {
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

pub fn stash_push(repo: &mut GitRepo, context: &str) -> Option<git2::Oid> {
    let branch = repo.head_branch();
    let stash_msg = format!(
        "WIP on {} ({})",
        branch.as_ref().map(|b| b.name.as_str()).unwrap_or("HEAD"),
        context
    );
    match repo.stash_push(Some(&stash_msg)) {
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

pub fn stash_pop(repo: &mut GitRepo, stash_id: Option<git2::Oid>) {
    if let Some(stash_id) = stash_id {
        match repo.stash_pop(stash_id) {
            Ok(()) => {
                log::info!("Dropped refs/stash {}", stash_id);
            }
            Err(err) => {
                log::error!("Failed to pop {} from stash: {}", stash_id, err);
            }
        }
    }
}
