use std::{borrow::Borrow, fmt::Debug, time::Duration};

use base64::Engine;
use log::{error, trace, warn};
use reqwest::{header::*, Client};
use url::Url;

use crate::{
    command::Command,
    error::SFError,
    gamestate::{
        character::{Class, Gender, Race},
        GameState,
    },
    misc::{
        sha1_hash, DEFAULT_CRYPTO_ID, DEFAULT_CRYPTO_KEY, DEFAULT_SESSION_ID,
        HASH_CONST,
    },
};
pub use crate::{misc::decrypt_url, response::*};

#[derive(Debug, Clone)]
/// The session, that manages the server communication for a character
pub struct Session {
    /// The information necessary to log in
    login_data: LoginData,
    /// The server this account is on
    server_url: url::Url,
    /// The id of our session. This will remain the same as long as our login
    /// is valid and nobody else logs in
    session_id: String,
    /// The amount of commands we have send
    player_id: u32,
    login_count: u32,
    crypto_id: String,
    crypto_key: String,
    // We keep this instead of creating a new one, because as per the reqwest
    // docs: "The Client holds a connection pool internally, so it is advised
    // that you create one and reuse it."
    client: reqwest::Client,
    options: ConnectionOptions,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The password of a character, hashed in the way, that the server expects
pub struct PWHash(String);

impl PWHash {
    /// Hashes the password the way the server expects it. You can use this to
    /// store user passwords safely (not in cleartext)
    #[must_use]
    pub fn new(password: &str) -> Self {
        Self(sha1_hash(&(password.to_string() + HASH_CONST)))
    }
    /// If you have access to the hash of the password directly, this method
    /// lets you construct a `PWHash` directly
    #[must_use]
    pub fn from_hash(hash: String) -> Self {
        Self(hash)
    }

    /// Gives you the hash of the password directly
    #[must_use]
    pub fn get(&self) -> &str {
        &self.0
    }
}

impl Session {
    /// Constructs a new session for a normal (not SSO) account with the
    /// credentials provided. To use this session, you should call `login()`
    /// to actually find out, if the credentials work and to get the initial
    /// login response
    #[must_use]
    pub fn new(
        username: &str,
        password: &str,
        server: ServerConnection,
    ) -> Self {
        Self::new_hashed(username, PWHash::new(password), server)
    }

    /// Does the same as `new()`, but takes a hashed password directly
    #[must_use]
    pub fn new_hashed(
        username: &str,
        pw_hash: PWHash,
        server: ServerConnection,
    ) -> Self {
        let ld = LoginData::Basic {
            username: username.to_string(),
            pw_hash,
        };
        Self::new_full(ld, server.client, server.options, server.url)
    }

    fn new_full(
        ld: LoginData,
        client: Client,
        options: ConnectionOptions,
        url: Url,
    ) -> Self {
        Self {
            login_data: ld,
            server_url: url,
            client,
            session_id: DEFAULT_SESSION_ID.to_string(),
            crypto_id: DEFAULT_CRYPTO_ID.to_string(),
            crypto_key: DEFAULT_CRYPTO_KEY.to_string(),
            login_count: 1,
            options,
            player_id: 0,
        }
    }

    /// Resets a session by setting all values related to the server connection
    /// back to the "not logged in" state. This is basically the equivalent of
    /// clearing browserdata, to logout
    fn logout(&mut self) {
        self.crypto_key = DEFAULT_CRYPTO_KEY.to_string();
        self.crypto_id = DEFAULT_CRYPTO_ID.to_string();
        self.login_count = 1;
        self.session_id = DEFAULT_SESSION_ID.to_string();
        self.player_id = 0;
    }

    /// Returns a reference to the server URL, that this session is sending
    /// requests to
    #[must_use]
    pub fn server_url(&self) -> &url::Url {
        &self.server_url
    }

    /// Checks if this session has ever been able to successfully login to the
    /// server to establish a session id. You should not need to check this, as
    /// `login()` should return error on unsuccessful logins, but if you want
    /// to make sure, you can make sure here
    #[must_use]
    pub fn has_session_id(&self) -> bool {
        self.session_id.chars().any(|a| a != '0')
    }

