//! This module is responsible for recursively loading a scenario along with its
//! dependencies.
//!
//! Use a [`SourceCodeLoader`] to load a scenario from the file to a
//! [`SourceCode`].
//!
//! [`SourceCodeLoader`] as a concept of search-path — a list of paths that will
//! be checked for when trying to resolve an included file.
//!
//! Scenarios from the preceding paths may "shadow" those having the same name,
//! but from the paths coming later in the search-path list.
//!
//! Example:
//!
//! ```rust
//! let (entry_point_key, sources) = SourceCodeLoader::new()
//!     .with_search_path([
//!         "../../tests-stdlib",
//!         "../../list-of-episodes",
//!         "../../list-of-seasons",
//!         "tests/this-crate-goodies",
//!     ])
//!     .load("the-one-with-the-blackout.yaml")
//!     .expect("something went awry");
//! ```
//!
//! An instance of [`SourceCode`] contains a list of [scenarios](`Scenario`) in
//! it. It is guaranteed that the all the scenarios are syntactically correct,
//! refer to only existing scenarios, and make no cycles in refering to other
//! scenarios.
//!
//! [`SourceCode`] is essentially a map from some [`KeyScenario`] into a
//! [`SingleScenarioSource`], and a lookup table from the path on the filesystem
//! to a [`KeyScenario`].
//!
//! Each [`SingleScenarioSource`] contains the parsed [`Scenario`],
//! and the table of subroutines refered by this scenario as
//! a map from [`SubroutineName`] to the [`KeyScenario`] corresponding to the
//! subroutine's scenario.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut, Index};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io};

use slotmap::SlotMap;
use tracing::trace;

use crate::execution::KeyScenario;
use crate::names::SubroutineName;
use crate::scenario::Scenario;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {}", _0)]
    Io(#[source] io::Error),

    #[error("syntax: {}", _0)]
    Syntax(#[source] serde_yaml::Error),

    #[error(
        "path should be relative, and should not contain any special components: {:?}",
        _0
    )]
    InvalidPath(PathBuf),

    #[error("file not found: {:?}", _0)]
    FileNotFound(PathBuf),

    #[error("cyclic reference in source files: {:?}", _0)]
    SourceFileCyclicDependency(PathBuf),

    #[error("duplicate subroutine definition: {}", _0)]
    DuplicateSubroutine(SubroutineName),
}

#[derive(Debug)]
pub struct SourceCodeLoader {
    pub search_path: Vec<PathBuf>,
}

#[derive(Default)]
pub struct SourceCode {
    by_effective_path:  BTreeMap<Arc<Path>, KeyScenario>,
    pub(crate) sources: SlotMap<KeyScenario, SingleScenarioSource>,
}

pub struct SingleScenarioSource {
    pub source_file: Arc<Path>,
    pub scenario:    Scenario,
    pub subroutines: BTreeMap<SubroutineName, KeyScenario>,
}

impl Index<KeyScenario> for SourceCode {
    type Output = SingleScenarioSource;

    fn index(&self, index: KeyScenario) -> &Self::Output {
        &self.sources[index]
    }
}

impl SourceCodeLoader {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn reset_search_path(self) -> Self {
        Self {
            search_path: vec![],
            ..self
        }
    }

    pub fn with_search_path<I, P>(mut self, extra_search_path: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.search_path
            .extend(extra_search_path.into_iter().map(Into::into));
        self
    }

    /// Loads the the scenario from the specified path.
    ///
    /// Returns the [`KeySource`] of the entry point along with the [`Sources`].
    pub fn load(
        &self,
        entry_point_scenario: impl Into<PathBuf>,
    ) -> Result<(KeyScenario, SourceCode), LoadError> {
        let main = sanitize_path(&entry_point_scenario.into())?;

        let mut sources: SourceCode = Default::default();
        let mut context = LoaderContext {
            search_path: &self.search_path,
            this_dir:    &Path::new("."),
            this_file:   &main,
            sources:     &mut sources,
        };
        let root_source_key = context.load()?;

        Ok((root_source_key, sources))
    }
}

struct LoaderContext<'a> {
    search_path: &'a [PathBuf],
    this_dir:    &'a Path,
    this_file:   &'a Path,
    sources:     &'a mut SourceCode,
}

impl Default for SourceCodeLoader {
    fn default() -> Self {
        SourceCodeLoader {
            search_path: vec![".".into()],
        }
    }
}

impl<'a> LoaderContext<'a> {
    fn load(&mut self) -> Result<KeyScenario, LoadError> {
        let mut parent_keys: Vec<KeyScenario> = vec![];
        self.load_inner(&mut parent_keys)
    }

