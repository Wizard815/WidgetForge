//! The `media` source: what is playing, and play/pause/next/previous.
//! Linux uses MPRIS (D-Bus). Windows (SMTC) and macOS are not written yet.

use serde_json::{json, Value};

#[cfg(target_os = "linux")]
pub fn status() -> Value {
    use mpris::{PlaybackStatus, PlayerFinder};
    let Ok(finder) = PlayerFinder::new() else { return json!({ "available": false }) };
    let Ok(player) = finder.find_active() else { return json!({ "available": false }) };
    let meta = player.get_metadata().ok();
    let status = match player.get_playback_status() {
        Ok(PlaybackStatus::Playing) => "playing",
        Ok(PlaybackStatus::Paused) => "paused",
        _ => "stopped",
    };
    let length = meta.as_ref().and_then(|m| m.length()).map(|d| d.as_secs_f64());
    let position = player.get_position().ok().map(|d| d.as_secs_f64());
    json!({
        "available": true,
        "player": player.identity(),
        "status": status,
        "playing": status == "playing",
        "title": meta.as_ref().and_then(|m| m.title().map(String::from)),
        "artist": meta.as_ref().and_then(|m| m.artists().map(|a| a.join(", "))),
        "album": meta.as_ref().and_then(|m| m.album_name().map(String::from)),
        "length": length,
        "position": position,
    })
}

#[cfg(target_os = "linux")]
pub fn control(action: &str) -> Result<(), String> {
    use mpris::PlayerFinder;
    let finder = PlayerFinder::new().map_err(|e| e.to_string())?;
    let player = finder.find_active().map_err(|_| "nothing is playing".to_string())?;
    let r = match action {
        "play_pause" => player.play_pause(),
        "next" => player.next(),
        "previous" => player.previous(),
        "stop" => player.stop(),
        other => return Err(format!("unknown media action `{other}`")),
    };
    r.map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
pub fn status() -> Value {
    json!({ "available": false, "unsupported": true })
}

#[cfg(not(target_os = "linux"))]
pub fn control(_action: &str) -> Result<(), String> {
    Err("media controls are not available on this system yet".into())
}