    /// Logges in the session by sending a login response to the server and
    /// updating the internal cryptography values. If the session is currently
    /// logged in, this also clears the existing state beforehand.
    ///
    /// # Errors
    /// Look at `send_command()` to get a full overview of all the
    /// possible errors
    pub async fn login(&mut self) -> Result<Response, SFError> {
        self.logout();
        #[allow(deprecated)]
        let login_cmd = match self.login_data.clone() {
            LoginData::Basic { username, pw_hash } => Command::Login {
                username,
                pw_hash: pw_hash.get().to_string(),
                login_count: self.login_count,
            },
            #[cfg(feature = "sso")]
            LoginData::SSO {
                character_id,
                session,
                ..
            } => Command::SSOLogin {
                uuid: session.uuid,
                character_id,
                bearer_token: session.bearer_token,
            },
        };

        self.send_command(&login_cmd).await
    }

    /// Registers a new character on the server. If everything works, the logged
    /// in character session and its login response will be returned
    ///
    /// # Errors
    /// Look at `send_command()` to get a full overview of all the
    /// possible errors
    pub async fn register(
        username: &str,
        password: &str,
        server: ServerConnection,
        gender: Gender,
        race: Race,
        class: Class,
    ) -> Result<(Self, Response), SFError> {
        let mut s = Self::new(username, password, server);
        #[allow(deprecated)]
        let resp = s
            .send_command(&Command::Register {
                username: username.to_string(),
                password: password.to_string(),
                gender,
                race,
                class,
            })
            .await?;

        let Some(tracking) = resp.values().get("tracking") else {
            error!("Got no tracking response from server after registering");
            return Err(SFError::ParsingError(
                "register response",
                resp.raw_response().to_string(),
            ));
        };

        if tracking.as_str() != "signup" {
            error!("Got something else than signup response during register");
            return Err(SFError::ParsingError(
                "register tracking response",
                tracking.as_str().to_string(),
            ));
        }

        // At this point we are certain, that the server has registered us, so
        // we `should` be able to login
        let resp = s.login().await?;
        Ok((s, resp))
    }

    /// The internal version `send_command()`. It allows you to send
    /// requests with only a normal ref, because this version does not
    /// update the cryptography settings of this session, if the server
    /// responds with them. If you do not expect the server to send you new
    /// crypto settings, because you only do predictable simple requests (no
    /// login, etc), or you want to update them yourself, because that is
    /// easier to handle for you, you can use this function to increase your
    /// commands/account/sec speed
    ///
    /// # Errors
    /// Look at `send_command()` to get a full overview of all the
    /// possible errors
    pub async fn send_command_raw<T: Borrow<Command>>(
        &self,
        command: T,
    ) -> Result<Response, SFError> {
        let command = command.borrow();
        trace!("Sending a {command:?} command");

        let old_cmd = command.request_string()?;
        trace!("Command string: {old_cmd}");

        let (cmd_name, cmd_args) =
            old_cmd.split_once(':').unwrap_or((old_cmd.as_str(), ""));

        let url = format!(
            "{}cmd.php?req={cmd_name}&params={}&sid={}",
            self.server_url,
            base64::engine::general_purpose::URL_SAFE.encode(cmd_args),
            &self.crypto_id,
        );

        trace!("Full request url: {url}");

        // Make sure we dont have any weird stuff in our URL
        url::Url::parse(&url).map_err(|_| {
            SFError::InvalidRequest("Could not parse command url")
        })?;

        #[allow(unused_mut)]
        let mut req = self
            .client
            .get(&url)
            .header(REFERER, &self.server_url.to_string());

        #[cfg(feature = "sso")]
        if let LoginData::SSO { session, .. } = &self.login_data {
            req = req.bearer_auth(&session.bearer_token);
        }
        if self.has_session_id() {
            req = req.header(
                HeaderName::from_static("PG-Session"),
                HeaderValue::from_str(&self.session_id).map_err(|_| {
                    SFError::InvalidRequest("Invalid session id")
                })?,
            );
        }
        req = req.header(
            HeaderName::from_static("PG-Player"),
            HeaderValue::from_str(&self.player_id.to_string())
                .map_err(|_| SFError::InvalidRequest("Invalid player id"))?,
        );

        let resp = req.send().await.map_err(|_| SFError::ConnectionError)?;

        if !resp.status().is_success() {
            return Err(SFError::ConnectionError);
        }

        let response_body =
            resp.text().await.map_err(|_| SFError::ConnectionError)?;

        match response_body {
            body if body.is_empty() => Err(SFError::EmptyResponse),
            body => {
                let resp =
                    Response::parse(body, chrono::Local::now().naive_local())?;
                if let Some(lc) = resp.values().get("serverversion").copied() {
                    let version: u32 = lc.into("server version")?;
                    if version > self.options.expected_server_version {
                        warn!("Untested S&F Server version: {version}");
                        if self.options.error_on_unsupported_version {
                            return Err(SFError::UnsupportedVersion(version));
                        }
                    }
                }
                Ok(resp)
            }
        }
    }

