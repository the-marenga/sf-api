use std::{
    collections::HashMap,
    fmt::Debug,
    str::FromStr,
    sync::{atomic::AtomicU32, Arc},
};

use base64::Engine;
use chrono::NaiveDateTime;
use log::{error, trace, warn};
use reqwest::{header::*, Client};

use crate::{
    command::Command,
    error::SFError,
    gamestate::character::{Class, Gender, Race},
    misc::{sha1_hash, HASH_CONST},
};

#[derive(Debug, Clone)]
pub struct CharacterSession {
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
pub struct PWHash(String);

impl PWHash {
    /// Hashes the password the way the server expects it. You can use this to
    /// store user passwords safely (not in cleartext)
    pub fn new(password: &str) -> Self {
        Self(sha1_hash(&(password.to_string() + HASH_CONST)))
    }

    pub fn from_hash(hash: String) -> Self {
        Self(hash)
    }

    pub fn get(&self) -> &str {
        &self.0
    }
}

impl CharacterSession {
    pub fn new(
        username: &str,
        password: &str,
        server: ServerConnection,
    ) -> Self {
        Self::new_hashed(username, PWHash::new(password), server)
    }

    pub fn new_hashed(
        username: &str,
        pw_hash: PWHash,
        server: ServerConnection,
    ) -> Self {
        let mut res = Self {
            login_data: LoginData::Basic {
                username: username.to_string(),
                pw_hash,
            },

            server_url: server.url,
            client: server.client,
            session_id: Default::default(),
            crypto_id: Default::default(),
            crypto_key: Default::default(),
            command_count: Default::default(),
            login_count: Default::default(),
            options: server.options,
        };
        res.reset_session();
        res
    }

    /// Resets a session by setting all values related to the server connection
    /// back to the "not logged in" state. This is basically the equivalent of
    /// clearing browserdata, to logout, instead of actually logging out
    fn reset_session(&mut self) {
        self.crypto_key = "[_/$VV&*Qg&)r?~g".to_string();
        self.crypto_id = "0-00000000000000".to_string();
        self.login_count = 1;
        self.command_count = Default::default();
        self.session_id = "00000000000000000000000000000000".to_string();
    }

    pub fn server_url(&self) -> &url::Url {
        &self.server_url
    }

    /// Checks if this session has ever been able to successfully login to the
    /// server to establish a session id. You should not need to check this, as
    /// `login()` should return error on unsuccessfull logins, but if you want
    /// to make sure, you can make sure here
    pub fn has_session_id(&self) -> bool {
        self.session_id.chars().any(|a| a != '0')
    }

