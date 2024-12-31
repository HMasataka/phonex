use tokio::sync::mpsc::Sender;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

#[derive(Default, Debug)]
pub enum MatchRequestType {
    #[default]
    None,
    Register,
    SessionDescription,
    Candidate,
}

#[derive(Default)]
pub struct MatchRequest {
    pub request_type: MatchRequestType,
    pub id: Option<String>,
    pub chan: Option<Sender<MatchResponse>>,
    pub target_id: Option<String>,
    pub sdp: Option<RTCSessionDescription>,
    pub candidate: Option<String>,
}

impl MatchRequest {
    pub fn new_register_request(id: String, chan: Sender<MatchResponse>) -> Self {
        Self {
            request_type: MatchRequestType::Register,
            id: Some(id),
            chan: Some(chan),
            ..Default::default()
        }
    }

    pub fn new_sdp_request(target_id: String, sdp: RTCSessionDescription) -> Self {
        Self {
            request_type: MatchRequestType::SessionDescription,
            target_id: Some(target_id),
            sdp: Some(sdp),
            ..Default::default()
        }
    }

    pub fn new_candidate_request(target_id: String, candidate: String) -> Self {
        Self {
            request_type: MatchRequestType::Register,
            target_id: Some(target_id),
            candidate: Some(candidate),
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub struct MatchRegisterRequest {
    pub id: String,
    pub chan: Sender<MatchResponse>,
}

impl TryFrom<MatchRequest> for MatchRegisterRequest {
    type Error = ();

    fn try_from(req: MatchRequest) -> Result<Self, Self::Error> {
        if req.id.is_some() && req.chan.is_some() {
            Ok(MatchRegisterRequest {
                id: req.id.unwrap(),
                chan: req.chan.unwrap(),
            })
        } else {
            Err(())
        }
    }
}

#[derive(Debug)]
pub struct MatchSessionDescriptionRequest {
    pub target_id: String,
    pub sdp: RTCSessionDescription,
}

impl TryFrom<MatchRequest> for MatchSessionDescriptionRequest {
    type Error = ();

    fn try_from(req: MatchRequest) -> Result<Self, Self::Error> {
        if req.target_id.is_some() && req.sdp.is_some() {
            Ok(MatchSessionDescriptionRequest {
                target_id: req.target_id.unwrap(),
                sdp: req.sdp.unwrap(),
            })
        } else {
            Err(())
        }
    }
}

#[derive(Debug)]
pub struct MatchCandidateRequest {
    pub target_id: String,
    pub candidate: String,
}

impl TryFrom<MatchRequest> for MatchCandidateRequest {
    type Error = ();

    fn try_from(req: MatchRequest) -> Result<Self, Self::Error> {
        if req.target_id.is_some() && req.candidate.is_some() {
            Ok(MatchCandidateRequest {
                target_id: req.target_id.unwrap(),
                candidate: req.candidate.unwrap(),
            })
        } else {
            Err(())
        }
    }
}

#[derive(Default)]
pub struct MatchResponse {
    pub sdp: Option<RTCSessionDescription>,
    pub candidate: Option<String>,
}

impl MatchResponse {
    pub fn new_sdp_response(sdp: RTCSessionDescription) -> Self {
        Self {
            sdp: Some(sdp),
            ..Default::default()
        }
    }

    pub fn new_candidate_response(candidate: String) -> Self {
        Self {
            candidate: Some(candidate),
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub struct MatchSessionDescriptionResponse {
    pub sdp: RTCSessionDescription,
}

impl TryFrom<MatchResponse> for MatchSessionDescriptionResponse {
    type Error = ();

    fn try_from(req: MatchResponse) -> Result<Self, Self::Error> {
        if let Some(sdp) = req.sdp {
            Ok(MatchSessionDescriptionResponse { sdp })
        } else {
            Err(())
        }
    }
}

#[derive(Debug)]
pub struct MatchCandidateResponse {
    pub candidate: String,
}

impl TryFrom<MatchResponse> for MatchCandidateResponse {
    type Error = ();

    fn try_from(req: MatchResponse) -> Result<Self, Self::Error> {
        if let Some(candidate) = req.candidate {
            Ok(MatchCandidateResponse { candidate })
        } else {
            Err(())
        }
    }
}