    fn load_inner(&mut self, parent_keys: &mut Vec<KeyScenario>) -> Result<KeyScenario, LoadError> {
        let effective_path = self.choose_effective_path()?;
        let (source_key, is_new) = self.read_scenario(effective_path.as_ref())?;

        if parent_keys.iter().any(|pk| *pk == source_key) {
            return Err(LoadError::SourceFileCyclicDependency(effective_path));
        }

        if !is_new {
            return Ok(source_key);
        }

        let source = &self.sources.sources[source_key];
        let base_dir = source.base_dir().to_owned();
        let subroutines = source.scenario.subroutines.clone();
        for import in subroutines {
            let parent_keys = &mut *PopOnDrop::new(parent_keys, source_key);
            let mut context = LoaderContext {
                search_path: self.search_path,
                this_dir:    &base_dir,
                this_file:   &sanitize_path(&import.file_name)?,
                sources:     self.sources,
            };
            let sub_source_key = context.load_inner(parent_keys)?;
            if self.sources.sources[source_key]
                .subroutines
                .insert(import.subroutine_name.clone(), sub_source_key)
                .is_some()
            {
                return Err(LoadError::DuplicateSubroutine(import.subroutine_name));
            }
        }

        Ok(source_key)
    }

    fn choose_effective_path(&self) -> Result<PathBuf, LoadError> {
        if self.this_file.is_absolute() {
            return Err(LoadError::InvalidPath(self.this_file.to_owned()));
        }
        if self
            .this_file
            .components()
            .any(|pc| !matches!(pc, std::path::Component::Normal(_)))
        {
            return Err(LoadError::InvalidPath(self.this_file.to_owned()));
        }

        let candidates = std::iter::once(self.this_dir.join(self.this_file)).chain(
            self.search_path
                .iter()
                .inspect(|p| trace!("search-path candidate: {:?}", p))
                .filter(|search_path| search_path.is_dir())
                .inspect(|p| trace!("is a directory — search path: {:?}", p))
                .map(|search_path| search_path.join(self.this_file))
                .inspect(|f| trace!("source file path candidate: {:?}", f)),
        );
        let effective_path = candidates
            .into_iter()
            .find(|candidate| candidate.is_file())
            .inspect(|f| trace!("resolved {:?} as {:?}", self.this_file, f))
            .ok_or_else(|| LoadError::FileNotFound(self.this_file.to_owned()))?;

        Ok(effective_path)
    }

    fn read_scenario(&mut self, effective_path: &Path) -> Result<(KeyScenario, bool), LoadError> {
        if let Some(key) = self.sources.by_effective_path.get(effective_path).copied() {
            Ok((key, false))
        } else {
            let source_code = std::fs::read_to_string(effective_path).map_err(LoadError::Io)?;
            let scenario: Scenario =
                serde_yaml::from_str(&source_code).map_err(LoadError::Syntax)?;
            let source_file: Arc<Path> = effective_path.into();
            let source = SingleScenarioSource {
                scenario,
                source_file: source_file.clone(),
                subroutines: Default::default(),
            };
            let key = self.sources.sources.insert(source);
            self.sources.by_effective_path.insert(source_file, key);

            Ok((key, true))
        }
    }
}

fn sanitize_path(p: &Path) -> Result<PathBuf, LoadError> {
    use std::path::Component::*;
    p.components()
        .filter_map(|pc| {
            match pc {
                CurDir => None,
                normal @ Normal(_) => Some(Ok(normal)),
                _ => Some(Err(LoadError::InvalidPath(p.to_owned()))),
            }
        })
        .collect::<Result<PathBuf, LoadError>>()
}

impl SingleScenarioSource {
    fn base_dir(&self) -> &Path {
        self.source_file.parent().unwrap_or(Path::new("."))
    }
}

struct PopOnDrop<'a, T>(&'a mut Vec<T>);

impl<'a, T> PopOnDrop<'a, T> {
    fn new(vec: &'a mut Vec<T>, item: T) -> Self {
        vec.push(item);
        Self(vec)
    }
}
impl<'a, T> Deref for PopOnDrop<'a, T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl<'a, T> DerefMut for PopOnDrop<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}
impl<'a, T> Drop for PopOnDrop<'a, T> {
    fn drop(&mut self) {
        self.0.pop();
    }
}

// this implementation is needed for the tests (tests/source_loading.rs)
impl fmt::Debug for SourceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_map();

        for (effective_path, source_key) in self.by_effective_path.iter() {
            f.entry(&effective_path, &self.sources[*source_key]);
        }

        f.finish()
    }
}

// this implementation is needed for the tests (tests/soruce_loading.rs)
impl fmt::Debug for SingleScenarioSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sub_names = self.subroutines.keys().collect::<BTreeSet<_>>();
        f.debug_struct("Source")
            .field("source_file", &self.source_file)
            .field("subs", &sub_names)
            .field("scenario", &self.scenario)
            .finish()
    }
}
