use tokio::sync::mpsc::Sender;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::message::{CandidateMessage, SessionDescriptionMessage};

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

impl From<SessionDescriptionMessage> for MatchSessionDescriptionRequest {
    fn from(m: SessionDescriptionMessage) -> Self {
        MatchSessionDescriptionRequest {
            target_id: m.target_id,
            sdp: m.sdp,
        }
    }
}

#[derive(Debug)]
pub struct MatchCandidateRequest {
    pub target_id: String,
    pub candidate: String,
}

impl From<CandidateMessage> for MatchCandidateRequest {
    fn from(m: CandidateMessage) -> Self {
        MatchCandidateRequest {
            target_id: m.target_id,
            candidate: m.candidate,
        }
    }
}

#[derive(Debug)]
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
