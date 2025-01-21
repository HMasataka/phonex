use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use signal::{CandidateMessage, SessionDescriptionMessage};

#[derive(Debug)]
pub enum Handshake {
    SessionDescription(SessionDescription),
    Candidate(Candidate),
}

#[derive(Debug)]
pub struct SessionDescription {
    pub target_id: String,
    pub sdp: RTCSessionDescription,
}

impl Into<SessionDescriptionMessage> for SessionDescription {
    fn into(self) -> SessionDescriptionMessage {
        SessionDescriptionMessage {
            target_id: self.target_id,
            sdp: self.sdp,
        }
    }
}

#[derive(Debug)]
pub struct Candidate {
    pub target_id: String,
    pub candidate: String,
}

impl Into<CandidateMessage> for Candidate {
    fn into(self) -> CandidateMessage {
        CandidateMessage {
            target_id: self.target_id,
            candidate: self.candidate,
        }
    }
}
