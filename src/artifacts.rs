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
    owns_directory: bool,
}

impl Artifacts {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            owned: Vec::new(),
            owns_directory: false,
        }
    }

    pub fn temporary() -> io::Result<Self> {
        let parent = std::env::temp_dir();
        for _ in 0..128 {
            let sequence = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            let directory = parent.join(format!(
                "imglean-check-{}-{timestamp:x}-{sequence:x}",
                std::process::id()
            ));
            match create_temporary_directory(&directory) {
                Ok(()) => {
                    return Ok(Self {
                        directory,
                        owned: Vec::new(),
                        owns_directory: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique ImgLean temporary directory",
        ))
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

fn create_temporary_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;

        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
}

impl Drop for Artifacts {
    fn drop(&mut self) {
        for path in self.owned.drain(..) {
            let _ = fs::remove_file(path);
        }
        if self.owns_directory {
            let _ = fs::remove_dir(&self.directory);
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

    #[test]
    fn temporary_artifacts_remove_their_directory() {
        let directory;
        {
            let mut artifacts = Artifacts::temporary().unwrap();
            directory = artifacts.directory().to_path_buf();
            let _ = artifacts.create("test").unwrap();
            assert!(directory.is_dir());
        }
        assert!(!directory.exists());
    }

    #[cfg(unix)]
    #[test]
    fn temporary_artifacts_use_an_owner_only_directory() {
        use std::os::unix::fs::PermissionsExt as _;

        let artifacts = Artifacts::temporary().unwrap();
        let mode = fs::metadata(artifacts.directory())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
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
