/// State of all branches
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub branches: Vec<Branch>,
    #[serde(default)]
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Snapshot {
    /// Load branch state from a file
    pub fn load(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let b = serde_json::from_reader(reader)?;
        Ok(b)
    }

    /// Save branch state to a file
    pub fn save(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let s = serde_json::to_string_pretty(self)?;
        std::fs::write(path, &s)?;
        Ok(())
    }

    /// Extract branch state from an existing repo
    pub fn from_repo(repo: &crate::git::GitRepo) -> Result<Self, git2::Error> {
        let mut branches: Vec<_> = repo
            .local_branches()
            .map(|b| {
                let commit = repo.find_commit(b.id).unwrap();
                Branch {
                    name: b.name,
                    id: b.id,
                    metadata: maplit::btreemap! {
                        "summary".to_owned() => serde_json::Value::String(
                            String::from_utf8_lossy(commit.summary.as_slice()).into_owned()
                        ),
                    },
                }
            })
            .collect();
        branches.sort_unstable();
        let metadata = Default::default();
        Ok(Self { branches, metadata })
    }

    /// Update repo to match the branch state
    pub fn apply(&self, repo: &mut crate::git::GitRepo) -> Result<(), git2::Error> {
        let head_branch = repo.head_branch();
        let head_branch_name = head_branch.as_ref().map(|b| b.name.as_str());
        for branch in self.branches.iter() {
            let existing = repo.find_local_branch(&branch.name);
            if existing.map(|b| b.id) == Some(branch.id) {
                log::trace!("No change for {}", branch.name);
            } else {
                if head_branch_name == Some(branch.name.as_str()) {
                    log::debug!("Restoring {} (HEAD)", branch.name);
                    repo.detach()?;
                    repo.branch(&branch.name, branch.id)?;
                    repo.switch(&branch.name)?;
                } else {
                    log::debug!("Restoring {}", branch.name);
                    repo.branch(&branch.name, branch.id)?;
                }
            }
        }
        Ok(())
    }

    /// Add message metadata
    pub fn insert_message(&mut self, message: &str) {
        self.metadata.insert(
            "message".to_owned(),
            serde_json::Value::String(message.to_owned()),
        );
    }

    /// Add branch-relationship metadata
    pub fn insert_parent(
        &mut self,
        repo: &crate::git::GitRepo,
        branches: &crate::git::Branches,
        protected_branches: &crate::git::Branches,
    ) {
        for branch in self.branches.iter_mut() {
            if let Some(parent) = crate::git::find_base(repo, branches, branch.id)
                .or_else(|| crate::git::find_protected_base(repo, protected_branches, branch.id))
            {
                branch.metadata.insert(
                    "parent".to_owned(),
                    serde_json::Value::String(parent.name.clone()),
                );
            }
        }
    }
}

/// State of an individual branch
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Branch {
    pub name: String,
    #[serde(serialize_with = "serialize_oid")]
    #[serde(deserialize_with = "deserialize_oid")]
    pub id: git2::Oid,
    #[serde(default)]
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

fn serialize_oid<S>(id: &git2::Oid, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let id = id.to_string();
    serializer.serialize_str(&id)
}

fn deserialize_oid<'de, D>(deserializer: D) -> Result<git2::Oid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    git2::Oid::from_str(&s).map_err(serde::de::Error::custom)
}

impl PartialOrd for Branch {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some((&self.name, self.id).cmp(&(&other.name, other.id)))
    }
}

impl Ord for Branch {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.name, self.id).cmp(&(&other.name, other.id))
    }
}
