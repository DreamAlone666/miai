//! 登录小爱服务。

use std::{collections::HashMap, sync::Arc};

use base64ct::{Base64, Encoding};
use cookie_store::{CookieStore, RawCookie};
use md5::{Digest, Md5};
use reqwest::{Client, Url};
use reqwest_cookie_store::CookieStoreMutex;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use sha1::Sha1;
use tracing::trace;

use crate::util::random_id;

/// 登录小爱服务。
///
/// 更低层级的抽象，可以用来辅助理解小爱服务的登录流程，或对登录进行更精细的控制。使用时需严格遵守先
/// [`login`][Login::login]，再 [`auth`][Login::auth]，最后 [`get_token`][Login::get_token] 的步骤。
#[derive(Clone, Debug)]
pub struct Login {
    client: Client,
    server: Url,
    username: String,
    password_hash: String,
    cookie_store: Arc<CookieStoreMutex>,
}

const LOGIN_SERVER: &str = "https://account.xiaomi.com/pass/";
const LOGIN_UA: &str = "APP/com.xiaomi.mihome APPV/6.0.103 iosPassportSDK/3.9.0 iOS/14.4 miHSTS";

impl Login {
    pub fn new(username: impl Into<String>, password: impl AsRef<[u8]>) -> crate::Result<Self> {
        let server = Url::parse(LOGIN_SERVER)?;

        // 预先添加 Cookies
        let mut cookie_store = CookieStore::new(None);
        let device_id = random_device_id();
        for (name, value) in [("sdkVersion", "3.9"), ("deviceId", &device_id)] {
            let cookie = RawCookie::build((name, value)).path("/").build();
            cookie_store.insert_raw(&cookie, &server)?;
            trace!("预先添加 Cookies: {}", cookie);
        }
        let cookie_store = Arc::new(CookieStoreMutex::new(cookie_store));

        // 用于登录的 Client
        let client = Client::builder()
            .cookie_provider(Arc::clone(&cookie_store))
            .user_agent(LOGIN_UA)
            .build()?;

        Ok(Self {
            client,
            server,
            username: username.into(),
            password_hash: hash_password(password),
            cookie_store,
        })
    }

    /// 初步登录小爱服务。
    ///
    /// 结果中可能会出现登录失败的信息，但这无伤大雅，初步登录只是为了获取一些接下来认证所需的数据。
    pub async fn login(&self) -> crate::Result<LoginResponse> {
        let raw = self.raw_login().await?;

        Ok(serde_json::from_value(raw)?)
    }

    /// 同 [`Login::login`]，但返回原始的 JSON。
    pub async fn raw_login(&self) -> crate::Result<Value> {
        // 初步登录以获取一些认证信息
        let bytes = self
            .client
            .get(self.server.join("serviceLogin?sid=micoapi&_json=true")?)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        // 前 11 个字节不知道是什么，后面追加 json 响应体
        let response = serde_json::from_slice(&bytes[11..])?;
        trace!("尝试初步登录: {response}");

        Ok(response)
    }

    /// 认证小爱服务。
    ///
    /// 需要使用初步登录的结果进行。
    pub async fn auth(&self, login_response: LoginResponse) -> crate::Result<AuthResponse> {
        let raw = self.raw_auth(login_response).await?;

        Ok(serde_json::from_value(raw)?)
    }

    /// 同 [`Login::auth`]，但返回原始的 JSON。
    pub async fn raw_auth(&self, login_response: LoginResponse) -> crate::Result<Value> {
        // 认证
        let form = HashMap::from([
            ("_json", "true"),
            ("qs", &login_response.qs),
            ("sid", &login_response.sid),
            ("_sign", &login_response._sign),
            ("callback", &login_response.callback),
            ("user", &self.username),
            ("hash", &self.password_hash),
        ]);
        let bytes = self
            .client
            .post(self.server.join("serviceLoginAuth2")?)
            .form(&form)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let response = serde_json::from_slice(&bytes[11..])?;
        trace!("尝试认证: {response}");

        Ok(response)
    }

    /// 获取小爱服务的 token，是登录的核心步骤。
    ///
    /// 需要在认证成功后进行。
    pub async fn get_token(&self, auth_response: AuthResponse) -> crate::Result<Value> {
        // 获取 serviceToken，存于 Cookies
        let client_sign = client_sign(&auth_response);
        let url = Url::parse_with_params(&auth_response.location, [("clientSign", &client_sign)])?;
        let response = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        trace!("尝试获取 serviceToken: {response}");

        Ok(response)
    }

    /// 消耗 `Login` 并提取 Cookies，其中存储了当前的登录状态。
    pub fn into_cookie_store(self) -> Arc<CookieStoreMutex> {
        self.cookie_store
    }
}

/// [`Login::login`] 的响应体，但仅包含 [`Login::auth`] 所需的字段。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub qs: String,
    pub sid: String,
    pub _sign: String,
    pub callback: String,
}

/// [`Login::auth`] 的响应体，但仅包含 [`Login::get_token`] 所需的字段。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub location: String,
    pub nonce: Number,
    pub ssecurity: String,
}

fn random_device_id() -> String {
    let mut device_id = random_id(16);
    device_id.make_ascii_uppercase();

    device_id
}

fn hash_password(password: impl AsRef<[u8]>) -> String {
    let result = Md5::new().chain_update(password).finalize();
    let mut result = base16ct::lower::encode_string(&result);
    result.make_ascii_uppercase();

    result
}

fn client_sign(payload: &AuthResponse) -> String {
    let nsec = Sha1::new()
        .chain_update("nonce=")
        .chain_update(payload.nonce.to_string())
        .chain_update("&")
        .chain_update(&payload.ssecurity)
        .finalize();

    Base64::encode_string(&nsec)
}
