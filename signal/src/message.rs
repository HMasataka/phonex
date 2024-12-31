use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum RequestType {
    Ping,
    Pong,
    Register,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub request_type: RequestType,
}
