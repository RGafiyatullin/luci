use std::{io, path::{Path, PathBuf}};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("io: {}", _0)]
    Io(io::Error),
}

#[derive(Debug)]
pub struct Loader {
    pub search_path: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Sources {
    
}

impl Loader {
    pub fn new() -> Self {Default::default()}

    pub fn load(&self, main: impl Into<PathBuf>) -> Result<Sources, LoadError> {
        let main = main.into();

        let mut sources: Sources = Default::default();
        let mut context = LoaderContext {
            loader: self,
            current_file: &main,
        };
        let () = context.load(&mut sources)?;

        Ok(sources)
    }
}

struct LoaderContext<'a> {
    loader: &'a Loader,
    current_file: &'a Path,
}

impl Default for Loader {
    fn default() -> Self {
        Loader { search_path: vec![".".into()] }
    }
}

impl<'a> LoaderContext<'a> {
    fn load(&mut self, sources: &mut Sources) -> Result<(), LoadError> {
        unimplemented!()
    }
}