use axum::extract::ws::WebSocket;
use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;

pub struct Server {
    sockets: Arc<Mutex<Vec<WebSocket>>>,
    register_receiver: Cell<Receiver<RegisterRequest>>,
}

#[derive(Debug)]
pub struct RegisterRequest {
    pub id: String,
}

impl Server {
    pub fn new(rx: Cell<Receiver<RegisterRequest>>) -> Self {
        Self {
            sockets: Arc::new(Mutex::new(vec![])),
            register_receiver: rx,
        }
    }

    pub async fn serve(&mut self) {
        loop {
            tokio::select! {
                val = self.register_receiver.get_mut().recv() => {
                    println!("register_receiver: {:?}", val);
                }
            };
        }
    }
}
