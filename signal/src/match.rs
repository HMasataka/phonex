use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum MatchRequestType {
    Register,
}

pub struct MatchRequest {
    pub request_type: MatchRequestType,
    pub id: Option<String>,
    pub chan: Option<Sender<MatchResponse>>,
}

struct MatchRegisterRequest {
    id: String,
    chan: Sender<MatchResponse>,
}

impl TryFrom<MatchRequest> for MatchRegisterRequest {
    type Error = ();

    fn try_from(req: MatchRequest) -> Result<Self, Self::Error> {
        if req.id.is_some() && req.chan.is_some() {
            Ok(MatchRegisterRequest {
                id: req.id.unwrap(),
                chan: req.chan.unwrap(),
            })
        } else {
            Err(())
        }
    }
}

pub struct MatchResponse {
    pub id: String,
}

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
