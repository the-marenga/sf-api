use std::{collections::HashMap, sync::Arc};

use chrono::{NaiveDateTime, Utc};
use reqwest::{
    Client,
    header::{ACCEPT, HeaderMap, HeaderValue, REFERER},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use url::Url;

use crate::{
    error::SFError,
    misc::sha1_hash,
    session::{ConnectionOptions, PWHash, Session, reqwest_client},
};

#[derive(Debug)]
#[allow(dead_code)]
enum SSOAuthData {
    SF { pw_hash: PWHash },
    Google,
    Steam,
}

/// A logged-in Shakes & Fidget SSO (Single Sign-On) account.
#[derive(Debug)]
pub struct SFAccount {
    pub(super) username: String,
    auth: SSOAuthData,
    pub(super) session: AccountSession,
    pub(super) client: Client,
    pub(super) options: ConnectionOptions,
}

/// Holds the active session credentials (UUID and bearer token) for an SSO
/// account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSession {
    pub(super) uuid: String,
    pub(super) bearer_token: String,
}

// This could just be a Login/Characters thing, but who knows what else will be
// integrated into this API. Maybe Register?
#[derive(Debug)]
enum APIRequest {
    Get,
    Post {
        parameters: Vec<&'static str>,
        form_data: HashMap<String, String>,
    },
}

/// Information about a character associated with an SSO account.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SSOCharacter {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) server_id: i32,
}
impl SFAccount {
    /// Returns the username of this S&F account
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Initializes a `SFAccount` by logging the user in using the supplied
    /// clear text credentials
    ///
    /// # Errors
    /// May return basically every possible `SFError` variant, because we are
    /// both sending a command and parsing the result
    // TODO: Document what wring pw returns
    pub async fn login(
        username: String,
        password: String,
    ) -> Result<SFAccount, SFError> {
        Self::login_with_options(
            username,
            password,
            ConnectionOptions::default(),
        )
        .await
    }

    /// Initializes a `SFAccount` by logging the user in using the supplied
    /// clear text credentials and the provided options to use for the
    /// communication with the server
    ///
    /// # Errors
    /// May return basically every possible `SFError` variant, because we are
    /// both sending a command and parsing the result
    pub async fn login_with_options(
        username: String,
        password: String,
        options: ConnectionOptions,
    ) -> Result<SFAccount, SFError> {
        let pw_hash = PWHash::new(&password);
        Self::login_hashed_with_options(username, pw_hash, options).await
    }

    /// Initializes a `SFAccount` by logging the user in using the hashed
    /// password
    ///
    /// # Errors
    /// May return basically every possible `SFError` variant, because we are
    /// both sending a command and parsing the result
    pub async fn login_hashed(
        username: String,
        pw_hash: PWHash,
    ) -> Result<SFAccount, SFError> {
        Self::login_hashed_with_options(
            username,
            pw_hash,
            ConnectionOptions::default(),
        )
        .await
    }

    /// Initializes a `SFAccount` by logging the user in using the hashed
    /// password and the provided options to use for communication
    ///
    /// # Errors
    /// May return basically every possible `SFError` variant, because we are
    /// both sending a command and parsing the result
    pub async fn login_hashed_with_options(
        username: String,
        pw_hash: PWHash,
        options: ConnectionOptions,
    ) -> Result<SFAccount, SFError> {
        let mut tmp_self = Self {
            username,
            auth: SSOAuthData::SF { pw_hash },
            session: AccountSession {
                uuid: String::new(),
                bearer_token: String::new(),
            },
            client: reqwest_client(&options).ok_or(SFError::ConnectionError)?,
            options,
        };

        tmp_self.refresh_login().await?;
        Ok(tmp_self)
    }

    /// Refreshes the SSO session by logging in again with the stored
    /// credentials. This can be used when the server removed our session
    /// either for being connected too long, or the server was
    /// restarted/cache cleared and forgot us
    ///
    /// # Errors
    /// `InvalidRequest` if you call this with anything else then a SSO/SF
    /// account character. Even Google/Steam accounts will not work.
    pub async fn refresh_login(&mut self) -> Result<(), SFError> {
        let SSOAuthData::SF { pw_hash } = &self.auth else {
            // I do not think there is a way to reauth without going through
            // the SSO process again for these
            return Err(SFError::InvalidRequest(
                "Refreshing the SSO-login is only supported for SSO-Accounts",
            ));
        };

        let mut form_data = HashMap::new();
        form_data.insert("username".to_string(), self.username.clone());
        form_data.insert(
            "password".to_string(),
            sha1_hash(&(pw_hash.get().to_string() + "0")),
        );

        let res = self
            .send_api_request(
                "json/login",
                APIRequest::Post {
                    parameters: vec![
                        "client_id=i43nwwnmfc5tced4jtuk4auuygqghud2yopx",
                        "auth_type=access_token",
                    ],
                    form_data,
                },
            )
            .await?;

        #[allow(clippy::indexing_slicing)]
        let (Some(bearer_token), Some(uuid)) = (
            val_to_string(&res["token"]["access_token"]),
            val_to_string(&res["account"]["uuid"]),
        ) else {
            return Err(SFError::ParsingError(
                "missing auth value in api response",
                format!("{res:?}"),
            ));
        };

        self.session = AccountSession { uuid, bearer_token };

        Ok(())
    }

