use std::{
    collections::HashMap,
    io,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::Arc,
};

use slotmap::SlotMap;
use tracing::trace;

use crate::{execution::KeySource, scenario::Scenario};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {}", _0)]
    Io(#[source] io::Error),

    #[error("syntax: {}", _0)]
    Syntax(#[source] serde_yaml::Error),

    #[error("path should be relative")]
    PathIsAbsolute,

    #[error("file not found: {:?}", _0)]
    FileNotFound(PathBuf),

    #[error("cyclic reference in source files: {:?}", _0)]
    SourceFileCyclicDependency(PathBuf),
}

#[derive(Debug)]
pub struct Loader {
    pub search_path: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Sources {
    by_effective_path: HashMap<Arc<Path>, KeySource>,
    sources: SlotMap<KeySource, Source>,
}

#[derive(Debug)]
pub struct Source {
    source_file: Arc<Path>,
    scenario: Scenario,
}

impl Loader {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(&self, main: impl Into<PathBuf>) -> Result<Sources, LoadError> {
        let main = main.into();

        let mut sources: Sources = Default::default();
        let mut context = LoaderContext {
            loader: self,
            this_dir: &Path::new("."),
            this_file: &main,
            sources: &mut sources,
        };
        let () = context.load()?;

        Ok(sources)
    }
}

struct LoaderContext<'a> {
    loader: &'a Loader,
    this_dir: &'a Path,
    this_file: &'a Path,
    sources: &'a mut Sources,
}

impl Default for Loader {
    fn default() -> Self {
        Loader {
            search_path: vec![".".into()],
        }
    }
}

impl<'a> LoaderContext<'a> {
    fn load(&mut self) -> Result<(), LoadError> {
        let mut parent_keys: Vec<KeySource> = vec![];
        self.load_inner(&mut parent_keys)
    }
    fn load_inner(&mut self, parent_keys: &mut Vec<KeySource>) -> Result<(), LoadError> {
        let effective_path = self.choose_effective_path()?;
        let source_key = self.read_scenario(effective_path.as_ref())?;

        if parent_keys.iter().any(|pk| *pk == source_key) {
            return Err(LoadError::SourceFileCyclicDependency(effective_path));
        }

        unimplemented!()
    }

    fn choose_effective_path(&self) -> Result<PathBuf, LoadError> {
        if self.this_file.is_absolute() {
            return Err(LoadError::PathIsAbsolute);
        }

        let candidates = std::iter::once(self.this_dir.join(self.this_file)).chain(
            self.loader
                .search_path
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

    fn read_scenario(&mut self, effective_path: &Path) -> Result<KeySource, LoadError> {
        if let Some(key) = self.sources.by_effective_path.get(effective_path).copied() {
            Ok(key)
        } else {
            let source_code = std::fs::read_to_string(effective_path).map_err(LoadError::Io)?;
            let scenario: Scenario =
                serde_yaml::from_str(&source_code).map_err(LoadError::Syntax)?;
            let source_file: Arc<Path> = effective_path.into();
            let source = Source {
                scenario,
                source_file: source_file.clone(),
            };
            let key = self.sources.sources.insert(source);
            self.sources.by_effective_path.insert(source_file, key);

            Ok(key)
        }
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
