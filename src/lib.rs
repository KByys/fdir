// pub mod _async;
pub mod convert;
#[allow(non_snake_case)]
pub(crate) mod error;
pub mod sync;
use std::io::Result;
use std::{
    env::current_dir,
    ffi::OsStr,
    path::{Path, PathBuf},
};
pub use self::sync::*;
use error::*;

fn push_file_name<P: AsRef<Path>>(file_name: Option<&OsStr>, path: P) -> Result<PathBuf> {
    let mut path = path.as_ref().to_path_buf();
    let file_name = match file_name {
        Some(file_name) => file_name,
        _ => return INVALID_PATH(),
    };
    path.push(file_name);
    Ok(path)
}

fn is_same_root(path: &Path, to: &Path) -> bool {
    let mut path = path.to_path_buf();
    while path.pop() {}
    to.starts_with(path)
}

pub struct Recorder {
    pub pos: u64,
    pub len: u64
}
impl Recorder {
    pub fn read(len: u64) -> Self {
        Self { pos: 0, len }
    }
    pub fn write(pos: u64) -> Self {
        Self { pos, len: 0 }
    }
    #[allow(non_snake_case)]
    pub fn EOF(&self) -> bool {
        self.pos >= self.len
    }

}


pub(crate) fn fix_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let mut builder = if path.is_absolute() {
        PathBuf::new()
    } else {
        current_dir()?
    };
    for os in path.iter() {
        push_os_str(os, &mut builder)?
    }
    Ok(builder)
}

fn push_os_str(os_str: &OsStr, path: &mut PathBuf) -> Result<()> {
    let pat = os_str.to_string_lossy();
    match pat.as_ref() {
        "." => (),
        "~" => *path = dirs::home_dir().expect("'~' is not supported in the current system"),
        ".." => {
            path.pop();
        }
        _ => path.push(os_str),
    }
    Ok(())
}

pub fn get_file_path(file_debug: String) -> String {
    let split_path: Vec<&str> = file_debug.split('\"').collect();
    let path = split_path[1].to_string();
    #[cfg(windows)]
    let path = path.replace("\\\\", "\\").replace(r"\\?\", "");
    path
}

#[inline]
pub(crate) fn replace(path: &Path, base: &Path, to: &Path) -> PathBuf {
    let path_queue: Vec<_> = path.iter().collect();
    let base_queue: Vec<_> = base.iter().collect();
    let mut to = to.to_path_buf();
    let mut i = 0;
    while i < base_queue.len() && path_queue.get(i) == base_queue.get(i) {
        i += 1;
    }
    for os in path_queue.iter().skip(i) {
        to.push(os)
    }
    to
}