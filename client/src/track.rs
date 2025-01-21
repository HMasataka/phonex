use std::sync::Arc;

use tracing::instrument;
use tracing_spanned::SpanErr;
use webrtc::{
    api::media_engine::MIME_TYPE_H264, rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::err::PhonexError;

#[instrument(skip_all, name = "initialize_channel", level = "trace")]
pub async fn new_video_track() -> Result<Arc<TrackLocalStaticSample>, SpanErr<PhonexError>> {
    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_H264.to_owned(),
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    Ok(video_track)
}