    /// Queries the SSO for all characters associated with this account. This
    /// consumes the Account, as the character sessions may need to refresh
    /// the accounts session, which they are only allowed to do, if they own it
    /// (in an Arc<Mutex<_>>) and there should be no need to keep the account
    /// around anyways
    ///
    /// # Errors
    /// May return `ParsingError` if the server changed its API, or
    /// `ConnectionError`, if the server could not be reached. The characters
    /// in the Vec may be `InvalidRequest`, iff the server the
    /// character would be on could not be determined
    pub async fn characters(
        self,
    ) -> Result<Vec<Result<Session, SFError>>, SFError> {
        // This could be passed in as an argument in case of multiple SSO
        // accounts to safe on requests, but I dont think people have multiple
        // and this is way easier
        let server_lookup =
            ServerLookup::fetch_with_client(&self.client).await?;
        let mut res = self
            .send_api_request("json/client/characters", APIRequest::Get)
            .await?;

        #[allow(clippy::indexing_slicing)]
        let characters: Vec<SSOCharacter> =
            serde_json::from_value(res["characters"].take()).map_err(|_| {
                SFError::ParsingError("missing json value ", String::new())
            })?;

        let account = Arc::new(Mutex::new(self));

        let mut chars = vec![];
        for char in characters {
            chars.push(
                Session::from_sso_char(char, account.clone(), &server_lookup)
                    .await,
            );
        }

        Ok(chars)
    }

    async fn send_api_request(
        &self,
        endpoint: &str,
        method: APIRequest,
    ) -> Result<Value, SFError> {
        send_api_request(
            &self.client,
            &self.session.bearer_token,
            endpoint,
            method,
        )
        .await
    }
}

/// Send a request to the SSO server. The endpoint will be "json/*". We try
/// to check if the response is bad in any way, but S&F responses never obey
/// to HTML status codes, or their own system, so good luck
#[allow(clippy::items_after_statements)]
async fn send_api_request(
    client: &Client,
    bearer_token: &str,
    endpoint: &str,
    method: APIRequest,
) -> Result<Value, SFError> {
    let mut url = url::Url::parse("https://sso.playa-games.com")
        .map_err(|_| SFError::ConnectionError)?;
    url.set_path(endpoint);

    let mut request = match method {
        APIRequest::Get => client.get(url.as_str()),
        APIRequest::Post {
            parameters,
            form_data,
        } => {
            url.set_query(Some(&parameters.join("&")));
            client.post(url.as_str()).form(&form_data)
        }
    };

    // Set all necessary header values to make our request succeed
    if !bearer_token.is_empty() {
        request = request.bearer_auth(bearer_token);
    }
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        REFERER,
        HeaderValue::from_str(url.authority())
            .map_err(|_| SFError::ConnectionError)?,
    );

    let res = request
        .headers(headers)
        .send()
        .await
        .map_err(|_| SFError::ConnectionError)?;
    if !res.status().is_success() {
        return Err(SFError::ConnectionError);
    }
    let text = res.text().await.map_err(|_| SFError::ConnectionError)?;

    #[derive(Debug, Serialize, Deserialize)]
    struct APIResponse {
        success: bool,
        status: u8,
        data: Option<Value>,
        message: Option<Value>,
    }

    let resp: APIResponse = serde_json::from_str(&text)
        .map_err(|_| SFError::ParsingError("API response", text))?;

    if !resp.success {
        return Err(SFError::ConnectionError);
    }
    let data = match resp.data {
        Some(data) => data,
        None => match resp.message {
            Some(message) => message,
            None => return Err(SFError::ConnectionError),
        },
    };

    Ok(data)
}

