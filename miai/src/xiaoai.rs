mod conversation;

use std::{
    collections::HashMap,
    io::{BufRead, Write},
    sync::Arc,
};

use cookie_store::{
    RawCookie,
    serde::json::{load_all, save_incl_expired_and_nonpersistent},
};
use reqwest::{Client, Url};
use reqwest_cookie_store::CookieStoreMutex;
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use tracing::trace;

use crate::{XiaoaiResponse, login::Login, util::random_id};
use conversation::ConversationData;
pub use conversation::ConversationRecord;

const API_SERVER: &str = "https://api2.mina.mi.com/";
const API_UA: &str = "MiHome/6.0.103 (com.xiaomi.mihome; build:6.0.103.1; iOS 14.4.0) Alamofire/6.0.103 MICO/iOSApp/appStore/6.0.103";

/// 提供小爱服务请求。
///
/// `Xiaoai` 代表着一个账号的登录状态，但如果需要重用的话，也无需再包一层
/// [`std::rc::Rc`] 或 [`Arc`]，`Xiaoai` 已经在内部使用 [`Arc`] 共享状态。
#[derive(Clone, Debug)]
pub struct Xiaoai {
    client: Client,
    cookie_store: Arc<CookieStoreMutex>,
    server: Url,
}

impl Xiaoai {
    /// 登录以调用小爱服务。
    pub async fn login(username: &str, password: &str) -> crate::Result<Self> {
        let login = Login::new(username, password)?;
        let login_response = login.login().await?;
        let auth_response = login.auth(login_response).await?;
        login.get_token(auth_response).await?;

        Self::from_login(login)
    }

    /// 从 [`Login`][`crate::login::Login`] 构造。
    pub fn from_login(login: Login) -> crate::Result<Self> {
        let cookie_store = login.into_cookie_store();
        let client = Client::builder()
            .user_agent(API_UA)
            .cookie_provider(cookie_store.clone())
            .build()?;

        Ok(Self {
            client,
            cookie_store,
            server: Url::parse(API_SERVER)?,
        })
    }

    /// 列出所有设备的信息。
    pub async fn device_info(&self) -> crate::Result<Vec<DeviceInfo>> {
        self.raw_device_info().await?.extract_data()
    }

    /// 同 [`Self::device_info`]，但返回原始的响应。
    pub async fn raw_device_info(&self) -> crate::Result<XiaoaiResponse> {
        let response = self.get("admin/v2/device_list?master=0").await?;
        trace!("获取到设备列表: {}", response.data);

        Ok(response)
    }

    /// 返回内部使用的 [`reqwest::Client`]。
    ///
    /// 该 `Client` 会共享登录状态，可以用来做一些 `Xiaoai` 没有提供的更底层的请求。
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// 小爱服务的通用 GET 请求。
    ///
    /// API 服务器会和 `uri` 做 [`Url::join`]。
    pub async fn get(&self, uri: &str) -> crate::Result<XiaoaiResponse> {
        let request_id = random_request_id();
        let url =
            Url::parse_with_params(self.server.join(uri)?.as_str(), [("requestId", request_id)])?;
        let response = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<XiaoaiResponse>()
            .await?
            .error_for_code()?;

        Ok(response)
    }

    /// 小爱服务的通用 POST 请求。
    ///
    /// 同 [`Self::get`]，但可以带表单数据。
    pub async fn post(
        &self,
        uri: &str,
        mut form: HashMap<&str, &str>,
    ) -> crate::Result<XiaoaiResponse> {
        let request_id = random_request_id();
        form.insert("requestId", &request_id);
        let url = self.server.join(uri)?;
        let response = self
            .client
            .post(url)
            .form(&form)
            .send()
            .await?
            .error_for_status()?
            .json::<XiaoaiResponse>()
            .await?
            .error_for_code()?;

        Ok(response)
    }

