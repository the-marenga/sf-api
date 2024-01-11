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
pub struct SFAccount {
    pub(super) username: String,
    pub(super) pw_hash: String,
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
            pw_hash,
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
        let mut form_data = HashMap::new();
        form_data.insert("username".to_string(), self.username.clone());
        form_data.insert("password".to_string(), self.pw_hash.clone());

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

        let to_str = |d: &Value| d.as_str().map(|a| a.to_string());
        let (Some(bearer_token), Some(uuid)) = (
            to_str(&res["token"]["access_token"]),
            to_str(&res["account"]["uuid"]),
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
        let server_lookup = self.fetch_server_lookup().await?;
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

    /// Send a request to the SSO server. The endoint will be "json/*". We try
    /// to check if the response is bad in any way, but S&F responses never obey
    /// to HTML status codes, or their own system, so good luck
    async fn send_api_request(
        &self,
        endpoint: &str,
        method: APIRequest,
    ) -> Result<Value, SFError> {
        let mut url = url::Url::parse("https://sso.playa-games.com")
            .map_err(|_| SFError::ConnectionError)?;
        url.set_path(endpoint);

        let mut request = match method {
            APIRequest::Get => self.client.get(url.as_str()),
            APIRequest::Post {
                parameters,
                form_data,
            } => {
                url.set_query(Some(&parameters.join("&")));
                self.client.post(url.as_str()).form(&form_data)
            }
        };

        // Set all necessary header values to make our request succeed
        if !self.session.bearer_token.is_empty() {
            request = request.bearer_auth(&self.session.bearer_token);
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
            data: Value,
        }
        let resp: APIResponse = serde_json::from_str(&text)
            .map_err(|_| SFError::ParsingError("API response", text))?;

        if !resp.success {
            return Err(SFError::ConnectionError);
        }
        Ok(resp.data)
    }

    /// Fetches the current mapping of server ids to server urls.
    async fn fetch_server_lookup(&self) -> Result<ServerLookup, SFError> {
        let res = self
            .client
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
}

#[derive(Debug, Clone)]
pub struct ServerLookup(HashMap<i32, Url>);

impl ServerLookup {
    /// Gets the mapping of a server id to a url
    pub fn get(&self, server_id: i32) -> Result<Url, SFError> {
        self.0
            .get(&server_id)
            .cloned()
            .ok_or(SFError::InvalidRequest)
    }
}
