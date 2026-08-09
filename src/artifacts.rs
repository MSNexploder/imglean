use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ARTIFACT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct Artifacts {
    directory: PathBuf,
    owned: Vec<PathBuf>,
}

impl Artifacts {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            owned: Vec::new(),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn create(&mut self, purpose: &str) -> io::Result<(PathBuf, File)> {
        for _ in 0..128 {
            let path = self.next_path(purpose);
            match OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    self.owned.push(path.clone());
                    return Ok((path, file));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique ImgLean artifact",
        ))
    }

    pub fn reserve_path(&mut self, purpose: &str) -> io::Result<PathBuf> {
        for _ in 0..128 {
            let path = self.next_path(purpose);
            match fs::symlink_metadata(&path) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    self.owned.push(path.clone());
                    return Ok(path);
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a unique ImgLean artifact path",
        ))
    }

    pub fn forget(&mut self, path: &Path) {
        if let Some(index) = self.owned.iter().position(|owned| owned == path) {
            self.owned.swap_remove(index);
        }
    }

    pub fn remove(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)?;
        self.forget(path);
        Ok(())
    }

    fn next_path(&self, purpose: &str) -> PathBuf {
        let sequence = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        self.directory.join(format!(
            ".imglean-{}-{timestamp:x}-{sequence:x}-{purpose}",
            std::process::id()
        ))
    }
}

impl Drop for Artifacts {
    fn drop(&mut self) {
        for path in self.owned.drain(..) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn drop_removes_only_owned_paths() {
        let directory = test_directory();
        let earlier = directory.join(".imglean-earlier");
        fs::write(&earlier, b"earlier").unwrap();
        let owned;
        {
            let mut artifacts = Artifacts::new(directory.clone());
            (owned, _) = artifacts.create("test").unwrap();
            assert!(owned.exists());
        }
        assert!(!owned.exists());
        assert!(earlier.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reserved_path_is_absent_but_owned_for_cleanup() {
        let directory = test_directory();
        let reserved;
        {
            let mut artifacts = Artifacts::new(directory.clone());
            reserved = artifacts.reserve_path("candidate").unwrap();
            assert!(!reserved.exists());
            fs::write(&reserved, b"candidate").unwrap();
        }
        assert!(!reserved.exists());
        fs::remove_dir_all(directory).unwrap();
    }

    fn test_directory() -> PathBuf {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "imglean-artifact-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
}
