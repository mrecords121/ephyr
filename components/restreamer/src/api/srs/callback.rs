//! [HTTP Callback API][1] of [SRS] exposed by application.
//!
//! [SRS]: https://github.com/ossrs/srs
//! [1]: https://github.com/ossrs/srs/wiki/v3_EN_HTTPCallback

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Request {
    pub action: Action,
    pub client_id: u32,
    pub ip: IpAddr,
    pub app: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    OnConnect,
    OnPublish,
    OnUnpublish,
}
