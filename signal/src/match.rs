use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum MatchRequestType {
    Register,
}

pub struct MatchRequest {
    pub request_type: MatchRequestType,
    pub id: Option<String>,
    pub chan: Option<Sender<MatchResponse>>,
}

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

pub struct MatchResponse {
    pub id: String,
}
