use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use signal::{CandidateMessage, SessionDescriptionMessage};

#[derive(Debug)]
pub enum HandshakeRequest {
    SessionDescriptionRequest(SessionDescriptionRequest),
    CandidateRequest(CandidateRequest),
}

#[derive(Debug)]
pub enum HandshakeResponse {
    SessionDescriptionResponse(SessionDescriptionResponse),
    CandidateResponse(CandidateResponse),
}

#[derive(Debug)]
pub struct SessionDescriptionRequest {
    pub target_id: String,
    pub sdp: RTCSessionDescription,
}

impl Into<SessionDescriptionMessage> for SessionDescriptionRequest {
    fn into(self) -> SessionDescriptionMessage {
        SessionDescriptionMessage {
            target_id: self.target_id,
            sdp: self.sdp,
        }
    }
}

#[derive(Debug)]
pub struct SessionDescriptionResponse {
    pub target_id: String,
    pub sdp: RTCSessionDescription,
}

impl Into<SessionDescriptionMessage> for SessionDescriptionResponse {
    fn into(self) -> SessionDescriptionMessage {
        SessionDescriptionMessage {
            target_id: self.target_id,
            sdp: self.sdp,
        }
    }
}

#[derive(Debug)]
pub struct CandidateRequest {
    pub target_id: String,
    pub candidate: String,
}

impl Into<CandidateMessage> for CandidateRequest {
    fn into(self) -> CandidateMessage {
        CandidateMessage {
            target_id: self.target_id,
            candidate: self.candidate,
        }
    }
}

#[derive(Debug)]
pub struct CandidateResponse {
    pub target_id: String,
    pub candidate: String,
}

impl From<CandidateMessage> for CandidateResponse {
    fn from(m: CandidateMessage) -> Self {
        CandidateResponse {
            target_id: m.target_id,
            candidate: m.candidate,
        }
    }
}
