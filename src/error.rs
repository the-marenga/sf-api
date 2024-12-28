use std::{error::Error, fmt::Display};

#[derive(Debug)]
#[non_exhaustive]
#[allow(clippy::module_name_repetitions)]
/// An error, that occurred during the communication (sending/receiving/parsing)
/// of requests to the S&F server
pub enum SFError {
    /// Whatever you were trying to send was not possible to send. This is
    /// either our issue when you were doing something normal, or you were
    /// sending invalid stuff, like a SSO login on a normal logged in character
    InvalidRequest(&'static str),
    /// The server replied with an empty response. This could have a range of
    /// reasons. Could be a bad request, not logged in, or something else
    EmptyResponse,
    /// There was some error encountered when sending data to the server. Most
    /// likely the server, or your connection is down
    ConnectionError,
    /// Whatever the server send back was invalid. Could be because of features
    /// not yet supported, or a bug in the API
    ParsingError(&'static str, String),
    /// The server responded with an error. If you are already logged in, this
    /// is likely recoverable,  i.e you are able to reuse your session. You
    /// should just not resend the same command, as the server had some error
    /// with it. Most likely that you were not allowed to do your action (spend
    /// money you don't have, etc.)
    ServerError(String),
    /// The server version is newer, than the limit set in the server
    /// communication
    UnsupportedVersion(u32),
    /// The server responded with a response, that was too short
    TooShortResponse {
        /// The name of the item, that was accessed
        name: &'static str,
        /// The position at which the array access failed
        pos: usize,
        /// The full array in debug print
        array: String,
    },
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
            SFError::InvalidRequest(msg) => f.write_fmt(format_args!(
                "Tried to send an invalid request: {msg}"
            )),
            SFError::EmptyResponse => {
                f.write_str("Received an empty response from the server")
            }
            SFError::ConnectionError => {
                f.write_str("Could not communicate with the server")
            }
            SFError::ParsingError(name, value) => f.write_fmt(format_args!(
                "Error parsing the server response because {name} had an \
                 unexpected value of: {value}"
            )),
            SFError::ServerError(e) => {
                f.write_fmt(format_args!("Server responded with error: {e}"))
            }
            SFError::UnsupportedVersion(v) => f.write_fmt(format_args!(
                "The server version {v} is not supported"
            )),
            SFError::TooShortResponse { name, pos, array } => {
                f.write_fmt(format_args!(
                    "Tried to access the response for {name} at [{pos}] , but \
                     the response is too short. The response is: {array}"
                ))
            }
        }
    }
}
