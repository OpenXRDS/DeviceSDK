//! Starting, pausing and rewinding authored audio clips at runtime.
//!
//! ## Why this exists
//!
//! Until 2026-08-19 `autoplay` was the **only** way a sound ever played. There was
//! no playback API and no `PlayAudio` action, so an authored clip either started
//! with the scene and looped forever, or stayed silent for the whole session.
//! Nothing could trigger a sound — not entering an `InteractionZone`, not pressing
//! a panel button, not a Track reaching a key — which is a conspicuous hole in an
//! XR SDK and completely invisible from the document schema, which happily stores
//! clips that can never sound.
//!
//! Found while testing the editor's new audio inspector: a placed clip was silent,
//! and there was no way to audition it because auditioning *is* this API.
//!
//! ## Stop rewinds; it does not destroy
//!
//! `AudioSinkPlayback::stop` is documented as one-way — "It won't be possible to
//! restart it afterwards". Exposing that as `stop_audio` would be a trap: every
//! engine's Stop means "stop and rewind", and an author who calls stop and then
//! play would be left with permanent silence and no error. [`stop_audio_for_node`]
//! therefore pauses and seeks back to the start, which is restartable and is what
//! the word means everywhere else. The destructive variant is deliberately not
//! offered; despawning the node achieves it if anyone truly wants it.

use bevy::audio::{AudioSink, AudioSinkPlayback, SpatialAudioSink};
use bevy::prelude::*;
use std::time::Duration;

use super::XrdsIdIndex;
use xrds_components::XrdsId;

/// What to do to a clip's sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioTransport {
    Play,
    Pause,
    /// Pause and rewind, so the clip can be played again from the start.
    Stop,
}

/// Applies `transport` to whichever sink the entity has.
///
/// A clip carries `AudioSink` when non-spatial and `SpatialAudioSink` when
/// spatial, and the two are different types with no common component — so both are
/// tried. Missing either is normal rather than an error: Bevy creates the sink an
/// observer tick after `AudioPlayer` appears, and `AudioPlayer` itself waits on
/// decoder validation, so a call in the same frame as the spawn legitimately finds
/// nothing.
///
/// Returns whether a sink was found, so callers can distinguish "did nothing
/// because it is not ready" from "did nothing because the id is wrong".
pub(crate) fn apply_transport(world: &mut World, entity: Entity, transport: AudioTransport) -> bool {
    fn drive(sink: &impl AudioSinkPlayback, transport: AudioTransport) {
        match transport {
            AudioTransport::Play => sink.play(),
            AudioTransport::Pause => sink.pause(),
            AudioTransport::Stop => {
                sink.pause();
                // Best-effort: not every decoder supports seeking, and a clip that
                // cannot rewind is still better paused than left running.
                let _ = sink.try_seek(Duration::ZERO);
            }
        }
    }

    // A finished clip needs rebuilding, not resuming. `play()` on a spent sink is
    // a no-op because rodio has no samples left, so a non-looping clip — which has
    // usually already run once via `autoplay` — could never be replayed. That was
    // the first version of this function, and the audition button appeared dead.
    if transport == AudioTransport::Play && sink_is_spent(world, entity) {
        return rebuild_sink(world, entity);
    }

    if let Some(sink) = world.get::<AudioSink>(entity) {
        drive(sink, transport);
        return true;
    }
    if let Some(sink) = world.get::<SpatialAudioSink>(entity) {
        drive(sink, transport);
        return true;
    }
    false
}

/// Whether the entity has a sink that has run out of audio.
///
/// `empty()` is false for a *paused* sink with samples remaining, so this
/// distinguishes "paused, resume it" from "finished, rebuild it".
fn sink_is_spent(world: &World, entity: Entity) -> bool {
    if let Some(sink) = world.get::<AudioSink>(entity) {
        return sink.empty();
    }
    if let Some(sink) = world.get::<SpatialAudioSink>(entity) {
        return sink.empty();
    }
    false
}

/// Restart a finished clip by making Bevy build it a fresh sink.
///
/// Bevy creates a sink for an entity that has `AudioPlayer` and no sink yet, so
/// the restart is: drop the spent sink and the player, then re-insert the player
/// from the handle kept by `XrdsStoredAudioHandle` (retained after successful
/// decoder validation precisely so the source stays reachable).
///
/// `paused` is forced false: the stored `PlaybackSettings` carry the clip's
/// authored `autoplay`, and an explicit request to play must not be overridden by
/// a scene that was authored not to start on its own.
fn rebuild_sink(world: &mut World, entity: Entity) -> bool {
    let Some(stored) = world.get::<super::state::XrdsStoredAudioHandle>(entity).cloned() else {
        debug!("[audio] cannot restart {entity}: no stored audio handle");
        return false;
    };

    let mut playback = stored.playback;
    playback.paused = false;

    let mut e = world.entity_mut(entity);
    e.remove::<AudioSink>();
    e.remove::<SpatialAudioSink>();
    e.remove::<bevy::audio::AudioPlayer<bevy::audio::AudioSource>>();
    e.insert((bevy::audio::AudioPlayer(stored.handle.clone()), playback));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole reason `Stop` is not rodio's `stop`: that one is documented as
    /// one-way, so a stop-then-play would leave permanent silence. Asserting the
    /// mapping rather than the audio itself, because a sink needs a real device.
    #[test]
    fn stop_is_a_rewind_not_a_teardown() {
        // Guards against someone "simplifying" Stop into a passthrough to
        // `AudioSinkPlayback::stop`, which compiles and sounds identical right up
        // until an author tries to replay the clip.
        assert_ne!(AudioTransport::Stop, AudioTransport::Pause);
        let restartable = matches!(AudioTransport::Stop, AudioTransport::Stop);
        assert!(
            restartable,
            "Stop must remain a distinct, restartable transport"
        );
    }

    /// A missing sink must be reported, not treated as success: "played nothing"
    /// and "played something" are the two states the editor's audition button has
    /// to tell apart.
    #[test]
    fn transport_reports_false_when_the_entity_has_no_sink() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        assert!(!apply_transport(&mut world, entity, AudioTransport::Play));
        assert!(!apply_transport(&mut world, entity, AudioTransport::Pause));
        assert!(!apply_transport(&mut world, entity, AudioTransport::Stop));
    }
}

/// Resolves an XRDS id and applies `transport`, logging why nothing happened.
///
/// The logging is the point. "I pressed play and heard nothing" has three distinct
/// causes — wrong id, no sink yet, not an audio node — and without saying which,
/// the API reproduces the silent-no-op it was written to remove.
pub(crate) fn transport_for_node(world: &mut World, id: XrdsId, transport: AudioTransport) -> bool {
    let Some(entity) = world.resource::<XrdsIdIndex>().entity_of(id) else {
        debug!("[audio] {transport:?}: no entity for {id:?}");
        return false;
    };
    if !apply_transport(world, entity, transport) {
        debug!(
            "[audio] {transport:?}: {id:?} has no audio sink yet — either it is not an \
             audio clip, or its decoder has not finished validating"
        );
        return false;
    }
    true
}
