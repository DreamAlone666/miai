use serde::Deserialize;
use serde_json::Value;
use time::{OffsetDateTime, serde::timestamp::milliseconds};

/// 表示小爱对话响应体中 `data` 字段的值。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationData {
    pub records: Vec<Value>,
}

/// 表示小爱对话的记录。
///
/// 该结构体不反映原始响应体的构造，相反，它从原始响应体中提取出有用的字段。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRecord {
    /// 小爱的回答。
    ///
    /// 目前仅解析响应体中 `answers` 中的第一个，大部分情况下也只有一个。
    #[serde(skip)]
    pub answer: String,

    /// 用户的提问。
    pub query: String,

    /// 请求的 ID。
    pub request_id: String,

    /// 记录的时间。
    #[serde(with = "milliseconds")]
    pub time: OffsetDateTime,

    // 这个字段较为复杂，目前需要手动解析为 `answer`，解析后不保证其完整性
    answers: Vec<Value>,
}

impl ConversationRecord {
    /// 从 [`serde_json::Value`] 解析。
    ///
    /// 虽然该类型实现了 [`Deserialize`]，但还是需要从此方法解析，否则 [`Self::answer`] 会永远为空。
    /// 当然，如果此方法解析不到数据，`answer` 也会为空。
    pub fn from_value(value: Value) -> crate::Result<Self> {
        let mut record: Self = serde_json::from_value(value)?;
        if let Some(answer) = record.answers.first_mut()
            && let Some(Value::String(type_)) = answer.get("type") {
                // 解析基于 `type` 字段和内容的键一一对应的关系
                if let Some(payload) = answer.get_mut(type_.to_ascii_lowercase()) {
                    let payload: Payload = serde_json::from_value(payload.take())?;
                    record.answer = payload.text;
                }
            }

        Ok(record)
    }
}

#[derive(Debug, Deserialize)]
struct Payload {
    text: String,
}
