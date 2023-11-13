use std::collections::VecDeque;
use std::fmt::Display;
use std::fs::{self, create_dir_all, rename};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

use crate::error::already_exist;
use crate::sync::recover::{Status, TryRecover};
use crate::{fix_path, replace};

use super::file::FileInfo;
use super::recover::TryRecoverResult;
use super::{Action, Info};

#[derive(Debug, Clone)]
pub struct DirectoryInfo {
    path: PathBuf,
}
impl Display for DirectoryInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.path.display()))
    }
}

impl DirectoryInfo {
    pub fn children(&self) -> Result<Vec<PathBuf>> {
        read_dir(self.as_path(), |_| true)
    }
    pub fn files(&self) -> Result<Vec<FileInfo>> {
        Ok(read_dir(self.as_path(), |path| path.is_file())?
            .into_iter()
            .map(|path| unsafe { FileInfo::open_uncheck(path) })
            .collect())
    }

    pub fn directories(&self) -> Result<Vec<DirectoryInfo>> {
        Ok(read_dir(self.as_path(), |path| path.is_dir())?
            .into_iter()
            .map(|path| unsafe { DirectoryInfo::open_uncheck(path) })
            .collect())
    }
}

pub fn read_dir<F>(path: impl AsRef<Path>, f: F) -> Result<Vec<PathBuf>>
where
    F: Fn(&PathBuf) -> bool,
{
    let read_dir = fs::read_dir(path)?
        .filter_map(|d| {
            d.ok().and_then(|d| {
                let path = d.path();
                if f(&path) {
                    Some(path)
                } else {
                    None
                }
            })
        })
        .collect();
    Ok(read_dir)
}

impl Action for DirectoryInfo {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = fix_path(path)?;
        if path.is_dir() {
            Ok(DirectoryInfo { path })
        } else {
            Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "The path '{}' is not a directory or does not exist",
                    path.display()
                ),
            ))
        }
    }

    unsafe fn open_uncheck<P: AsRef<Path>>(path: P) -> Self {
        DirectoryInfo {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn rename<T: AsRef<std::ffi::OsStr>>(&mut self, name: T) -> Result<()> {
        let mut path = self.path.clone();
        path.set_file_name(name);
        rename(self.as_path(), &path)?;
        self.path = path;
        Ok(())
    }

    fn copy_new<P: AsRef<Path>>(&self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::CopyDirectory(self, path),
            ));
        }
        _write_dir(self.clone(), &path, true)?;
        Ok(())
    }

    fn move_new<P: AsRef<Path>>(&mut self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::MoveDirectory(self, path),
            ));
        }
        if rename(self.as_path(), path.as_path()).is_err() {
            _write_dir(self.clone(), &path, false)?;
        }
        self.path = path;
        Ok(())
    }
}
pub(crate) fn _write_dir(dir: DirectoryInfo, to: &Path, is_copy: bool) -> Result<()> {
    let mut queue = VecDeque::new();
    let path = dir.path.clone();
    queue.push_back(dir.clone());
    while let Some(dir) = queue.pop_front() {
        queue.append(&mut dir.directories()?.into());
        let dir_path = replace(dir.as_path(), path.as_path(), to);
        if !dir_path.is_dir() {
            create_dir_all(dir_path.as_path())?;
        }
        for mut file in dir.files()? {
            if is_copy {
                file.copy_to(dir_path.as_path())
                    .or_else(|op| op.try_recover())?;
            } else {
                file.move_to(dir_path.as_path())
                    .or_else(|op| op.try_recover())?;
            }
        }
    }
    if !is_copy {
        dir.delete()?;
    }
    Ok(())
}

impl Info for DirectoryInfo {
    fn as_path(&self) -> &Path {
        &self.path
    }

    fn size(&self) -> u64 {
        let mut queue = VecDeque::new();
        queue.push_back(self.as_path().to_path_buf());
        let mut size = 0;
        while let Some(dir) = queue.pop_front() {
            if let Ok(readdir) = std::fs::read_dir(dir) {
                for dir_entry in readdir.flatten() {
                    let path = dir_entry.path();
                    if path.is_dir() {
                        queue.push_back(path)
                    } else {
                        size += path.metadata().map_or(0, |f| f.len());
                    }
                }
            }
        }
        size
    }
}
