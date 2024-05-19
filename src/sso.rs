use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{Local, NaiveDateTime};
use reqwest::{header::*, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use url::Url;

use crate::{
    error::SFError,
    misc::sha1_hash,
    session::{reqwest_client, ConnectionOptions, PWHash, Session},
};

#[derive(Debug)]
#[allow(dead_code)]
enum SSOAuthData {
    SF { pw_hash: PWHash },
    Google,
    Steam,
}

#[derive(Debug)]
pub struct SFAccount {
    pub(super) username: String,
    auth: SSOAuthData,
    pub(super) session: AccountSession,
    pub(super) client: Client,
    pub(super) options: ConnectionOptions,
}

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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SSOCharacter {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) server_id: i32,
}
impl SFAccount {
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Initializes a SFAccount by logging the user in using the supplied clear
    /// text credentials
    pub async fn login(
        username: String,
        password: String,
    ) -> Result<SFAccount, SFError> {
        Self::login_with_options(username, password, Default::default()).await
    }

    pub async fn login_with_options(
        username: String,
        password: String,
        options: ConnectionOptions,
    ) -> Result<SFAccount, SFError> {
        let pw_hash = PWHash::new(&password);
        Self::login_hashed_with_options(username, pw_hash, options).await
    }

    /// Initializes a SFAccount by logging the user in using the hashed password
    pub async fn login_hashed(
        username: String,
        pw_hash: PWHash,
    ) -> Result<SFAccount, SFError> {
        Self::login_hashed_with_options(username, pw_hash, Default::default())
            .await
    }

    pub async fn login_hashed_with_options(
        username: String,
        pw_hash: PWHash,
        options: ConnectionOptions,
    ) -> Result<SFAccount, SFError> {
        let mut tmp_self = Self {
            username,
            auth: SSOAuthData::SF { pw_hash },
            session: AccountSession {
                uuid: Default::default(),
                bearer_token: Default::default(),
            },
            client: reqwest_client(&options).ok_or(SFError::ConnectionError)?,
            options,
        };

        tmp_self.refresh_login().await?;
        Ok(tmp_self)
    }

    /// Refreshes the session by logging in again with the stored credentials.
    /// This can be used when the server removed our session either for being
    /// connected too long, or the server was restarted/cache cleared and forgot
    /// us
    pub async fn refresh_login(&mut self) -> Result<(), SFError> {
        let pw_hash = match &self.auth {
            SSOAuthData::SF { pw_hash } => pw_hash,
            _ => {
                // I do not think there is a way to reauth without going through
                // the SSO process again for these
                return Err(SFError::InvalidRequest(
                    "Refreshing the SSO-login is only supported for \
                     SSO-Accounts",
                ));
            }
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
    /// consumes the Account, as the character sessions may need to referesh
    /// the accounts session, which they are only allowed to do, if they own it
    /// (in an Arc<Mutex<_>>) and there should be no need to keep the account
    /// around anyways
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
            )
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

/// Send a request to the SSO server. The endoint will be "json/*". We try
/// to check if the response is bad in any way, but S&F responses never obey
/// to HTML status codes, or their own system, so good luck
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

#[derive(Debug, Clone)]
pub struct ServerLookup(HashMap<i32, Url>);

impl ServerLookup {
    pub async fn fetch() -> Result<ServerLookup, SFError> {
        Self::fetch_with_client(&reqwest::Client::new()).await
    }

    /// Fetches the current mapping of server ids to server urls.
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

        #[derive(Debug, Deserialize, Serialize)]
        struct ServerResp {
            servers: Vec<ServerInfo>,
        }

        #[derive(Debug, Deserialize, Serialize)]
        struct ServerInfo {
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
            SFError::ParsingError("server response", res.to_string())
        })?;

        let servers: HashMap<i32, Url> = resp
            .servers
            .into_iter()
            .filter_map(|s| {
                let mut server_url = s.url;
                if let Some(merged_url) = s.merged_into {
                    if let Some(mdt) = s.merge_date_time.and_then(|a| {
                        NaiveDateTime::parse_from_str(&a, "%Y-%m-%d %H:%M:%S")
                            .ok()
                    }) {
                        if Local::now().naive_utc() > mdt {
                            server_url = merged_url
                        }
                    } else {
                        server_url = merged_url
                    }
                }

                Some((s.id, format!("https://{}", server_url).parse().ok()?))
            })
            .collect();
        if servers.is_empty() {
            return Err(SFError::ParsingError("empty server list", res));
        }

        Ok(ServerLookup(servers))
    }

    /// Gets the mapping of a server id to a url
    pub fn get(&self, server_id: i32) -> Result<Url, SFError> {
        self.0
            .get(&server_id)
            .cloned()
            .ok_or(SFError::InvalidRequest("There is no server with this id"))
    }

    pub fn all(&self) -> HashSet<Url> {
        self.0.iter().map(|a| a.1.clone()).collect()
    }
}

#[derive(Debug)]
pub enum AuthResponse {
    Success(SFAccount),
    NoAuth(SSOAuth),
}

fn val_to_string(val: &Value) -> Option<String> {
    val.as_str().map(|a| a.to_string())
}

#[derive(Debug)]
pub struct SSOAuth {
    client: Client,
    options: ConnectionOptions,
    auth_url: Url,
    auth_id: String,
    provider: SSOProvider,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum SSOProvider {
    Google,
    Steam,
}

impl SSOProvider {
    fn endpoint_ident(&self) -> &'static str {
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

    /// Returns the SSO auth url, that the user has to login through
    pub fn auth_url(&self) -> &Url {
        &self.auth_url
    }

    /// Tries to login. If the user has successfully authenticated via the
    /// auth_url, this will return the normal SFAccount. Otherwise, this will
    /// return the existing Auth for you to reattempt the login after a
    /// few seconds
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
    /// has to interact with the auth_url this returns to validate the
    /// login. Afterwards you can login and transform this into a normal
    /// SFAccount
    pub async fn new(provider: SSOProvider) -> Result<Self, SFError> {
        Self::new_with_options(provider, Default::default()).await
    }

    /// The same as `new()`, but with optional connection options
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
