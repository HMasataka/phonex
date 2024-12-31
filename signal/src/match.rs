use tokio::sync::mpsc::Sender;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

#[derive(Debug)]
pub enum MatchRequest {
    Register(MatchRegisterRequest),
    SessionDescription(MatchSessionDescriptionRequest),
    Candidate(MatchCandidateRequest),
}

#[derive(Debug)]
pub struct MatchRegisterRequest {
    pub id: String,
    pub chan: Sender<MatchResponse>,
}

#[derive(Debug)]
pub struct MatchSessionDescriptionRequest {
    pub target_id: String,
    pub sdp: RTCSessionDescription,
}

#[derive(Debug)]
pub struct MatchCandidateRequest {
    pub target_id: String,
    pub candidate: String,
}

pub enum MatchResponse {
    SessionDescription(MatchSessionDescriptionResponse),
    Candidate(MatchCandidateResponse),
}

#[derive(Debug)]
pub struct MatchSessionDescriptionResponse {
    pub sdp: RTCSessionDescription,
}

#[derive(Debug)]
pub struct MatchCandidateResponse {
    pub candidate: String,
}
