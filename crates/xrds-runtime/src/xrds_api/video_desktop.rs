//! Software video playback, for everywhere that is not a headset.
//!
//! The desktop counterpart to [`super::video_android`]. That path decodes in
//! hardware and never brings a pixel to the CPU; this one decodes with ffmpeg and
//! writes RGBA through [`super::video::write_video_frame_in_world`], which is
//! affordable on a desktop and would not be on an Adreno.
//!
//! Both exist so that *the author does not choose*. A scene naming a video asset in
//! a material slot plays it on either platform, and the difference is a build
//! target rather than something to write two versions of.

use super::*;
use bevy::prelude::Update;
use std::collections::HashMap;
use std::path::PathBuf;

use xrds_media::video::PacedVideo;

/// Clips currently playing, keyed by the id a material slot names.
///
/// `NonSend` because a decoder is not `Sync`, and because there is no reason for
/// this to be touched from more than one thread — the pump is one ordinary system.
#[derive(Default)]
pub(super) struct XrdsVideoPlayers {
    players: HashMap<String, (PacedVideo, bool)>,
}

/// Start playing `path` into the texture named `id`.
///
/// Returns false if the clip cannot be opened, so a caller gets a plain answer
/// rather than a surface that stays blank for reasons it cannot see.
pub(super) fn play_video_in_world(
    world: &mut World,
    id: impl Into<String>,
    path: impl Into<PathBuf>,
    looping: bool,
) -> bool {
    let id = id.into();
    let path = path.into();

    let video = match PacedVideo::open(&path, looping) {
        Ok(video) => video,
        Err(e) => {
            warn!("video '{id}': cannot open {}: {e}", path.display());
            return false;
        }
    };

    // The texture must match the clip, and the clip is the only thing that knows
    // its size — guessing would give the surface the wrong aspect until the first
    // frame lands, and silently the wrong buffer length forever if it were wrong.
    super::video::create_video_texture_in_world(world, id.clone(), video.width(), video.height());

    info!(
        "video '{id}': {}x{} @ {:.1} fps from {}",
        video.width(),
        video.height(),
        video.frame_rate(),
        path.display()
    );
    world
        .non_send_resource_mut::<XrdsVideoPlayers>()
        .players
        .insert(id, (video, looping));
    true
}

/// Whether `id` currently has a decoder running.
pub(super) fn is_playing_in_world(world: &World, id: &str) -> bool {
    world
        .get_non_send_resource::<XrdsVideoPlayers>()
        .is_some_and(|players| players.players.contains_key(id))
}

/// Whether `id` is playing and already set to `looping`.
pub(super) fn is_playing_as_in_world(world: &World, id: &str, looping: bool) -> bool {
    world
        .get_non_send_resource::<XrdsVideoPlayers>()
        .and_then(|players| players.players.get(id).map(|(_, l)| *l == looping))
        .unwrap_or(false)
}

pub(super) fn stop_video_in_world(world: &mut World, id: &str) {
    world
        .non_send_resource_mut::<XrdsVideoPlayers>()
        .players
        .remove(id);
}

/// Move the newest decoded frame of each clip into its texture, once per frame.
pub(super) fn pump_video_players(world: &mut World) {
    // Collected first because writing a frame needs `&mut World`, which cannot be
    // held while the players are borrowed.
    let ready: Vec<(String, Vec<u8>)> = {
        let players = world.non_send_resource::<XrdsVideoPlayers>();
        players
            .players
            .iter()
            .filter_map(|(id, (video, _))| {
                video
                    .newest_frame()
                    .map(|frame| (id.clone(), frame.rgba))
            })
            .collect()
    };

    for (id, rgba) in ready {
        super::video::write_video_frame_in_world(world, &id, &rgba);
    }
}

/// Register the pump. Called from `install_xrds`.
pub(super) fn install(app: &mut App) {
    app.init_non_send_resource::<XrdsVideoPlayers>();
    app.add_systems(Update, pump_video_players);
}
