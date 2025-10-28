use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use webrtc::track::track_remote::TrackRemote;

pub trait VideoTrackHandler: Send + Sync {
    fn handle_video_track<'a>(
        &'a self,
        track: Arc<TrackRemote>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>>;
}

pub trait AudioTrackHandler: Send + Sync {
    fn handle_audio_track<'a>(
        &'a self,
        track: Arc<TrackRemote>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>>;
}

pub trait MediaTrackHandler: Send + Sync {
    fn handle_media_tracks<'a>(
        &'a self,
        video_track: Option<Arc<TrackRemote>>,
        audio_track: Option<Arc<TrackRemote>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>>;
}

// Convenience type aliases for function-based callbacks
pub type VideoTrackCallback = Arc<dyn Fn(Arc<TrackRemote>) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync>;
pub type AudioTrackCallback = Arc<dyn Fn(Arc<TrackRemote>) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync>;
pub type MediaTrackCallback = Arc<dyn Fn(Option<Arc<TrackRemote>>, Option<Arc<TrackRemote>>) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync>;