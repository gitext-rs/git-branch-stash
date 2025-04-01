#[derive(Default, Clone, Debug)]
pub struct RepoConfig {
    pub protected_branches: Option<Vec<String>>,
    pub capacity: Option<usize>,
}

static STACK_FIELD: &str = "stack.stack";
static PROTECTED_STACK_FIELD: &str = "stack.protected-branch";
static BACKUP_CAPACITY_FIELD: &str = "branch-stash.capacity";

static DEFAULT_PROTECTED_BRANCHES: [&str; 4] = ["main", "master", "dev", "stable"];
const DEFAULT_CAPACITY: usize = 30;

impl RepoConfig {
    pub fn from_all(repo: &git2::Repository) -> eyre::Result<Self> {
        log::trace!("Loading gitconfig");
        let default_config = match git2::Config::open_default() {
            Ok(config) => Some(config),
            Err(err) => {
                log::debug!("Failed to load git config: {err}");
                None
            }
        };
        let config = Self::from_defaults_internal(default_config.as_ref());
        let config = if let Some(default_config) = default_config.as_ref() {
            config.update(Self::from_gitconfig(default_config))
        } else {
            config
        };
        let config = config.update(Self::from_workdir(repo)?);
        let config = config.update(Self::from_repo(repo)?);
        let config = config.update(Self::from_env());
        Ok(config)
    }

    pub fn from_repo(repo: &git2::Repository) -> eyre::Result<Self> {
        let config_path = git_dir_config(repo);
        log::trace!("Loading {}", config_path.display());
        if config_path.exists() {
            match git2::Config::open(&config_path) {
                Ok(config) => Ok(Self::from_gitconfig(&config)),
                Err(err) => {
                    log::debug!("Failed to load git config: {err}");
                    Ok(Default::default())
                }
            }
        } else {
            Ok(Default::default())
        }
    }

