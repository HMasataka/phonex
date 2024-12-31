use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::r#match::{MatchRegisterRequest, MatchRequest, MatchRequestType, MatchResponse};

pub struct Server {
    register_receiver: Cell<Receiver<MatchRequest>>,
    response_channels: Arc<Mutex<Vec<Sender<MatchResponse>>>>,
}

impl Server {
    pub fn new(rx: Cell<Receiver<MatchRequest>>) -> Self {
        Self {
            register_receiver: rx,
            response_channels: Arc::new(Mutex::new(vec![])),
        }
    }

    pub async fn serve(&mut self) {
        loop {
            tokio::select! {
                val = self.register_receiver.get_mut().recv() => {
                    let request = val.unwrap();
                    self.handle_request(request).await;
                }
            };
        }
    }

    pub async fn handle_request(&mut self, request: MatchRequest) {
        match request.request_type {
            MatchRequestType::Register => {
                let value: Result<MatchRegisterRequest, ()> = request.try_into();
                if let Ok(v) = value {
                    self.response_channels.lock().await.push(v.chan);
                }
            }
        }
    }
}
