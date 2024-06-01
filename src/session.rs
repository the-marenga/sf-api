use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt::Debug,
    str::FromStr,
    sync::{atomic::AtomicU32, Arc},
};

use base64::Engine;
use chrono::NaiveDateTime;
use log::{error, trace, warn};
use reqwest::{header::*, Client};
use url::Url;

use crate::{
    command::Command,
    error::SFError,
    gamestate::character::{Class, Gender, Race},
    misc::{sha1_hash, HASH_CONST},
};

pub(crate) const DEFAULT_CRYPTO_KEY: &str = "[_/$VV&*Qg&)r?~g";
pub(crate) const DEFAULT_CRYPTO_ID: &str = "0-00000000000000";
pub(crate) const DEFAULT_SESSION_ID: &str = "00000000000000000000000000000000";
const CRYPTO_IV: &str = "jXT#/vz]3]5X7Jl\\";

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
    command_count: Arc<AtomicU32>,
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
/// The pasword of a character, hashed in the way, that the server expects
pub struct PWHash(String);

impl PWHash {
    /// Hashes the password the way the server expects it. You can use this to
    /// store user passwords safely (not in cleartext)
    #[must_use]
    pub fn new(password: &str) -> Self {
        Self(sha1_hash(&(password.to_string() + HASH_CONST)))
    }
    /// If you have access to the hash of the password directly, this method
    /// lets you contruct a `PWHash` directly
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
            command_count: Arc::new(AtomicU32::new(0)),
            login_count: 1,
            options,
        }
    }

    /// Resets a session by setting all values related to the server connection
    /// back to the "not logged in" state. This is basically the equivalent of
    /// clearing browserdata, to logout
    fn logout(&mut self) {
        self.crypto_key = DEFAULT_CRYPTO_KEY.to_string();
        self.crypto_id = DEFAULT_CRYPTO_ID.to_string();
        self.login_count = 1;
        self.command_count = Arc::new(AtomicU32::new(0));
        self.session_id = DEFAULT_SESSION_ID.to_string();
    }

    /// Returns a reference to the server url, that this session is sending
    /// requests to
    #[must_use]
    pub fn server_url(&self) -> &url::Url {
        &self.server_url
    }

    /// Checks if this session has ever been able to successfully login to the
    /// server to establish a session id. You should not need to check this, as
    /// `login()` should return error on unsuccessfull logins, but if you want
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

        let mut command_str =
            format!("{}|{}", self.session_id, command.request_string()?);

        while command_str.len() % 16 > 0 {
            command_str.push('|');
        }

        trace!("Command string: {command_str}");
        let url = format!(
            "{}req.php?req={}{}&rnd={:.7}&c={}",
            self.server_url,
            &self.crypto_id,
            encrypt_server_request(command_str, &self.crypto_key)?,
            fastrand::f64(), // Pretty sure this is just cache busting
            self.command_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        trace!("Full request url: {url}");

        // Make sure we dont have any weird stuff in our url
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
        if let Some(lc) = data.get("cryptokey") {
            self.crypto_key.clear();
            self.crypto_key.push_str(lc.as_str());
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
    ///   an
    /// SSO-Session
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

#[ouroboros::self_referencing]
/// A bunch of new information about the state of the server and/or the
/// player
///
/// NOTE: This has a weird syntax to access, because we do not want to create
/// 10000 strings on each request and instead just store the raw response body
/// and references into it. This is faster & uses less memory, but because of
/// rusts borrow checker requires some weird syntax here.
// Technically we could do this safely with an iterator, that parses on demand,
// but send_command() needs to access specifix response keys to keep the session
// running, which means a hashmap needs to be constructed no matter what
pub struct Response {
    body: String,
    #[borrows(body)]
    #[covariant]
    resp: HashMap<&'this str, ResponseVal<'this>>,
    /// We store this to make sure the time calculations are still correct, if
    /// this response is held any amount of time before being used to update
    /// character state
    received_at: NaiveDateTime,
}

impl Clone for Response {
    // This is not a good clone..
    #[allow(clippy::expect_used)]
    fn clone(&self) -> Self {
        Self::parse(self.raw_response().to_string(), self.received_at())
            .expect("Invalid response cloned")
    }
}

impl Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.values().iter().map(|a| (a.0, a.1.as_str())))
            .finish()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Response {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Response", 2)?;
        s.serialize_field("body", self.borrow_body())?;
        s.serialize_field("received_at", &self.received_at())?;
        s.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Response {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AVisitor;

        impl<'de> serde::de::Visitor<'de> for AVisitor {
            type Value = Response;

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str(
                    "struct Response with fields body and received_at",
                )
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut body = None;
                let mut received_at = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "body" => {
                            body = Some(map.next_value()?);
                        }
                        "received_at" => {
                            received_at = Some(map.next_value()?);
                        }
                        _ => {
                            // Ignore unknown fields
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }

                let body: String =
                    body.ok_or_else(|| serde::de::Error::missing_field("q"))?;
                let received_at: NaiveDateTime = received_at
                    .ok_or_else(|| serde::de::Error::missing_field("j"))?;

                Response::parse(body, received_at).map_err(|_| {
                    serde::de::Error::custom("invalid resopnse body")
                })
            }
        }

        deserializer.deserialize_struct(
            "Response",
            &["body", "received_at"],
            AVisitor,
        )
    }
}

