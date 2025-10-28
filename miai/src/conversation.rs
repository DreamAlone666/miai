//! 小爱对话相关响应体。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use time::{OffsetDateTime, serde::timestamp::milliseconds};

/// 表示小爱对话响应体中 `data` 字段的值。
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    /// 对话记录。
    pub records: Vec<Record>,
}

/// 表示小爱对话的记录。
///
/// 该结构体不反映原始响应体的构造，相反，它从原始响应体中提取出有用的字段。
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    /// 小爱的应答。
    pub answers: Vec<Answer>,

    /// 用户的提问。
    pub query: String,

    /// 请求的 ID。
    pub request_id: String,

    /// 记录的时间。
    #[serde(with = "milliseconds")]
    pub time: OffsetDateTime,
}

/// 表示小爱对话记录的应答。
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// 应答的类型。
    #[serde(rename = "type")]
    pub kind: String,

    // 为了实现 payload 而尝试捕获该值
    bit_set: Option<Vec<u8>>,

    /// 应答的有效数据。
    #[serde(flatten)]
    pub payload: AnswerPayload,
}

/// 表示小爱对话记录应答的有效数据。
#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AnswerPayload {
    /// 类型为 TTS。
    #[non_exhaustive]
    Tts {
        /// 应答的文本。
        text: String,
    },
    /// 类型为 LLM。
    #[non_exhaustive]
    Llm {
        /// 应答的文本。
        text: String,
    },
    /// 未知的类型。
    #[serde(untagged)] // https://github.com/serde-rs/serde/issues/912#issuecomment-1868785603
    Unknown(Map<String, Value>),
}
