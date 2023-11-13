use std::{
    collections::VecDeque,
    ffi::OsStr,
    fmt::Display,
    fs::Metadata,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
};

use crate::{error::already_exist, fix_path, replace};

use super::{
    file::AsyncFileInfo,
    recover::{Status, TryRecover, TryRecoverResult},
    AsyncAction, AsyncInfo,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use std::io::Result;
use tokio::fs::{self, create_dir_all, metadata, rename};

#[derive(Clone, Debug)]
pub struct AsyncDirectoryInfo {
    path: PathBuf,
}

impl Display for AsyncDirectoryInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.path.display()))
    }
}

impl AsyncDirectoryInfo {
    pub async fn children(&self) -> Result<Vec<PathBuf>> {
        read_dir(self.as_path(), |_| true).await
    }
    pub async fn files(&self) -> Result<Vec<AsyncFileInfo>> {
        read_dir(self.as_path(), |path| path.is_file())
            .await
            .map(|op| {
                op.into_iter()
                    .map(|path| unsafe { AsyncAction::open_uncheck(path) })
                    .collect()
            })
    }
    pub async fn directories(&self) -> Result<Vec<AsyncDirectoryInfo>> {
        read_dir(self.as_path(), |path| path.is_dir())
            .await
            .map(|op| {
                op.into_iter()
                    .map(|path| unsafe { AsyncAction::open_uncheck(path) })
                    .collect()
            })
    }
}

pub async fn read_dir<F>(path: impl AsRef<Path>, f: F) -> Result<Vec<PathBuf>>
where
    F: Fn(&PathBuf) -> bool,
{
    let mut read_dir = fs::read_dir(path).await?;
    let mut children = Vec::new();
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if f(&path) {
            children.push(path)
        }
    }
    Ok(children)
}

#[async_trait]
impl AsyncInfo for AsyncDirectoryInfo {
    fn as_path(&self) -> &Path {
        self.path.as_path()
    }
    async fn metadata(&self) -> Result<Metadata> {
        metadata(self.as_path()).await
    }
    async fn size(&self) -> u64 {
        self.metadata().await.map_or(0, |f| f.len())
    }
}

#[async_trait]
impl AsyncAction for AsyncDirectoryInfo {
    async fn open<P: AsRef<Path> + Send + Sync>(path: P) -> Result<Self> {
        let path = fix_path(path)?;
        if path.is_dir() {
            Ok(Self { path })
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
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
    /// Rename a file or directory
    async fn rename<T: AsRef<OsStr> + Send + Sync>(&mut self, name: T) -> Result<()> {
        let mut new_path = self.path.clone();
        new_path.set_file_name(name.as_ref());
        rename(self.as_path(), &new_path).await?;
        self.path = new_path;
        Ok(())
    }
    async fn copy_new<P: AsRef<Path> + Send + Sync>(&self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::CopyDirectory(self, path),
            ));
        }
        _write_dir(self, &path, true).await?;
        Ok(())
    }
    async fn move_new<P: AsRef<Path> + Send + Sync>(&mut self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::MoveDirectory(self, path),
            ));
        }
        if rename(self.as_path(), path.as_path()).await.is_err() {
            _write_dir(self, &path, false).await?;
        }
        self.path = path;
        Ok(())
    }
}
#[async_recursion]
pub(crate) async fn _write_dir(dir: &AsyncDirectoryInfo, to: &Path, is_copy: bool) -> Result<()> {
    let mut queue = VecDeque::new();
    let path = dir.path.clone();
    queue.push_back(dir.clone());
    while let Some(dir) = queue.pop_front() {
        queue.append(&mut dir.directories().await?.into());
        let dir_path = replace(dir.as_path(), path.as_path(), to);
        if !dir_path.is_dir() {
            create_dir_all(dir_path.as_path()).await?;
        }
        for mut file in dir.files().await? {
            let result = if is_copy {
                file.copy_to(dir_path.as_path()).await
            } else {
                file.move_to(dir_path.as_path()).await
            };
            match result {
                Ok(_) => (),
                Err(e) => e.try_recover().await?,
            }
        }
    }
    if !is_copy {
        dir.clone().delete().await?;
    }
    Ok(())
}
