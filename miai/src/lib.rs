//! 调用你的小米、小爱音箱，或其他任何支持的小爱设备。
//!
//! 灵感和实现思路源于 [miservice_fork](https://github.com/yihong0618/MiService)，但主要聚焦于小爱音箱这一设备。
//!
//! # 主要功能
//!
//! - 播报文字。
//! - 播放音乐。
//! - 调整音量。
//! - 控制播放状态。
//! - 执行文本（询问小爱）。
//!
//! # 示例
//!
//! ```no_run
//! use miai::Xiaoai;
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!     // 登录你的账号
//!     let xiaoai = Xiaoai::login("username", "password").await.unwrap();
//!
//!     // 获取你的设备信息
//!     for info in xiaoai.device_info().await.unwrap() {
//!         // 向设备发送请求吧！
//!         xiaoai.tts(&info.device_id, "你好！").await.unwrap();
//!     }
//! }
//! ```

pub mod conversation;
mod error;
pub mod login;
mod util;
mod xiaoai;

pub use ::time;
pub mod json {
    pub use serde_json::*;
}

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

pub use error::*;
pub use xiaoai::*;

/// 小爱服务请求的响应。
#[derive(Clone, Deserialize, Debug)]
pub struct XiaoaiResponse<T = Value> {
    /// 错误码。
    ///
    /// 非 0 的错误码表示当前请求出错了。
    pub code: i64,

    /// 一条简短的消息。
    ///
    /// 常用于定位错误，当请求成功时，用处不大。
    pub message: String,

    /// 返回的实际数据。
    ///
    /// 当请求发生错误时，无法保证返回的数据。
    /// 建议在解析数据前，先使用 [`XiaoaiResponse::error_for_code`] 校验错误码。
    pub data: T,
}

impl XiaoaiResponse {
    /// 校验响应的 `code`，如果不对，此函数将报错。
    ///
    /// # Errors
    ///
    /// `code` 不对时，将返回 [`Error::Api`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use miai::XiaoaiResponse;
    /// fn on_response(res: XiaoaiResponse) {
    ///     match res.error_for_code() {
    ///         Ok(res) => assert_eq!(res.code, 0),
    ///         Err(_err) => ()
    ///     }
    /// }
    /// ```
    pub fn error_for_code(self) -> crate::Result<Self> {
        if self.code == 0 {
            Ok(self)
        } else {
            Err(crate::Error::Api(self))
        }
    }

    /// 提取响应的 `data` 并反序列化。
    ///
    /// # Errors
    ///
    /// 当 `data` 不能反序列化为 `T` 时报错，详见 [`serde_json::from_value`]。
    pub fn extract_data<T: DeserializeOwned>(self) -> crate::Result<T> {
        Ok(serde_json::from_value(self.data)?)
    }
}