    pub fn from_workdir(repo: &git2::Repository) -> eyre::Result<Self> {
        let workdir = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("Cannot read config in bare repository."))?;
        let config_path = workdir.join(".gitconfig");
        log::trace!("Loading {}", config_path.display());
        if config_path.exists() {
            match git2::Config::open(&config_path) {
                Ok(config) => Ok(Self::from_gitconfig(&config)),
                Err(err) => {
                    log::debug!("Failed to load git config: {err}");
                    Ok(Default::default())
                }
            }
        } else {
            Ok(Default::default())
        }
    }

    pub fn from_env() -> Self {
        let mut config = Self::default();

        let params = git_config_env::ConfigParameters::new();
        config = config.update(Self::from_env_iter(params.iter()));

        let params = git_config_env::ConfigEnv::new();
        config = config.update(Self::from_env_iter(
            params.iter().map(|(k, v)| (k, Some(v))),
        ));

        config
    }

    fn from_env_iter<'s>(
        iter: impl Iterator<Item = (std::borrow::Cow<'s, str>, Option<std::borrow::Cow<'s, str>>)>,
    ) -> Self {
        let mut config = Self::default();

        for (key, value) in iter {
            log::trace!("Env config: {key}={value:?}");
            if key == PROTECTED_STACK_FIELD {
                if let Some(value) = value {
                    config
                        .protected_branches
                        .get_or_insert_with(Vec::new)
                        .push(value.into_owned());
                }
            } else if key == BACKUP_CAPACITY_FIELD {
                config.capacity = value.as_deref().and_then(|s| s.parse::<usize>().ok());
            } else {
                log::warn!(
                    "Unsupported config: {}={}",
                    key,
                    value.as_deref().unwrap_or("")
                );
            }
        }

        config
    }

    pub fn from_defaults() -> Self {
        log::trace!("Loading gitconfig");
        let config = match git2::Config::open_default() {
            Ok(config) => Some(config),
            Err(err) => {
                log::debug!("Failed to load git config: {err}");
                None
            }
        };
        Self::from_defaults_internal(config.as_ref())
    }

    fn from_defaults_internal(config: Option<&git2::Config>) -> Self {
        let mut conf = Self::default();

        let mut protected_branches: Vec<String> = Vec::new();
        if let Some(config) = config {
            let default_branch = default_branch(config);
            let default_branch_ignore = default_branch.to_owned();
            protected_branches.push(default_branch_ignore);
        }
        // Don't bother with removing duplicates if `default_branch` is the same as one of our
        // default protected branches
        protected_branches.extend(DEFAULT_PROTECTED_BRANCHES.iter().map(|s| (*s).to_owned()));
        conf.protected_branches = Some(protected_branches);

        conf.capacity = Some(DEFAULT_CAPACITY);

        conf
    }

    pub fn from_gitconfig(config: &git2::Config) -> Self {
        let protected_branches = config
            .multivar(PROTECTED_STACK_FIELD, None)
            .map(|entries| {
                let mut protected_branches = Vec::new();
                entries
                    .for_each(|entry| {
                        if let Some(value) = entry.value() {
                            protected_branches.push(value.to_owned());
                        }
                    })
                    .unwrap();
                if protected_branches.is_empty() {
                    None
                } else {
                    Some(protected_branches)
                }
            })
            .unwrap_or(None);

        let capacity = config
            .get_i64(BACKUP_CAPACITY_FIELD)
            .map(|i| i as usize)
            .ok();

        Self {
            protected_branches,
            capacity,
        }
    }

    pub fn write_repo(&self, repo: &git2::Repository) -> eyre::Result<()> {
        let config_path = git_dir_config(repo);
        log::trace!("Loading {}", config_path.display());
        let mut config = git2::Config::open(&config_path)?;
        log::info!("Writing {}", config_path.display());
        self.to_gitconfig(&mut config)?;
        Ok(())
    }

    pub fn to_gitconfig(&self, config: &mut git2::Config) -> eyre::Result<()> {
        if let Some(protected_branches) = self.protected_branches.as_ref() {
            // Ignore errors if there aren't keys to remove
            let _ = config.remove_multivar(PROTECTED_STACK_FIELD, ".*");
            for branch in protected_branches {
                config.set_multivar(PROTECTED_STACK_FIELD, "^$", branch)?;
            }
        }
        Ok(())
    }

    pub fn update(mut self, other: Self) -> Self {
        match (&mut self.protected_branches, other.protected_branches) {
            (Some(lhs), Some(rhs)) => lhs.extend(rhs),
            (None, Some(rhs)) => self.protected_branches = Some(rhs),
            (_, _) => (),
        }
        self.capacity = other.capacity.or(self.capacity);

        self
    }

    pub fn protected_branches(&self) -> &[String] {
        self.protected_branches.as_deref().unwrap_or(&[])
    }

    pub fn capacity(&self) -> Option<usize> {
        let capacity = self.capacity.unwrap_or(DEFAULT_CAPACITY);
        (capacity != 0).then_some(capacity)
    }
}

impl std::fmt::Display for RepoConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "[{}]", STACK_FIELD.split_once('.').unwrap().0)?;
        for branch in self.protected_branches() {
            writeln!(
                f,
                "\t{}={}",
                PROTECTED_STACK_FIELD.split_once('.').unwrap().1,
                branch
            )?;
        }
        writeln!(f, "[{}]", BACKUP_CAPACITY_FIELD.split_once('.').unwrap().0)?;
        writeln!(
            f,
            "\t{}={}",
            BACKUP_CAPACITY_FIELD.split_once('.').unwrap().1,
            self.capacity().unwrap_or(0)
        )?;
        Ok(())
    }
}

fn git_dir_config(repo: &git2::Repository) -> std::path::PathBuf {
    repo.path().join("config")
}

fn default_branch(config: &git2::Config) -> &str {
    config.get_str("init.defaultBranch").ok().unwrap_or("main")
}
