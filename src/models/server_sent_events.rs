use serde::{Deserialize, Serialize};

use super::code_radio::CodeRadioMessage;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeverSentEventsChannelMessage<TData> {
    pub channel: String,
    pub r#pub: Pub<TData>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pub<TData> {
    pub data: TData,
    pub offset: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Np {
    pub np: CodeRadioMessage,
}