    /// Encode and send a command to the server, decrypts and parses its
    /// response and returns the response. When this returns an error, the
    /// Session might be in an invalid state, so you should login again just to
    /// be safe
    ///
    /// # Errors
    /// - `UnsupportedVersion`: If `error_on_unsupported_version` is set and the
    ///   server is running an unsupported version
    /// - `EmptyResponse`: If the servers response was empty
    /// - `InvalidRequest`: If your response was invalid to send in some way
    /// - `ConnectionError`: If the command could not be send, or the response
    ///   could not successfully be received
    /// - `ParsingError`: If the response from the server was unexpected in some
    ///   way
    /// - `ServerError`: If the server itself responded with an ingame error
    ///   like "you do not have enough silver to do that"
    pub async fn send_command<T: Borrow<Command>>(
        &mut self,
        command: T,
    ) -> Result<Response, SFError> {
        let res = self.send_command_raw(command).await?;
        self.update(&res);
        Ok(res)
    }

    /// Manually updates the cryptography setting of this session with the
    /// response provided
    pub fn update(&mut self, res: &Response) {
        let data = res.values();
        if let Some(lc) = data.get("login count") {
            self.login_count = (*lc).into("login count").unwrap_or_default();
        }
        if let Some(lc) = data.get("sessionid") {
            self.session_id.clear();
            self.session_id.push_str(lc.as_str());
        }
        if let Some(player_id) = data
            .get("ownplayersave")
            .and_then(|a| a.as_str().split('/').nth(1))
            .and_then(|a| a.parse::<u32>().ok())
        {
            self.player_id = player_id;
        }
        if let Some(lc) = data.get("cryptoid") {
            self.crypto_id.clear();
            self.crypto_id.push_str(lc.as_str());
        }
    }

    #[cfg(feature = "sso")]
    pub(super) async fn from_sso_char(
        character: crate::sso::SSOCharacter,
        account: std::sync::Arc<tokio::sync::Mutex<crate::sso::SFAccount>>,
        server_lookup: &crate::sso::ServerLookup,
    ) -> Result<Session, SFError> {
        let url = server_lookup.get(character.server_id)?;
        let session = account.lock().await.session.clone();
        let client = account.lock().await.client.clone();
        let options = account.lock().await.options.clone();

        let ld = LoginData::SSO {
            username: character.name,
            character_id: character.id,
            account,
            session,
        };
        Ok(Session::new_full(ld, client, options, url))
    }

    #[must_use]
    /// The username of the character, that this session is responsible for
    pub fn username(&self) -> &str {
        match &self.login_data {
            LoginData::Basic { username, .. } => username,
            #[cfg(feature = "sso")]
            LoginData::SSO {
                username: character_name,
                ..
            } => character_name,
        }
    }