impl Response {
    /// Returns a reference to the hashmap, that contains mappings of response
    /// keys to values
    #[must_use]
    pub fn values(&self) -> &HashMap<&str, ResponseVal<'_>> {
        self.borrow_resp()
    }

    /// Returns the raw response from the server. This should only ever be
    /// necessary for debugging, caching, or in case there is ever a new
    /// response format in a response, that is not yet supported. You can of
    /// course also use this to look at how horrible the S&F encoding is..
    #[must_use]
    pub fn raw_response(&self) -> &str {
        self.borrow_body()
    }

    /// Returns the time, at which the response was received
    #[must_use]
    pub fn received_at(&self) -> NaiveDateTime {
        self.with_received_at(|a| *a)
    }

    /// Parses a response body from the server into a useable format
    /// You might want to use this, if you are analyzing reponses from the
    /// browsers network tab. If you are trying to store/read responses to/from
    /// disk to cache them, or otherwise, you should use the sso feature to
    /// serialize/deserialize them instead
    ///
    /// # Errors
    /// - `ServerError`: If the server responsed with an error
    /// - `ParsingError`: If the reponse does not follow the standard S&F server
    ///   response schema
    pub fn parse(
        og_body: String,
        received_at: NaiveDateTime,
    ) -> Result<Response, SFError> {
        // We can not return from the closure below, so we have to do this work
        // twice (sadly)

        // NOTE: I think the trims might actually be completely unnecessary.
        // Pretty sure I mixed them up with command encoding, which is actually
        // '|' padded

        let body = og_body
            .trim_end_matches('|')
            .trim_start_matches(|a: char| !a.is_alphabetic());
        if !body.contains(':')
            && !body.starts_with("success")
            && !body.starts_with("Success")
        {
            return Err(SFError::ParsingError(
                "unexpected server response",
                body.to_string(),
            ));
        }

        if body.starts_with("error") || body.starts_with("Error") {
            let raw_error = body.split_once(':').unwrap_or_default().1;

            let error_msg = match raw_error {
                "adventure index must be 1-3" => "quest index must be 0-2",
                x => x,
            };

            return Err(SFError::ServerError(error_msg.to_string()));
        }

        let resp = ResponseBuilder {
            body: og_body,
            resp_builder: |body: &String| {
                let mut res = HashMap::new();
                for part in body
                    .trim_start_matches(|a: char| !a.is_alphabetic())
                    .trim_end_matches('|')
                    .split('&')
                    .filter(|a| !a.is_empty())
                {
                    let Some((full_key, value)) = part.split_once(':') else {
                        warn!("weird k/v in resp: {part}");
                        continue;
                    };

                    let (key, sub_key) = match full_key.split_once('.') {
                        Some(x) => {
                            // full_key == key.subkey
                            x
                        }
                        None => {
                            if let Some((k, sk)) = full_key.split_once('(') {
                                // full_key == key(4)
                                (k, sk.trim_matches(')'))
                            } else {
                                // full_key == key
                                (full_key, "")
                            }
                        }
                    };
                    if key.is_empty() {
                        continue;
                    }

                    res.insert(key, ResponseVal { value, sub_key });
                }
                res
            },
            received_at,
        }
        .build();

        Ok(resp)
    }
}

