//! A directory for one test, and nothing else.
//!
//! The tests here run processes and write files, so each one needs somewhere
//! of its own that goes away afterwards.

use std::fs;
use std::path::{Path, PathBuf};
use std::process;

/// A directory made for one test, removed when the test ends.
pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// Make one, named after what the test is about.
    pub fn new(name: &str) -> Scratch {
        let path = std::env::temp_dir().join(format!("githerb-test-{}-{name}", process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();

        Scratch { path }
    }

    /// Where it is.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
