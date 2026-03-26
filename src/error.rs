use thiserror::Error;

/// An error, that occurred during the communication (sending/receiving/parsing)
/// of requests to the S&F server
#[derive(Debug, Error)]
#[non_exhaustive]
#[allow(clippy::module_name_repetitions)]
pub enum SFError {
    /// Whatever you were trying to send was not possible to send. This is
    /// either our issue when you were doing something normal, or you were
    /// sending invalid stuff, like a SSO login on a normal logged in character
    #[error("Tried to send an invalid request: {0}")]
    InvalidRequest(&'static str),
    /// The server replied with an empty response. This could have a range of
    /// reasons. Could be a bad request, not logged in, or something else
    #[error("Received an empty response from the server")]
    EmptyResponse,
    /// There was some error encountered when sending data to the server. Most
    /// likely the server, or your connection is down
    #[error("Could not communicate with the server")]
    ConnectionError,
    /// Whatever the server sent back was invalid. Could be because of features
    /// not yet supported, or a bug in the API
    #[error(
        "Error parsing the server response because {0} had an unexpected \
         value of: {1}"
    )]
    ParsingError(&'static str, String),
    /// The server responded with an error. If you are already logged in, this
    /// is likely recoverable,  i.e you are able to reuse your session. You
    /// should just not resend the same command, as the server had some error
    /// with it. Most likely that you were not allowed to do your action (spend
    /// money you don't have, etc.)
    #[error("Server responded with error: {0}")]
    ServerError(String),
    /// The server version is newer, than the limit set in the server
    /// communication
    #[error("The server version {0} is not supported")]
    UnsupportedVersion(u32),
    /// The server responded with a response, that was too short
    #[error(
        "Tried to access the response for {name} at [{pos}] , but the \
         response is too short. The response is: {array}"
    )]
    TooShortResponse {
        /// The name of the item, that was accessed
        name: &'static str,
        /// The position at which the array access failed
        pos: usize,
        /// The full array in debug print
        array: String,
    },
    /// Multiple errors occured when parsing the response
    #[error("Multiple errors occurred:\n{}", .0.iter().map(|e| format!("  - {e}")).collect::<Vec<_>>().join("\n"))]
    NestedError(Vec<SFError>),
}
