use std::{io::{Error, ErrorKind, Result}, path::Path};




pub fn INVALID_PATH<T>() -> Result<T> {
    Err(Error::new(ErrorKind::InvalidInput, "Invalid path"))
}
pub fn already_exist(path: impl AsRef<Path>) -> Error {
    Error::new(ErrorKind::AlreadyExists, format!("The path '{}' already exists!", path.as_ref().display()))
}