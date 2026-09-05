use std::fmt;

/// 利用者向けの日本語メッセージを持つエラー。
#[derive(Debug)]
pub struct Error(pub String);

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn new<S: Into<String>>(message: S) -> Error {
        Error(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error(e.to_string())
    }
}

/// 文脈を添えて Result を Error に変換するためのヘルパー。
pub trait Context<T> {
    fn context<S: AsRef<str>>(self, what: S) -> Result<T>;
}

impl<T, E: fmt::Display> Context<T> for std::result::Result<T, E> {
    fn context<S: AsRef<str>>(self, what: S) -> Result<T> {
        self.map_err(|e| Error(format!("{}: {}", what.as_ref(), e)))
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
