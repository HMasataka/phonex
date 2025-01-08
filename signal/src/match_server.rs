use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tracing::instrument;

use crate::err::PhonexError;
use crate::r#match::{
    MatchCandidateResponse, MatchRequest, MatchResponse, MatchSessionDescriptionResponse,
};

pub struct Server {
    register_receiver: Cell<Receiver<MatchRequest>>,
    response_channels: Arc<Mutex<HashMap<String, Sender<MatchResponse>>>>,
}

impl Server {
    #[instrument(skip_all, name = "match_server_new", level = "trace")]
    pub fn new(rx: Cell<Receiver<MatchRequest>>) -> Self {
        Self {
            register_receiver: rx,
            response_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[instrument(skip_all, name = "match_server_serve", level = "trace")]
    pub async fn serve(&mut self) {
        loop {
            tokio::select! {
                val = self.register_receiver.get_mut().recv() => {
                    let request = val.unwrap();
                    let _ = self.handle_request(request).await;
                }
            };
        }
    }

    #[instrument(skip_all, name = "match_server_handle_request", level = "trace")]
    pub async fn handle_request(&mut self, request: MatchRequest) -> Result<(), PhonexError> {
        match request {
            MatchRequest::Register(value) => {
                println!("{:?}", value);

                let mut m = self.response_channels.lock().await;
                if m.contains_key(&value.id) {
                    return Err(PhonexError::RegisterNotFound());
                }

                m.insert(value.id, value.chan);
            }
            MatchRequest::SessionDescription(value) => {
                println!("{:?}", value);

                let m = self.response_channels.lock().await;
                if !m.contains_key(&value.target_id) {
                    return Err(PhonexError::RegisterNotFound());
                }

                let chan = m.get(&value.target_id).unwrap();
                chan.send(MatchResponse::SessionDescription(
                    MatchSessionDescriptionResponse {
                        target_id: value.target_id,
                        sdp: value.sdp,
                    },
                ))
                .await
                .map_err(PhonexError::SendMatchResponse)?;
            }
            MatchRequest::Candidate(value) => {
                println!("{:?}", value);

                let m = self.response_channels.lock().await;
                if !m.contains_key(&value.target_id) {
                    return Err(PhonexError::RegisterNotFound());
                }

                let chan = m.get(&value.target_id).unwrap();
                chan.send(MatchResponse::Candidate(MatchCandidateResponse {
                    target_id: value.target_id,
                    candidate: value.candidate,
                }))
                .await
                .map_err(PhonexError::SendMatchResponse)?;
            }
        }

        Ok(())
    }
}