    #[cfg(feature = "sso")]
    /// Retrieves new sso credentials from its sf account. If the account
    /// already has new creds stored, these are read, otherwise the account will
    /// be logged in again
    ///
    /// # Errors
    /// - `InvalidRequest`: If you call this function with anything other, than
    ///   an SSO-Session
    /// - Other errors, depending on if the session is able to renew the
    ///   credentials
    pub async fn renew_sso_creds(&mut self) -> Result<(), SFError> {
        let LoginData::SSO {
            account, session, ..
        } = &mut self.login_data
        else {
            return Err(SFError::InvalidRequest(
                "Can not renow sso credentials for a non-sso account",
            ));
        };
        let mut account = account.lock().await;

        if &account.session == session {
            account.refresh_login().await?;
        } else {
            *session = account.session.clone();
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::upper_case_acronyms)]
#[non_exhaustive]
enum LoginData {
    Basic {
        username: String,
        pw_hash: PWHash,
    },
    #[cfg(feature = "sso")]
    SSO {
        username: String,
        character_id: String,
        /// A reference to the Account, that owns this character. Used to have
        /// an easy way of renewing credentials.
        account: std::sync::Arc<tokio::sync::Mutex<crate::sso::SFAccount>>,
        /// The SSO account session. We "cache" this to A, not constanty do a
        /// mutex lookup and B, because we have to know, if the accounts
        /// session has changed since we last used it. Otherwise we
        /// could have multiple characters all seeing an expired
        /// session error, which has to be met with a renewal request,
        /// that leads to |characters| many new sessions created. All
        /// but one of which would be thrown away next request, or
        /// (depending on their multi device policy) could lead to an
        /// infinite chain of accounts invalidating their sessions
        /// against each other
        session: crate::sso::AccountSession,
    },
}

#[derive(Debug, Clone)]
/// Stores all information necessary to talk to the server. Notably, if you
/// clone this, instead of creating this multiple times for characters on a
/// server, this will use the same `reqwest::Client`, which can have slight
/// benefits to performance
pub struct ServerConnection {
    url: url::Url,
    client: Client,
    options: ConnectionOptions,
}

impl ServerConnection {
    /// Creates a new server instance. This basically just makes sure the URL
    /// is valid and otherwise tries to make it valid
    #[must_use]
    pub fn new(server_url: &str) -> Option<ServerConnection> {
        ServerConnection::new_with_options(
            server_url,
            ConnectionOptions::default(),
        )
    }

    /// Creates a new server instance with the options provided. This basically
    /// just makes sure the URL is valid and otherwise tries to make it
    /// valid
    #[must_use]
    pub fn new_with_options(
        server_url: &str,
        options: ConnectionOptions,
    ) -> Option<ServerConnection> {
        let url = if server_url.starts_with("http") {
            server_url.parse().ok()?
        } else {
            format!("https://{server_url}").parse().ok()?
        };

        Some(ServerConnection {
            url,
            client: reqwest_client(&options)?,
            options,
        })
    }
}

pub(crate) fn reqwest_client(
    options: &ConnectionOptions,
) -> Option<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static(ACCEPT_LANGUAGE.as_str()),
        HeaderValue::from_static("en;q=0.7,en-US;q=0.6"),
    );
    let mut builder = reqwest::Client::builder();
    if let Some(ua) = options.user_agent.clone() {
        builder = builder.user_agent(ua);
    }
    builder.default_headers(headers).build().ok()
}

#[derive(Debug, Clone)]
/// Options, that change the behaviour of the communication with the server
pub struct ConnectionOptions {
    /// A custom useragent to use, when sending requests to the server
    pub user_agent: Option<String>,
    /// The server version, that this API was last tested on
    pub expected_server_version: u32,
    /// If this is true, any request to the server will error, if the servers
    /// version is greater, than `expected_server_version`. This can be useful,
    /// if you want to make sure you never get surprised by unexpected changes
    /// on the server
    pub error_on_unsupported_version: bool,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            user_agent: Some(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36"
                    .to_string(),
            ),
            expected_server_version: 2005,
            error_on_unsupported_version: false,
        }
    }
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct SimpleSession {
    session: Session,
    gamestate: Option<GameState>,
}