    /// 保存登录状态到 `writer`。
    ///
    /// 状态被保存为明文的 json，请注意安全性。参见
    /// [`cookie_store::serde::json::save_incl_expired_and_nonpersistent`]。
    ///
    /// # Panics
    ///
    /// 当内部发生锁中毒时会 panic。
    pub fn save<W: Write>(&self, writer: &mut W) -> cookie_store::Result<()> {
        save_incl_expired_and_nonpersistent(&self.cookie_store.lock().unwrap(), writer)
    }

    /// 从 `reader` 加载登录状态。
    ///
    /// **不会**验证登录状态的有效性，如果在请求时出错，请尝试重新
    /// [`login`][Self::login]。另请参见 [`cookie_store::serde::json::load_all`]。
    pub fn load<R: BufRead>(reader: R) -> cookie_store::Result<Self> {
        let cookie_store = Arc::new(CookieStoreMutex::new(load_all(reader)?));
        let client = Client::builder()
            .user_agent(API_UA)
            .cookie_provider(Arc::clone(&cookie_store))
            .build()?;

        Ok(Self {
            client,
            cookie_store,
            server: Url::parse(API_SERVER)?,
        })
    }

    /// 向小爱设备发送 OpenWrt UBUS 调用请求。
    pub async fn ubus_call(
        &self,
        device_id: &str,
        path: &str,
        method: &str,
        message: &str,
    ) -> crate::Result<XiaoaiResponse> {
        let form = HashMap::from([
            ("deviceId", device_id),
            ("method", method),
            ("path", path),
            ("message", message),
        ]);

        self.post("remote/ubus", form).await
    }

    /// 请求小爱设备播报文本。
    pub async fn tts(&self, device_id: &str, text: &str) -> crate::Result<XiaoaiResponse> {
        let message = json!({"text": text}).to_string();

        self.ubus_call(device_id, "mibrain", "text_to_speech", &message)
            .await
    }

    /// 请求小爱播放 `url`。
    pub async fn play_url(&self, device_id: &str, url: &str) -> crate::Result<XiaoaiResponse> {
        let message = json!({
            "url": url,
            // type 字段不仅能控制亮灯行为，还能控制暂停行为？
            // 比如在机型 L16A 上，设为 3 才能有完整的播放、暂停控制，但无法停止
            // 设为 0、1 可以播放、停止，但暂停后就无法恢复，设为 2 则无法暂停
            // 貌似每个机型都不太一样，参考 https://github.com/yihong0618/MiService/issues/30
            "type": 3,
            "media": "app_ios"
        })
        .to_string();

        self.ubus_call(device_id, "mediaplayer", "player_play_url", &message)
            .await
    }

    /// 请求小爱播放音乐。
    ///
    /// 和 [`Self::play_url`] 相比，此方法针对音频特化，能支持更多参数，但并非所有机型都支持。
    /// 目前尚不支持配置这些参数，仅用作播放音乐的另一种方案。
    pub async fn play_music(&self, device_id: &str, url: &str) -> crate::Result<XiaoaiResponse> {
        const AUDIO_ID: &str = "1582971365183456177";
        const ID: &str = "355454500";
        let message = json!({
            "startaudioid": AUDIO_ID,
            "music": {
                "payload": {
                    // 来自 miservice:
                    // If set to "MUSIC", the light will be on
                    // "audio_type": "MUSIC",
                    "audio_items": [
                        {
                            "item_id": {
                                "audio_id": AUDIO_ID,
                                "cp": {
                                    "album_id": "-1",
                                    "episode_index": 0,
                                    "id": ID,
                                    "name": "xiaowei",
                                },
                            },
                            "stream": {"url": url},
                        }
                    ],
                    "list_params": {
                        "listId": "-1",
                        "loadmore_offset": 0,
                        "origin": "xiaowei",
                        "type": "MUSIC",
                    },
                },
                "play_behavior": "REPLACE_ALL",
            }
        })
        .to_string();

        self.ubus_call(device_id, "mediaplayer", "player_play_music", &message)
            .await
    }

    /// 请求小爱调整音量。
    pub async fn set_volume(&self, device_id: &str, volume: u32) -> crate::Result<XiaoaiResponse> {
        let message = json!({
            "volume": volume,
            "media": "app_ios"
        })
        .to_string();

        self.ubus_call(device_id, "mediaplayer", "player_set_volume", &message)
            .await
    }

