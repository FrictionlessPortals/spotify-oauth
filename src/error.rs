//! Error Type for the API.

use std::error::Error;
use std::fmt;

/// Generic Result for the Library
pub type SpotifyResult<T> = Result<T, SpotifyError>;

/// For distinguishing error kinds in results.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// The required value could not be parsed.
    ParsingFailed,
    /// The request to an URL failed.
    RequestFailed,
    /// The received callback URL could not be parsed.
    InvalidCallbackURL,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorKind::ParsingFailed => write!(f, "The value failed to parse."),
            ErrorKind::RequestFailed => write!(f, "The request to the URL failed."),
            ErrorKind::InvalidCallbackURL => write!(f, "The callback URL is invalid."),
        }
    }
}

/// General Spotify Error information.
#[derive(Debug)]
pub struct SpotifyError {
    kind: ErrorKind,
    context: Option<String>,
    cause: Option<Box<Error + Send + Sync + 'static>>,
}

impl SpotifyError {
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            context: None,
            cause: None,
        }
    }

    pub fn set_context<S>(mut self, context: S) -> Self
    where
        S: Into<String>,
    {
        let context = context.into();
        self.context = Some(context);
        self
    }

    pub fn set_cause<E>(mut self, cause: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        let cause = Box::new(cause);
        self.cause = Some(cause);
        self
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl Error for SpotifyError {
    fn description(&self) -> &str {
        "Spotify Authentication Flow failed."
    }

    fn cause(&self) -> Option<&Error> {
        self.cause.as_ref().map(|c| {
            let c: &Error = c.as_ref();
            c
        })
    }
}

impl fmt::Display for SpotifyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Spotify Authentication Flow failed: {}", self.kind)?;
        if let Some(ref context) = self.context {
            writeln!(f, "{}", context)?;
        }
        if let Some(ref cause) = self.cause {
            writeln!(f, "Cause: {}", cause)?;
        }
        Ok(())
    }
}
