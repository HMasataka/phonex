use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

pub struct Server {
    receiver: Cell<Receiver<MatchRequest>>,
    sockets: Arc<Mutex<Vec<Sender<MatchResponse>>>>,
}

#[derive(Debug)]
pub struct MatchRequest {
    pub id: String,
    pub chan: Sender<MatchResponse>,
}

pub struct MatchResponse {
    pub id: String,
}

impl Server {
    pub fn new(rx: Cell<Receiver<MatchRequest>>) -> Self {
        Self {
            sockets: Arc::new(Mutex::new(vec![])),
            receiver: rx,
        }
    }

    pub async fn serve(&mut self) {
        loop {
            tokio::select! {
                val = self.receiver.get_mut().recv() => {
                    let request = val.unwrap();
                    self.sockets.lock().await.push(request.chan);
                }
            };
        }
    }
}
