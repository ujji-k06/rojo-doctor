pub mod missing_path;

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("could not inspect mapped path `{path}`: {source}")]
    InspectMappedPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