/// Detailed information about a Shakes & Fidget game server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// The unique numeric ID of the server.
    pub id: i32,
    /// The canonical URL of this server.
    pub url: Url,
    /// The country or region code for this server (e.g. "de", "en", "int",
    /// "fu").
    pub country_code: String,
    /// The target server URL if this server was merged into another server.
    pub merged_into: Option<Url>,
    /// The date and time when the merge takes effect.
    pub merge_date_time: Option<NaiveDateTime>,
}

impl ServerInfo {
    /// Returns whether this server has been merged into another server.
    ///
    /// A server is considered merged if a merge target URL is set and the
    /// merge date and time (if specified) has already passed.
    #[must_use]
    pub fn is_merged(&self) -> bool {
        self.merged_into.is_some()
            && self
                .merge_date_time
                .is_none_or(|mdt| Utc::now().naive_utc() > mdt)
    }
}

/// A lookup table mapping numeric server IDs to their respective
/// [`ServerInfo`].
#[derive(Debug, Clone)]
pub struct ServerLookup(HashMap<i32, ServerInfo>);

impl ServerLookup {
    /// Fetches the current mapping of numeric server IDs to their server info.
    ///
    /// # Errors
    /// Returns `ConnectionError` if the server could not be reached, or
    /// `ParsingError` if the server list could not be parsed.
    pub async fn fetch() -> Result<ServerLookup, SFError> {
        Self::fetch_with_client(&reqwest::Client::new()).await
    }

    /// Fetches the current mapping of server IDs using the provided connection
    /// options.
    ///
    /// # Errors
    /// Returns `ConnectionError` if client initialization fails or the server
    /// could not be reached, or `ParsingError` if the response could not be
    /// parsed.
    pub async fn fetch_with_options(
        options: &ConnectionOptions,
    ) -> Result<ServerLookup, SFError> {
        let client = reqwest_client(options).ok_or(SFError::ConnectionError)?;
        Self::fetch_with_client(&client).await
    }

    /// Fetches the current mapping of server IDs to server info using the
    /// provided client.
    #[allow(clippy::items_after_statements)]
    async fn fetch_with_client(
        client: &Client,
    ) -> Result<ServerLookup, SFError> {
        let res = client
            .get("https://sfgame.net/config.json")
            .send()
            .await
            .map_err(|_| SFError::ConnectionError)?
            .text()
            .await
            .map_err(|_| SFError::ConnectionError)?;

        #[derive(Debug, Deserialize)]
        struct ServerResp {
            servers: Vec<RawServerInfo>,
        }

        #[derive(Debug, Deserialize)]
        struct RawServerInfo {
            #[serde(rename = "i")]
            id: i32,
            #[serde(rename = "d")]
            url: String,
            #[serde(rename = "c")]
            country_code: String,
            #[serde(rename = "md")]
            merged_into: Option<String>,
            #[serde(rename = "m")]
            merge_date_time: Option<String>,
        }

        let resp: ServerResp = serde_json::from_str(&res).map_err(|_| {
            SFError::ParsingError("server response", res.clone())
        })?;

        let servers: HashMap<i32, ServerInfo> = resp
            .servers
            .into_iter()
            .filter_map(|s| {
                let url: Url = format!("https://{}", s.url).parse().ok()?;
                let merged_into = s
                    .merged_into
                    .as_deref()
                    .filter(|u| !u.trim().is_empty())
                    .and_then(|u| format!("https://{u}").parse().ok());
                let merge_date_time =
                    s.merge_date_time.as_deref().and_then(|dt| {
                        NaiveDateTime::parse_from_str(dt, "%Y-%m-%d %H:%M:%S")
                            .ok()
                    });

                let info = ServerInfo {
                    id: s.id,
                    url,
                    country_code: s.country_code,
                    merged_into,
                    merge_date_time,
                };
                Some((s.id, info))
            })
            .collect();
        if servers.is_empty() {
            return Err(SFError::ParsingError("empty server list", res));
        }

        Ok(ServerLookup(servers))
    }

    /// Gets the server info for the given server ID.
    #[must_use]
    pub fn get(&self, server_id: i32) -> Option<&ServerInfo> {
        self.0.get(&server_id)
    }

    /// Looks up a server by its URL.
    #[must_use]
    pub fn get_by_url(&self, url: &Url) -> Option<&ServerInfo> {
        self.0.values().find(|s| {
            s.url == *url
                || (s.url.host_str().is_some()
                    && s.url.host_str() == url.host_str())
        })
    }

