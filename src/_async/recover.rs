use tokio::fs::{copy, rename};
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

pub type TryRecoverResult<'a, T> = std::result::Result<T, TryRecover<'a>>;

pub enum Status<'a> {
    CopyFile(&'a AsyncFileInfo, PathBuf),
    CopyDirectory(&'a AsyncDirectoryInfo, PathBuf),
    MoveFile(&'a mut AsyncFileInfo, PathBuf),
    MoveDirectory(&'a mut AsyncDirectoryInfo, PathBuf),
}

impl<'a> From<TryRecover<'a>> for Error {
    fn from(value: TryRecover<'a>) -> Self {
        value.error
    }
}

impl<'a> From<Error> for TryRecover<'a> {
    fn from(value: Error) -> Self {
        TryRecover {
            error: value,
            status: None,
        }
    }
}

pub struct TryRecover<'a> {
    pub error: Error,
    pub status: Option<Status<'a>>,
}
use Status::*;

use super::dir::{AsyncDirectoryInfo, _write_dir};
use super::file::AsyncFileInfo;
use super::{remove_file_any, AsyncAction, AsyncInfo};
impl<'a> TryRecover<'a> {
    pub fn new(error: Error, status: Status<'a>) -> TryRecover<'a> {
        Self {
            error,
            status: Some(status),
        }
    }
    pub async fn try_recover(self) -> Result<()> {
        if self.error.kind() == ErrorKind::AlreadyExists {
            let status = match self.status {
                Some(status) => status,
                _ => return Err(self.error),
            };
            match status {
                CopyFile(f, to) => {
                    remove_file_any(&to).await?;
                    copy(f.as_path(), to).await?;
                    Ok(())
                }
                MoveFile(f, to) => {
                    remove_file_any(&to).await?;
                    if rename(f.as_path(), &to).await.is_err() {
                        copy(f.as_path(), &to).await?;
                        remove_file_any(f.as_path()).await?;
                        *f = unsafe { AsyncFileInfo::open_uncheck(to) };
                    }
                    Ok(())
                }
                CopyDirectory(dir, to) => {
                    _write_dir(dir, &to, true).await
                }
                MoveDirectory(dir, to) => {
                    if rename(dir.as_path(), to.as_path()).await.is_err() {
                        _write_dir(dir, &to, false).await?;
                    }
                    *dir = unsafe { AsyncDirectoryInfo::open_uncheck(to) };
                    Ok(())
                }
            }
        } else {
            Err(self.error)
        }
    }
}
