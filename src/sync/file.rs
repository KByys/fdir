use super::recover::{Status, TryRecover, TryRecoverResult};
use super::{Action, Info, _delete_file};
use crate::error::{already_exist, INVALID_PATH};
use crate::{fix_path, get_file_path, is_same_root};
use std::fmt::{Debug, Display};
use std::fs::{copy, create_dir_all, rename, File};
use std::io::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FileInfo {
    path: PathBuf,
}
unsafe impl Send for FileInfo {}

unsafe impl Sync for FileInfo {}

impl Display for FileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.path.display()))
    }
}

impl TryFrom<File> for FileInfo {
    type Error = Error;

    fn try_from(value: File) -> std::result::Result<Self, Self::Error> {
        let path = fix_path(get_file_path(format!("{:?}", value)))?;
        Ok(Self { path })
    }
}

impl FileInfo {
    pub fn create<P: AsRef<Path>>(path: P) -> Result<FileInfo> {
        let path = fix_path(path)?;
        if let Some(parent) = path.parent() {
            if !parent.is_dir() {
                create_dir_all(parent)?;
            }
            File::create(&path)?;
            Ok(Self { path })
        } else {
            INVALID_PATH()
        }
    }

}

impl Action for FileInfo {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        FileInfo::try_from(File::open(path)?)
    }

    unsafe fn open_uncheck<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn rename<T: AsRef<std::ffi::OsStr>>(&mut self, name: T) -> Result<()> {
        let mut new_path = self.path.clone();
        new_path.set_file_name(name.as_ref());
        if let Some(ext) = self.as_path().extension() {
            new_path.set_extension(ext);
        }
        rename(self.as_path(), &new_path)?;
        self.path = new_path;
        Ok(())
    }

    fn copy_new<P: AsRef<Path>>(&self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::CopyFile(self, path),
            ));
        }
        copy(self.as_path(), &path)?;
        Ok(())
    }

    fn move_new<P: AsRef<Path>>(&mut self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::MoveFile(self, path),
            ));
        }
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        } else {
            INVALID_PATH()?;
        }
        if is_same_root(self.as_path(), &path) {
            rename(self.as_path(), &path)?;
        } else {
            copy(self.as_path(), &path)?;
            _delete_file(self)?;
        }
        self.path = path;
        Ok(())
    }
}

impl Info for FileInfo {
    fn as_path(&self) -> &Path {
        &self.path
    }

    fn size(&self) -> u64 {
        self.metadata().map_or(0, |f| f.len())
    }
}
