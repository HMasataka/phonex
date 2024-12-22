use crate::r#match;

use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct WsChannel {
    pub register_sender: Arc<Mutex<Sender<r#match::RegisterRequest>>>,
    pub sdp_sender: Arc<Mutex<Sender<r#match::SessionDescriptionRequest>>>,
}
