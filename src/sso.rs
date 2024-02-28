use std::{collections::HashMap, sync::Arc};

use chrono::{Local, NaiveDateTime};
use reqwest::{header::*, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use url::Url;

use crate::{
    error::SFError,
    misc::{sha1_hash, HASH_CONST},
    session::{reqwest_client, CharacterSession, ConnectionOptions},
};

#[derive(Debug)]
#[allow(dead_code)]
enum SSOAuth {
    SF { pw_hash: String },
    Google { api_token: String },
}

#[derive(Debug)]
pub struct SFAccount {
    pub(super) username: String,
    auth: SSOAuth,
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
    pub async fn login(
        username: String,
        password: String,
    ) -> Result<SFAccount, SFError> {
        SFAccount::login_with_options(username, password, Default::default())
            .await
    }

    /// Creates a new SSO account by logging the user in.
    pub async fn login_with_options(
        username: String,
        password: String,
        options: ConnectionOptions,
    ) -> Result<SFAccount, SFError> {
        let pw_hash = sha1_hash(&(password.clone() + HASH_CONST));
        let pw_hash = sha1_hash(&(pw_hash + "0"));

        let mut tmp_self = Self {
            username,
            auth: SSOAuth::SF { pw_hash },
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
            SSOAuth::SF { pw_hash } => pw_hash,
            SSOAuth::Google { .. } => {
                // TODO: Try to actually reauth with google (if that is even
                // posible)
                return Err(SFError::ConnectionError);
            }
        };

        let mut form_data = HashMap::new();
        form_data.insert("username".to_string(), self.username.clone());
        form_data.insert("password".to_string(), pw_hash.clone());

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
    ) -> Result<Vec<Result<CharacterSession, SFError>>, SFError> {
        // This could be passed in as an argument in case of multiple SSO
        // accounts to safe on requests, but I dont think people have multiple
        // and this is way easier
        let server_lookup = ServerLookup::fetch(&self.client).await?;
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
                CharacterSession::from_sso_char(
                    char,
                    account.clone(),
                    &server_lookup,
                )
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
    /// Fetches the current mapping of server ids to server urls.
    async fn fetch(client: &Client) -> Result<ServerLookup, SFError> {
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
            .ok_or(SFError::InvalidRequest)
    }
}

#[derive(Debug)]
pub struct GoogleAuth {
    client: Client,
    options: ConnectionOptions,
    auth_url: Url,
    auth_id: String,
}

#[derive(Debug)]
pub enum GoogleAuthResponse {
    Success(SFAccount),
    NoAuth(GoogleAuth),
}

impl GoogleAuth {
    pub fn auth_url(&self) -> &Url {
        &self.auth_url
    }

    pub async fn try_login(self) -> Result<GoogleAuthResponse, SFError> {
        let resp = send_api_request(
            &self.client,
            "",
            &format!("/json/sso/googleauth/check/{}", self.auth_id),
            APIRequest::Get,
        )
        .await?;

        if let Some(message) = val_to_string(&resp) {
            return match message.as_str() {
                "SSO_POPUP_STATE_PROCESSING" => {
                    Ok(GoogleAuthResponse::NoAuth(self))
                }
                _ => Err(SFError::ConnectionError),
            };
        }

        let google_access_token = val_to_string(&resp["access_token"])
            .ok_or(SFError::ConnectionError)?;
        let token_type = val_to_string(&resp["token_type"])
            .ok_or(SFError::ConnectionError)?;
        let id_token =
            val_to_string(&resp["id_token"]).ok_or(SFError::ConnectionError)?;

        if token_type != "Bearer" {
            return Err(SFError::ConnectionError);
        }

        let mut form_data = HashMap::new();
        form_data.insert("token".to_string(), id_token.clone());
        form_data.insert("language".to_string(), "en".to_string());

        let res = send_api_request(
            &self.client,
            "",
            "json/login/sso/googleauth",
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

        Ok(GoogleAuthResponse::Success(SFAccount {
            username,
            client: self.client,
            session: AccountSession {
                uuid,
                bearer_token: access_token,
            },
            options: self.options,
            auth: SSOAuth::Google {
                api_token: google_access_token,
            },
        }))
    }

    pub async fn new() -> Result<GoogleAuth, SFError> {
        GoogleAuth::new_with_options(Default::default()).await
    }

    pub async fn new_with_options(
        options: ConnectionOptions,
    ) -> Result<GoogleAuth, SFError> {
        let client =
            reqwest_client(&options).ok_or(SFError::ConnectionError)?;
        let resp = send_api_request(
            &client,
            "",
            "json/sso/googleauth",
            APIRequest::Get,
        )
        .await?;

        let auth_url = val_to_string(&resp["redirect"])
            .and_then(|a| Url::parse(&a).ok())
            .ok_or(SFError::ConnectionError)?;
        let auth_id =
            val_to_string(&resp["id"]).ok_or(SFError::ConnectionError)?;
        Ok(GoogleAuth {
            client,
            options,
            auth_url,
            auth_id,
        })
    }
}

fn val_to_string(val: &Value) -> Option<String> {
    val.as_str().map(|a| a.to_string())
}