#[derive(Debug, Clone, Copy)]
/// This is the raw &str, that the server send as a value to some key. This
/// often requires extra conversions/parsing to use practically, so we associate
/// the most common parsing functions as methods to this data.
pub struct ResponseVal<'a> {
    value: &'a str,
    sub_key: &'a str,
}

impl<'a> ResponseVal<'a> {
    /// Converts the response value into the required type
    ///
    /// # Errors
    /// If the reponse value can not be parsed into the output
    /// value, a `ParsingError` will be returned
    pub fn into<T: FromStr>(self, name: &'static str) -> Result<T, SFError> {
        self.value.trim().parse().map_err(|_| {
            error!("Could not convert {name} into target type: {self}");
            SFError::ParsingError(name, self.value.to_string())
        })
    }

    /// Converts the repsponse into a list, by splitting the raw value by '/'
    /// and converting each value into the required type. If any conversion
    /// fails, an error is returned
    ///
    /// # Errors
    /// If any of the values in the string can not be parsed into the output
    /// value, the `ParsingError` for that value will be returned
    pub fn into_list<T: FromStr>(
        self,
        name: &'static str,
    ) -> Result<Vec<T>, SFError> {
        let x = &self.value;
        if x.is_empty() {
            return Ok(Vec::new());
        }
        // Trimming ` ` & `\n` is not required. Might remove this later
        x.trim_matches(|a| ['/', ' ', '\n'].contains(&a))
            .split('/')
            .map(|c| {
                c.trim().parse::<T>().map_err(|_| {
                    error!("Could not convert {name} into list: {self}");
                    SFError::ParsingError(name, format!("{c:?}"))
                })
            })
            .collect()
    }

    /// The way keys are parsed will trim some info from the string. The key for
    /// the player save `ownplayersave` is actually `ownplayersave.playerSave`.
    /// As this `.playerSave` is not relevant here and not in most cases, I
    /// decided to trim that off. More common, this is also just `s`, `r`, or a
    /// size hint like `(10)`. In some cases though, this information can be
    /// helpful for parsing. Thus, you can access it here
    #[must_use]
    pub fn sub_key(&self) -> &str {
        self.sub_key
    }

    /// Returns the raw reference to the internal &str, that the server send
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.value
    }
}

impl<'a> std::fmt::Display for ResponseVal<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.value)
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
        /// The sso account session. We "cache" this to A, not constanty do a
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
/// Stores all information necessary to talk to the server. Noteably, if you
/// clone this, instead of creating this multiple times for characters on a
/// server, this will use the same `reqwest::Client`, which can have slight
/// benefits to performance
pub struct ServerConnection {
    url: url::Url,
    client: Client,
    options: ConnectionOptions,
}

impl ServerConnection {
    /// Creates a new server instance. This basically just makes sure the url
    /// is valid and otherwise tries to make it valid
    #[must_use]
    pub fn new(server_url: &str) -> Option<ServerConnection> {
        ServerConnection::new_with_options(
            server_url,
            ConnectionOptions::default(),
        )
    }

