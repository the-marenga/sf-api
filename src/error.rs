use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum SFError {
    /// Whatever you were trying to send was not possible to send. This is
    /// either our issue when you were doing something normal, or you were
    /// sending invalid stuff, like a ssologin on a normal logged in character
    InvalidRequest,
    /// The server replied with an empty response. This could have a range of
    /// reasons. Could be a bad request, not logged in, or something else
    EmptyResponse,
    /// There was some error encountered when sending data to the server. Most
    /// likely the server, or your connection is down
    ConnectionError,
    /// Whatever the server send back was invalid. Could be because of features
    /// not yet supported, or a bug in the api
    ParsingError(&'static str, String),
    /// The server responded with an error. If you are already logged in, this
    /// is likely recoverable,  i.e you are able to reuse your session. You
    /// should just not resend the same command, as the server had some error
    /// with it. Most likely that you were not allowed to do your action (spend
    /// money you dont have, etc.)
    ServerError(String),
    /// The server version is newer, than the
    UnsupportedVersion(u32),
}

impl Error for SFError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

impl Display for SFError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SFError::InvalidRequest => {
                f.write_str("Tried to send an invalid request")
            }
            SFError::EmptyResponse => {
                f.write_str("Received an empty response from the server")
            }
            SFError::ConnectionError => {
                f.write_str("Could not communicate with the server")
            }
            SFError::ParsingError(name, value) => f.write_str(&format!(
                "Error parsing the server response because {name} had an \
                 unexpected value of: {value}"
            )),
            SFError::ServerError(e) => {
                f.write_str(&format!("Server responded with error: {e}"))
            }
            SFError::UnsupportedVersion(v) => {
                f.write_str(&format!("The server version {v} is not supported"))
            }
        }
    }
}
