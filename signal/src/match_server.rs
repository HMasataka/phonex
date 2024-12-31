use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::r#match::{
    MatchCandidateRequest, MatchRegisterRequest, MatchRequest, MatchRequestType, MatchResponse,
    MatchSessionDescriptionRequest,
};

pub struct Server {
    register_receiver: Cell<Receiver<MatchRequest>>,
    response_channels: Arc<Mutex<HashMap<String, Sender<MatchResponse>>>>,
}

impl Server {
    pub fn new(rx: Cell<Receiver<MatchRequest>>) -> Self {
        Self {
            register_receiver: rx,
            response_channels: Arc::new(Mutex::new(HashMap::new())),
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
                    let mut m = self.response_channels.lock().await;
                    if m.contains_key(&v.id) {
                        return;
                    }

                    m.insert(v.id, v.chan);
                }
            }
            MatchRequestType::SessionDescription => {
                let value: Result<MatchSessionDescriptionRequest, ()> = request.try_into();
                if let Ok(v) = value {
                    // TODO send sdp
                    println!("{:?}", v)
                }
            }
            MatchRequestType::Candidate => {
                let value: Result<MatchCandidateRequest, ()> = request.try_into();
                if let Ok(v) = value {
                    // TODO send candidate
                    println!("{:?}", v)
                }
            }
            MatchRequestType::None => {}
        }
    }
}
