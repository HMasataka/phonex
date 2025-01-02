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