impl SimpleSession {
    async fn short_sleep() {
        tokio::time::sleep(Duration::from_millis(fastrand::u64(1000..2000)))
            .await;
    }

    /// Creates a new `SimpleSession`, by logging in a normal S&F character
    ///
    /// # Errors
    /// Have a look at `send_command` for a full list of possible errors
    pub async fn login(
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<Self, SFError> {
        let connection = ServerConnection::new(server_url)
            .ok_or(SFError::ConnectionError)?;
        let mut session = Session::new(username, password, connection);
        let resp = session.login().await?;
        let gs = GameState::new(resp)?;
        Self::short_sleep().await;
        Ok(Self {
            session,
            gamestate: Some(gs),
        })
    }

    #[cfg(feature = "sso")]
    ///  Creates new `SimpleSession`s, by logging in the S&S SSO account and
    /// returning all the characters associated with the account
    ///
    /// # Errors
    /// Have a look at `send_command` for a full list of possible errors
    pub async fn login_sf_account(
        username: &str,
        password: &str,
    ) -> Result<Vec<Self>, SFError> {
        let acc = crate::sso::SFAccount::login(
            username.to_string(),
            password.to_string(),
        )
        .await?;

        Ok(acc
            .characters()
            .await?
            .into_iter()
            .flatten()
            .map(|a| Self {
                session: a,
                gamestate: None,
            })
            .collect())
    }

    /// Returns a reference to the server URL, that this session is sending
    /// requests to
    #[must_use]
    pub fn server_url(&self) -> &url::Url {
        self.session.server_url()
    }

    /// The username of the character, that this session is responsible for
    #[must_use]
    pub fn username(&self) -> &str {
        self.session.username()
    }

    /// Checks if this session has ever been able to successfully login to the
    /// server to establish a session id. You should not need to check this, as
    /// `login()` should return error on unsuccessful logins, but if you want
    /// to make sure, you can make sure here
    #[must_use]
    pub fn has_session_id(&self) -> bool {
        self.session.has_session_id()
    }

    /// Returns a reference to the game state, if this `SimpleSession` is
    /// currently logged in
    #[must_use]
    pub fn game_state(&self) -> Option<&GameState> {
        self.gamestate.as_ref()
    }

    /// Returns a mutable reference to the game state, if this `SimpleSession`
    /// is currently logged in
    #[must_use]
    pub fn game_state_mut(&mut self) -> Option<&mut GameState> {
        self.gamestate.as_mut()
    }

    /// Sends the command and updates the gamestate with the response from the
    /// server. A mutable reference to the gamestate will be returned. If an
    /// error is encountered, the gamestate is cleared and the error will be
    /// returned. If you send a command after that, this function will try to
    /// login this session again, before sending the provided command
    ///
    /// # Errors
    /// - `EmptyResponse`: If the servers response was empty
    /// - `InvalidRequest`: If your response was invalid to send in some way
    /// - `ConnectionError`: If the command could not be send, or the response
    ///   could not successfully be received
    /// - `ParsingError`: If the response from the server was unexpected in some
    ///   way
    /// - `TooShortResponse` Similar to `ParsingError`, but specific to a
    ///   response being too short, which would normally trigger a out of bound
    ///   panic
    /// - `ServerError`: If the server itself responded with an ingame error
    ///   like "you do not have enough silver to do that"
    #[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
    pub async fn send_command<T: Borrow<Command>>(
        &mut self,
        cmd: T,
    ) -> Result<&mut GameState, SFError> {
        if self.gamestate.is_none() {
            let resp = self.session.login().await?;
            let gs = GameState::new(resp)?;
            self.gamestate = Some(gs);
            Self::short_sleep().await;
        }

        let resp = match self.session.send_command(cmd).await {
            Ok(resp) => resp,
            Err(err) => {
                self.gamestate = None;
                return Err(err);
            }
        };

        if let Some(gs) = &mut self.gamestate {
            if let Err(e) = gs.update(resp) {
                self.gamestate = None;
                return Err(e);
            }
        }

        Ok(self.gamestate.as_mut().unwrap())
    }
}
