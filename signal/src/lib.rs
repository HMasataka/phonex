use errors::PhonexError;
use serde::{Deserialize, Serialize};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

#[derive(Serialize, Deserialize, Debug)]
pub enum RequestType {
    Ping,
    Pong,
    Register,
    SessionDescription,
    Candidate,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub request_type: RequestType,
    pub raw: String,
}

impl Message {
    pub fn new_register_message(id: String) -> Result<Self, PhonexError> {
        let register_message = RegisterMessage { id };
        let r = serde_json::to_string(&register_message).map_err(PhonexError::FromJSONError)?;

        Ok(Self {
            request_type: RequestType::Register,
            raw: r,
        })
    }

    pub fn new_session_description_message(
        target_id: String,
        sdp: RTCSessionDescription,
    ) -> Result<Self, PhonexError> {
        let session_description_message = SessionDescriptionMessage { target_id, sdp };
        let r = serde_json::to_string(&session_description_message)
            .map_err(PhonexError::FromJSONError)?;

        Ok(Self {
            request_type: RequestType::SessionDescription,
            raw: r,
        })
    }

    pub fn new_candidate_message(
        target_id: String,
        candidate: String,
    ) -> Result<Self, PhonexError> {
        let candidate_message = CandidateMessage {
            target_id,
            candidate,
        };
        let r = serde_json::to_string(&candidate_message).map_err(PhonexError::FromJSONError)?;

        Ok(Self {
            request_type: RequestType::Candidate,
            raw: r,
        })
    }

    pub fn try_to_string(&self) -> Result<String, PhonexError> {
        return serde_json::to_string(self).map_err(PhonexError::FromJSONError);
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterMessage {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SessionDescriptionMessage {
    pub target_id: String,
    pub sdp: RTCSessionDescription,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CandidateMessage {
    pub target_id: String,
    pub candidate: String,
}
