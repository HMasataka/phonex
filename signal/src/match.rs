use axum::extract::ws::WebSocket;
use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;

pub struct Server {
    sockets: Arc<Mutex<Vec<Arc<Mutex<WebSocket>>>>>,
    register_receiver: Cell<Receiver<RegisterRequest>>,
    sdp_receiver: Cell<Receiver<SessionDescriptionRequest>>,
}

#[derive(Debug)]
pub struct RegisterRequest {
    pub id: String,
    pub ws: Arc<Mutex<WebSocket>>,
}

#[derive(Debug)]
pub struct SessionDescriptionRequest {
    pub id: String,
}

impl Server {
    pub fn new(
        rx: Cell<Receiver<RegisterRequest>>,
        rx1: Cell<Receiver<SessionDescriptionRequest>>,
    ) -> Self {
        Self {
            sockets: Arc::new(Mutex::new(vec![])),
            register_receiver: rx,
            sdp_receiver: rx1,
        }
    }

    pub async fn serve(&mut self) {
        loop {
            tokio::select! {
                val = self.register_receiver.get_mut().recv() => {
                    let request = val.unwrap();
                    self.sockets.lock().await.push(request.ws);
                }
                val = self.sdp_receiver.get_mut().recv() => {
                    let request = val.unwrap();
                    println!("{:?}",request);
                }
            };
        }
    }
}
