use super::dir::_write_dir;
use super::{dir::DirectoryInfo, file::FileInfo};
use super::{remove_file_any, Action, Info};
use std::fs::{copy, rename};
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

pub type TryRecoverResult<'a, T> = std::result::Result<T, TryRecover<'a>>;

pub enum Status<'a> {
    CopyFile(&'a FileInfo, PathBuf),
    CopyDirectory(&'a DirectoryInfo, PathBuf),
    MoveFile(&'a mut FileInfo, PathBuf),
    MoveDirectory(&'a mut DirectoryInfo, PathBuf),
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
impl<'a> TryRecover<'a> {
    pub fn new(error: Error, status: Status<'a>) -> TryRecover<'a> {
        Self {
            error,
            status: Some(status),
        }
    }
    pub fn try_recover(self) -> Result<()> {
        if self.error.kind() == ErrorKind::AlreadyExists {
            let status = match self.status {
                Some(status) => status,
                _ => return Err(self.error),
            };
            match status {
                CopyFile(f, to) => {
                    remove_file_any(&to)?;
                    copy(f.as_path(), to)?;
                    Ok(())
                }
                MoveFile(f, to) => {
                    remove_file_any(&to)?;
                    if rename(f.as_path(), &to).is_err() {
                        copy(f.as_path(), &to)?;
                        remove_file_any(f.as_path())?;
                        *f = unsafe { FileInfo::open_uncheck(to) };
                    }
                    Ok(())
                }
                CopyDirectory(dir, to) => {
                    _write_dir(dir.clone(), &to, true)
                }
                MoveDirectory(dir, to) => {
                    _write_dir(dir.clone(), &to, true)?;
                    *dir = unsafe {
                        DirectoryInfo::open_uncheck(to)
                    };
                    Ok(())
                }
            }
        } else {
            Err(self.error)
        }
    }
}