    /// Looks up a server ID by its URL.
    #[must_use]
    pub fn get_id(&self, url: &Url) -> Option<i32> {
        self.get_by_url(url).map(|s| s.id)
    }

    /// Returns the number of servers in this lookup table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether this lookup table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Checks if a server with the specified ID exists in this lookup table.
    #[must_use]
    pub fn contains(&self, server_id: i32) -> bool {
        self.0.contains_key(&server_id)
    }

    /// Returns an iterator over all `(&server_id, &ServerInfo)` entries.
    pub fn iter(&self) -> impl Iterator<Item = (&i32, &ServerInfo)> {
        self.0.iter()
    }

    /// Returns an iterator over all [`ServerInfo`] values.
    pub fn servers(&self) -> impl Iterator<Item = &ServerInfo> {
        self.0.values()
    }
}

/// The outcome of an SSO login check attempt.
#[derive(Debug)]
pub enum AuthResponse {
    /// Authentication succeeded, returning the authenticated account.
    Success(SFAccount),
    /// Authentication is still pending or not yet complete.
    NoAuth(SSOAuth),
}

fn val_to_string(val: &Value) -> Option<String> {
    val.as_str().map(std::string::ToString::to_string)
}

/// An in-progress SSO authentication session with a third-party provider.
#[derive(Debug)]
pub struct SSOAuth {
    client: Client,
    options: ConnectionOptions,
    auth_url: Url,
    auth_id: String,
    provider: SSOProvider,
}

/// Supported third-party SSO authentication providers.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum SSOProvider {
    /// Google account authentication.
    Google,
    /// Steam account authentication.
    Steam,
}

impl SSOProvider {
    fn endpoint_ident(self) -> &'static str {
        match self {
            SSOProvider::Google => "googleauth",
            SSOProvider::Steam => "steamauth",
        }
    }
}

impl SSOAuth {
    async fn send_api_request(
        &self,
        endpoint: &str,
        method: APIRequest,
    ) -> Result<Value, SFError> {
        send_api_request(&self.client, "", endpoint, method).await
    }

    /// Returns the SSO auth URL, that the user has to login through
    #[must_use]
    pub fn auth_url(&self) -> &Url {
        &self.auth_url
    }

    /// Tries to login. If the user has successfully authenticated via the
    /// `auth_url`, this will return the normal `SFAccount`. Otherwise, this
    /// will return the existing Auth for you to reattempt the login after a
    /// few seconds
    ///
    /// # Errors
    /// May return `ConnectionError`, or `ParsingError` depending on what part
    /// of the communication failed
    #[allow(clippy::indexing_slicing)]
    pub async fn try_login(self) -> Result<AuthResponse, SFError> {
        let endpoint = format!(
            "/json/sso/{}/check/{}",
            self.provider.endpoint_ident(),
            self.auth_id
        );
        let resp = self.send_api_request(&endpoint, APIRequest::Get).await?;

        if let Some(message) = val_to_string(&resp) {
            return match message.as_str() {
                "SSO_POPUP_STATE_PROCESSING" => Ok(AuthResponse::NoAuth(self)),
                _ => Err(SFError::ConnectionError),
            };
        }

        let id_token =
            val_to_string(&resp["id_token"]).ok_or(SFError::ConnectionError)?;

        let mut form_data = HashMap::new();
        form_data.insert("token".to_string(), id_token.clone());
        form_data.insert("language".to_string(), "en".to_string());

        let res = self
            .send_api_request(
                &format!("json/login/sso/{}", self.provider.endpoint_ident()),
                APIRequest::Post {
                    parameters: vec![
                        "client_id=i43nwwnmfc5tced4jtuk4auuygqghud2yopx",
                        "auth_type=access_token",
                    ],
                    form_data,
                },
            )
            .await?;

        let access_token = val_to_string(&res["token"]["access_token"])
            .ok_or(SFError::ConnectionError)?;
        let uuid = val_to_string(&res["account"]["uuid"])
            .ok_or(SFError::ConnectionError)?;
        let username = val_to_string(&res["account"]["username"])
            .ok_or(SFError::ConnectionError)?;

        Ok(AuthResponse::Success(SFAccount {
            username,
            client: self.client,
            session: AccountSession {
                uuid,
                bearer_token: access_token,
            },
            options: self.options,
            auth: match self.provider {
                SSOProvider::Google => SSOAuthData::Google,
                SSOProvider::Steam => SSOAuthData::Steam,
            },
        }))
    }