    /// Creates a new server instance with the options provided. This basically
    /// just makes sure the url is valid and otherwise tries to make it
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

fn encrypt_server_request(
    to_encrypt: String,
    key: &str,
) -> Result<String, SFError> {
    let mut my_key = [0; 16];
    my_key.copy_from_slice(
        key.as_bytes()
            .get(..16)
            .ok_or(SFError::InvalidRequest("Invalid crypto key"))?,
    );

    let mut cipher = libaes::Cipher::new_128(&my_key);
    cipher.set_auto_padding(false);

    // This feels wrong, but the normal padding does not work. No idea what the
    // default padding strategy is
    let mut to_encrypt = to_encrypt.into_bytes();
    while to_encrypt.len() % 16 != 0 {
        to_encrypt.push(0);
    }
    let encrypted = cipher.cbc_encrypt(CRYPTO_IV.as_bytes(), &to_encrypt);

    Ok(base64::engine::general_purpose::URL_SAFE.encode(encrypted))
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

/// This function is designed for reverseengineering encrypted commands from the
/// S&F web client. It expects a login resonse, which is the ~3KB string
/// response you can see in the network tab of your browser, that starts with
/// `serverversion` after a login. After that, you can take any url the client
/// sends to the server and have it decoded into the actual string command, that
/// was sent. Note that this function technically only needs the crypto key, not
/// the full response, but it is way easier to just copy paste the full
/// response. The command returned here will be `Command::Custom`
///
/// # Errors
///
/// If either the url, or the response do not contain the necessary crypto
/// values, an `InvalidRequest` error will be returned, that mentions the part,
/// that is missing or malformed. The same goes for the necessary parts of the
/// decrypted command
pub fn decrypt_url(
    encrypted_url: &str,
    login_resp: Option<&str>,
) -> Result<Command, SFError> {
    let crypto_key = if let Some(login_resp) = login_resp {
        login_resp
            .split('&')
            .filter_map(|a| a.split_once(':'))
            .find(|a| a.0 == "cryptokey")
            .ok_or(SFError::InvalidRequest("No crypto key in login resp"))?
            .1
    } else {
        DEFAULT_CRYPTO_KEY
    };

    let encrypted = encrypted_url
        .split_once("req=")
        .ok_or(SFError::InvalidRequest("url does not contain request"))?
        .1
        .rsplit_once("&rnd=")
        .ok_or(SFError::InvalidRequest("url does not contain rnd"))?
        .0;

    let resp = encrypted.get(DEFAULT_CRYPTO_ID.len()..).ok_or(
        SFError::InvalidRequest("encrypted command does not contain crypto id"),
    )?;
    let full_resp = decrypt_server_request(resp, crypto_key)?;

    let (_session_id, command) = full_resp.split_once('|').ok_or(
        SFError::InvalidRequest("decrypted command has no session id"),
    )?;

    let (cmd_name, args) = command
        .split_once(':')
        .ok_or(SFError::InvalidRequest("decrypted command has no name"))?;
    let args: Vec<_> = args
        .trim_end_matches('|')
        .split('/')
        .map(std::string::ToString::to_string)
        .collect();

    Ok(Command::Custom {
        cmd_name: cmd_name.to_string(),
        arguments: args,
    })
}

fn decrypt_server_request(
    to_decrypt: &str,
    key: &str,
) -> Result<String, SFError> {
    let text = base64::engine::general_purpose::URL_SAFE
        .decode(to_decrypt)
        .map_err(|_| {
            SFError::InvalidRequest("Value to decode is not base64")
        })?;

    let mut my_key = [0; 16];
    my_key.copy_from_slice(
        key.as_bytes()
            .get(..16)
            .ok_or(SFError::InvalidRequest("Key is not 16 bytes long"))?,
    );

    let mut cipher = libaes::Cipher::new_128(&my_key);
    cipher.set_auto_padding(false);
    let decrypted = cipher.cbc_decrypt(CRYPTO_IV.as_bytes(), &text);

    String::from_utf8(decrypted)
        .map_err(|_| SFError::InvalidRequest("Decrypted value is not UTF8"))
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
            expected_server_version: 2004,
            error_on_unsupported_version: false,
        }
    }
}
