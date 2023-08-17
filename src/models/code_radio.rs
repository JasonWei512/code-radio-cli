use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeRadioMessage {
    pub station: Station,
    pub listeners: Listeners,
    pub live: Live,
    pub now_playing: NowPlaying,
    pub playing_next: PlayingNext,
    pub song_history: Vec<SongHistory>,
    pub is_online: bool,
    pub cache: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    pub id: i64,
    pub name: String,
    pub shortcode: String,
    pub description: String,
    pub frontend: String,
    pub backend: String,
    pub listen_url: String,
    pub url: String,
    pub public_player_url: String,
    pub playlist_pls_url: String,
    pub playlist_m3u_url: String,
    pub is_public: bool,
    pub mounts: Vec<Mount>,
    pub remotes: Vec<Remote>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mount {
    pub path: String,
    pub is_default: bool,
    pub id: i64,
    pub name: String,
    pub url: String,
    pub bitrate: i64,
    pub format: String,
    pub listeners: Listeners,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listeners {
    pub total: i64,
    pub unique: i64,
    pub current: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub bitrate: i64,
    pub format: String,
    pub listeners: Listeners,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Live {
    pub is_live: bool,
    pub streamer_name: String,
    pub broadcast_start: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub elapsed: i64,
    pub remaining: i64,
    pub sh_id: i64,
    pub played_at: i64,
    pub duration: i64,
    pub playlist: String,
    pub streamer: String,
    pub is_request: bool,
    pub song: Song,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Song {
    pub id: String,
    pub text: String,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub genre: String,
    pub lyrics: String,
    pub art: String,
    pub custom_fields: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayingNext {
    pub cued_at: i64,
    pub duration: i64,
    pub playlist: String,
    pub is_request: bool,
    pub song: Song,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongHistory {
    pub sh_id: i64,
    pub played_at: i64,
    pub duration: i64,
    pub playlist: String,
    pub streamer: String,
    pub is_request: bool,
    pub song: Song,
}

impl From<Mount> for Remote {
    fn from(mount: Mount) -> Self {
        Self {
            id: mount.id,
            name: mount.name,
            url: mount.url,
            bitrate: mount.bitrate,
            format: mount.format,
            listeners: mount.listeners,
        }
    }
}