    /// Instantiates a new attempt to login through a SSO provider. A user then
    /// has to interact with the `auth_url` this returns to validate the
    /// login. Afterwards you can login and transform this into a normal
    /// `SFAccount`
    ///
    /// # Errors
    /// May return `ConnectionError`, or `ParsingError` depending on what part
    /// of the communication failed
    pub async fn new(provider: SSOProvider) -> Result<Self, SFError> {
        Self::new_with_options(provider, ConnectionOptions::default()).await
    }

    /// The same as `new()`, but with optional connection options
    ///
    /// # Errors
    /// May return `ConnectionError`, or `ParsingError` depending on what part
    /// of the communication failed
    #[allow(clippy::indexing_slicing)]
    pub async fn new_with_options(
        provider: SSOProvider,
        options: ConnectionOptions,
    ) -> Result<Self, SFError> {
        let client =
            reqwest_client(&options).ok_or(SFError::ConnectionError)?;

        let resp = send_api_request(
            &client,
            "",
            &format!("json/sso/{}", provider.endpoint_ident()),
            APIRequest::Get,
        )
        .await?;

        let auth_url = val_to_string(&resp["redirect"])
            .and_then(|a| Url::parse(&a).ok())
            .ok_or(SFError::ConnectionError)?;
        let auth_id =
            val_to_string(&resp["id"]).ok_or(SFError::ConnectionError)?;
        Ok(Self {
            client,
            options,
            auth_url,
            auth_id,
            provider,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info_active_and_merged() {
        let active = ServerInfo {
            id: 1,
            url: Url::parse("https://s1.sfgame.de").unwrap(),
            country_code: "de".to_string(),
            merged_into: None,
            merge_date_time: None,
        };
        assert!(!active.is_merged());

        let merged_past = ServerInfo {
            id: 2,
            url: Url::parse("https://s2.sfgame.de").unwrap(),
            country_code: "de".to_string(),
            merged_into: Some(Url::parse("https://f1.sfgame.net").unwrap()),
            merge_date_time: Some(
                NaiveDateTime::parse_from_str(
                    "2020-01-01 00:00:00",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
            ),
        };
        assert!(merged_past.is_merged());

        let merge_future = ServerInfo {
            id: 3,
            url: Url::parse("https://s3.sfgame.de").unwrap(),
            country_code: "de".to_string(),
            merged_into: Some(Url::parse("https://f1.sfgame.net").unwrap()),
            merge_date_time: Some(
                NaiveDateTime::parse_from_str(
                    "2099-01-01 00:00:00",
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap(),
            ),
        };
        assert!(!merge_future.is_merged());

        let merged_no_date = ServerInfo {
            id: 4,
            url: Url::parse("https://s4.sfgame.de").unwrap(),
            country_code: "de".to_string(),
            merged_into: Some(Url::parse("https://f1.sfgame.net").unwrap()),
            merge_date_time: None,
        };
        assert!(merged_no_date.is_merged());
    }

    #[test]
    fn test_server_lookup_all_and_queries() {
        let mut map = HashMap::new();
        map.insert(
            1,
            ServerInfo {
                id: 1,
                url: Url::parse("https://s1.sfgame.de").unwrap(),
                country_code: "de".to_string(),
                merged_into: None,
                merge_date_time: None,
            },
        );
        map.insert(
            2,
            ServerInfo {
                id: 2,
                url: Url::parse("https://s2.sfgame.de").unwrap(),
                country_code: "de".to_string(),
                merged_into: Some(Url::parse("https://f1.sfgame.net").unwrap()),
                merge_date_time: None,
            },
        );

        let lookup = ServerLookup(map);
        assert_eq!(lookup.len(), 2);
        assert!(!lookup.is_empty());
        assert!(lookup.contains(1));
        assert!(lookup.contains(2));
        assert!(!lookup.contains(999));

        assert_eq!(lookup.get(1).map(|s| s.id), Some(1));
        assert_eq!(
            lookup.get(1).map(|s| s.url.as_str()),
            Some("https://s1.sfgame.de/")
        );
        assert!(lookup.get(999).is_none());

        assert_eq!(lookup.iter().count(), 2);
        assert_eq!(lookup.servers().count(), 2);

        let url1 = Url::parse("https://s1.sfgame.de").unwrap();
        assert_eq!(lookup.get_by_url(&url1).map(|s| s.id), Some(1));
        assert_eq!(lookup.get_id(&url1), Some(1));

        let non_existent = Url::parse("https://unknown.sfgame.net").unwrap();
        assert!(lookup.get_by_url(&non_existent).is_none());
        assert_eq!(lookup.get_id(&non_existent), None);
    }
}