    /// 请求小爱执行文本。
    ///
    /// 效果和口头询问一样。
    pub async fn nlp(&self, device_id: &str, text: &str) -> crate::Result<XiaoaiResponse> {
        let message = json!({
            "tts": 1,
            "nlp": 1,
            "nlp_text": text
        })
        .to_string();

        self.ubus_call(device_id, "mibrain", "ai_service", &message)
            .await
    }

    /// 获取播放器的状态信息。
    ///
    /// 可能包含播放状态，音量和循环播放设置。
    pub async fn player_status(&self, device_id: &str) -> crate::Result<XiaoaiResponse> {
        let message = json!({"media": "app_ios"}).to_string();

        self.ubus_call(device_id, "mediaplayer", "player_get_play_status", &message)
            .await
    }

    /// 设置播放器的播放状态。
    pub async fn set_play_state(
        &self,
        device_id: &str,
        state: PlayState,
    ) -> crate::Result<XiaoaiResponse> {
        let action = match state {
            PlayState::Play => "play",
            PlayState::Pause => "pause",
            PlayState::Stop => "stop",
            PlayState::Toggle => "toggle",
        };
        let message = json!({"action": action, "media": "app_ios"}).to_string();

        self.ubus_call(device_id, "mediaplayer", "player_play_operation", &message)
            .await
    }

    /// 获取小爱的对话记录。
    ///
    /// 会获取直到 `until` 前最多 `limit` 条记录，请注意 `device_id` 要和 `hardware` 相匹配。
    ///
    /// # Panics
    ///
    /// 当内部的 Cookies 发生锁中毒时会 panic。
    pub async fn conversations(
        &self,
        device_id: &str,
        hardware: &str,
        until: OffsetDateTime,
        limit: u32,
    ) -> crate::Result<Vec<ConversationRecord>> {
        // 这个响应体的 `data` 是 JSON 字符串，需要通过 String 中转一层
        let data_string: String = self
            .raw_conversations(device_id, hardware, until, limit)
            .await?
            .extract_data()?;
        let data: ConversationData = serde_json::from_str(&data_string)?;
        let mut records = Vec::with_capacity(data.records.len());
        for value in data.records.into_iter() {
            records.push(ConversationRecord::from_value(value)?);
        }

        Ok(records)
    }

    /// 同 [`Self::conversations`]，但返回原始的响应。
    pub async fn raw_conversations(
        &self,
        device_id: &str,
        hardware: &str,
        until: OffsetDateTime,
        limit: u32,
    ) -> crate::Result<XiaoaiResponse> {
        let url = Url::parse_with_params(
            "https://userprofile.mina.mi.com/device_profile/v2/conversation?source=dialogu",
            &[
                ("hardware", hardware),
                ("timestamp", &(until.unix_timestamp() * 1000).to_string()),
                ("limit", &limit.to_string()),
            ],
        )?;

        // 服务端会从 Cookies 中读取 `deviceId`
        let cookie = RawCookie::build(("deviceId", device_id))
            .domain(url.domain().unwrap_or_default())
            .build();
        self.cookie_store
            .lock()
            .unwrap()
            .insert_raw(&cookie, &url)?;

        Ok(self.client.get(url).send().await?.json().await?)
    }
}

/// 表示播放器的播放状态。
#[derive(Clone, Debug)]
pub enum PlayState {
    Play,
    Pause,
    Stop,
    /// 在播放和暂停之间切换。
    Toggle,
}

/// 小爱设备信息。
#[derive(Clone, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// 设备 ID。
    ///
    /// 每个与设备相关的请求都会用 ID 指明对象。
    #[serde(rename = "deviceID")]
    pub device_id: String,

    /// 设备名称。
    pub name: String,

    /// 机型。
    pub hardware: String,
}

fn random_request_id() -> String {
    let mut request_id = random_id(30);
    request_id.insert_str(0, "app_ios_");

    request_id
}