    /// Clears the current session and sends a login request to the server.
    /// Returns the parsed response from the server.
    pub async fn login(&mut self) -> Result<Response, SFError> {
        self.reset_session();
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

    /// The internal version `send_command`. It allows you to send requests
    /// with only a normal ref, because this version does not update the
    /// cryptography settings of this session, if the server responds with them.
    /// If you do not expect the server to send you new crypto settings, because
    /// you only do predictable simple requests (no login, etc), or you
    /// want to update them yourself, because that is easier to handle for you,
    /// you can use this function to increase your commands/account/sec speed
    pub async fn send_command_raw(
        &self,
        command: &Command,
    ) -> Result<Response, SFError> {
        trace!("Sending a {command:?} command");

        let mut command_str =
            format!("{}|{}", self.session_id, command.request_string());

        while command_str.len() % 16 > 0 {
            command_str.push('|');
        }

        trace!("Command string: {command_str}");
        let url = format!(
            "{}req.php?req={}{}&rnd={:.7}&c={}",
            self.server_url,
            &self.crypto_id,
            encrypt_server_request(command_str, &self.crypto_key),
            fastrand::f64(), // Pretty sure this is just cache busting
            self.command_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        trace!("Full request url: {url}");

        // Make sure we dont have any weird stuff in our url
        url::Url::parse(&url).map_err(|_| SFError::InvalidRequest)?;

        #[allow(unused_mut)]
        let mut req = self
            .client
            .get(&url)
            .header(REFERER, &self.server_url.to_string());

        #[cfg(feature = "sso")]
        if let LoginData::SSO { session, .. } = &self.login_data {
            req = req.bearer_auth(&session.bearer_token);
        }

        let res = req.send().await.map_err(|_| SFError::ConnectionError)?;

        if !res.status().is_success() {
            return Err(SFError::ConnectionError);
        }

        let response_body =
            res.text().await.map_err(|_| SFError::ConnectionError)?;

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
    pub async fn send_command(
        &mut self,
        command: &Command,
    ) -> Result<Response, SFError> {
        let res = self.send_command_raw(command).await?;
        self.update(&res);
        Ok(res)
    }

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
    ) -> Result<CharacterSession, SFError> {
        let server = server_lookup.get(character.server_id)?;
        let session = account.lock().await.session.clone();
        let client = account.lock().await.client.clone();
        let options = account.lock().await.options.clone();

        Ok(CharacterSession {
            login_data: LoginData::SSO {
                username: character.name,
                character_id: character.id,
                account,
                session,
            },
            server_url: server,
            session_id: "00000000000000000000000000000000".to_string(),
            crypto_id: "0-00000000000000".to_string(),
            crypto_key: "[_/$VV&*Qg&)r?~g".to_string(),
            command_count: Arc::new(AtomicU32::new(1)),
            login_count: 0,
            client,
            options,
        })
    }

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
    pub async fn renew_sso_creds(&mut self) -> Result<(), SFError> {
        let LoginData::SSO {
            account, session, ..
        } = &mut self.login_data
        else {
            return Err(SFError::InvalidRequest);
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
    fn clone(&self) -> Self {
        Self::parse(self.raw_response().to_string(), self.received_at())
            .unwrap()
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
    // Returns a reference to a hashmap, that contains mappings of response keys
    // to values
    pub fn values(&self) -> &HashMap<&str, ResponseVal<'_>> {
        self.borrow_resp()
    }

    /// Returns the raw response from the server. This should only ever be
    /// necessary for debugging, caching, or in case there is ever a new
    /// response format in a response, that is not yet supported. You can of
    /// course also use this to look at how horrible the S&F encoding is..
    pub fn raw_response(&self) -> &str {
        self.borrow_body()
    }

    pub fn received_at(&self) -> NaiveDateTime {
        self.with_received_at(|a| *a)
    }

    /// Parses a response body from the server into a useable format
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
                "adventure index must be 1-3" => {
                    "quest index must be between 0-2"
                }
                x => x,
            };

            return Err(SFError::ServerError(
                error_msg.to_string(),
            ));
        }

        let resp = ResponseBuilder {
            body: og_body,
            resp_builder: |body: &String| {
                let mut res = HashMap::new();
                for part in body
                    .trim_start_matches(|a: char| !a.is_alphabetic())
                    .trim_end_matches('|')
                    .split('&')
                {
                    // a part might look like this: `key.subkey(2):88/99`
                    let base_key_len = part
                        .chars()
                        .position(|a| [':', '(', '.'].contains(&a))
                        .unwrap_or(part.len());

                    let key = part[..base_key_len].trim();

                    if key.is_empty() {
                        continue;
                    }

                    let seperator_pos = part
                        .chars()
                        .position(|a| a == ':')
                        .unwrap_or(part.len());

                    let val_start = (seperator_pos + 1).min(part.len());
                    let val = &part[val_start..];

                    let sub_key = &part
                        [(base_key_len).min(seperator_pos)..seperator_pos]
                        .trim_start_matches('.');

                    res.insert(
                        key,
                        ResponseVal {
                            value: val,
                            sub_key,
                        },
                    );
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
    pub fn into<T: FromStr>(self, name: &'static str) -> Result<T, SFError> {
        use SFError::*;
        self.value
            .trim()
            .parse()
            .map_err(|_| ParsingError(name, self.value.to_string()))
    }

    /// Converts the repsponse into a list, by splitting the raw value by '/'
    /// and converting each value into the required type. If any conversion
    /// fails, an error is returned
    pub fn into_list<T: FromStr>(
        self,
        name: &'static str,
    ) -> Result<Vec<T>, SFError> {
        use SFError::*;
        let x = &self.value;
        if x.is_empty() {
            return Ok(Vec::new());
        }
        // Trimming ` ` & `\n` is not required. Might remove this later
        x.trim_matches(|a| ['/', ' ', '\n'].contains(&a))
            .split('/')
            .map(|c| {
                c.trim()
                    .parse::<T>()
                    .map_err(|_| ParsingError(name, format!("{c:?}")))
            })
            .collect()
    }

    /// The way keys are parsed will trim some info from the string. The key for
    /// the player save `ownplayersave` is actually `ownplayersave.playerSave`.
    /// As this `.playerSave` is not relevant here and not in most cases, I
    /// decided to trim that off. More common, this is also just `s`, `r`, or a
    /// size hint like `(10)`. In some cases though, this information can be
    /// helpful for parsing. Thus, you can access it here
    pub fn sub_key(&self) -> &str {
        self.sub_key
    }

    /// Returns the raw reference to the internal &str, that the server send
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
pub struct ServerConnection {
    url: url::Url,
    client: Client,
    options: ConnectionOptions,
}

impl ServerConnection {
    /// Creates a new server instance. This basically just makes sure the url
    /// is valid and otherwise tries to make it valid
    pub fn new(server_url: &str) -> Option<ServerConnection> {
        ServerConnection::new_with_options(server_url, Default::default())
    }

    pub fn new_with_options(
        server_url: &str,
        options: ConnectionOptions,
    ) -> Option<ServerConnection> {
        let url = match server_url.starts_with("http") {
            true => server_url.parse().ok()?,
            false => format!("https://{server_url}").parse().ok()?,
        };

        Some(ServerConnection {
            url,
            client: reqwest_client(&options)?,
            options,
        })
    }
}

fn encrypt_server_request(to_encrypt: String, key: &str) -> String {
    let mut my_key = [0; 16];
    my_key.copy_from_slice(&key.as_bytes()[..16]);

    let mut cipher = libaes::Cipher::new_128(&my_key);
    cipher.set_auto_padding(false);

    // This feels wrong, but the normal padding does not work. No idea what the
    // default padding strategy is
    let mut to_encrypt = to_encrypt.into_bytes();
    while to_encrypt.len() % 16 != 0 {
        to_encrypt.push(0);
    }
    let encrypted = cipher.cbc_encrypt(CRYPTO_IV.as_bytes(), &to_encrypt);

    base64::engine::general_purpose::URL_SAFE.encode(encrypted)
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
/// s&f web client. It expects a login resonse, which is the ~3KB string
/// response you can see in the network tab of your browser, that starts with
/// `serverversion` after a login. After that, you can take any url the client
/// sends to the server and have it decoded into the actual string command, that
/// was sent. Note that this function technically only needs the crypto key, not
/// the full response, but it is way easier to just copy paste the full response
// just way easier to copy paste
pub fn decrypt_url(encrypted_url: &str, login_resp: &str) -> String {
    let crypto_key = login_resp
        .split('&')
        .flat_map(|a| a.split_once(':'))
        .find(|a| a.0 == "cryptokey")
        .unwrap()
        .1;

    let encrypted = encrypted_url
        .split_once("req=")
        .unwrap()
        .1
        .rsplit_once("&rnd=")
        .unwrap()
        .0;

    let resp = &encrypted["0-00000000000000".len()..];

    decrypt_server_request(resp, crypto_key)
        .split_once('|')
        .unwrap()
        .1
        .trim_end_matches('|')
        .to_string()
}

const CRYPTO_IV: &str = "jXT#/vz]3]5X7Jl\\";

fn decrypt_server_request(to_decrypt: &str, key: &str) -> String {
    let text = base64::engine::general_purpose::URL_SAFE
        .decode(to_decrypt)
        .unwrap();

    let mut my_key = [0; 16];
    my_key.copy_from_slice(&key.as_bytes()[..16]);

    let mut cipher = libaes::Cipher::new_128(&my_key);
    cipher.set_auto_padding(false);
    let decrypted = cipher.cbc_decrypt(CRYPTO_IV.as_bytes(), &text);

    String::from_utf8(decrypted).unwrap()
}

#[derive(Debug, Clone)]
pub struct ConnectionOptions {
    pub user_agent: Option<String>,
    pub expected_server_version: u32,
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
            expected_server_version: 2001,
            error_on_unsupported_version: false,
        }
    }
}
