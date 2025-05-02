use lazy_static::lazy_static;
use serde::Serialize;

lazy_static! {
    pub static ref STUN_SERVERS: Vec<String> = vec![
        "stun:stun.l.google.com:19302".to_string(),
        "stun:stun.l.google.com:5349".to_string(),
        "stun:stun1.l.google.com:3478".to_string(),
        "stun:stun1.l.google.com:5349".to_string(),
        "stun:stun2.l.google.com:19302".to_string(),
        "stun:stun2.l.google.com:5349".to_string(),
        "stun:stun3.l.google.com:3478".to_string(),
        "stun:stun3.l.google.com:5349".to_string(),
        "stun:stun4.l.google.com:19302".to_string(),
        "stun:stun4.l.google.com:5349".to_string(),
    ];
}

#[derive(Serialize)]
struct IceServerConfig {
    urls: Vec<String>,
    username: String,
    credential: String,
}

impl IceServerConfig {
    pub fn new(urls: Vec<String>, username: String, credential: String) -> Self {
        Self {
            urls,
            username,
            credential,
        }
    }
}

impl Default for IceServerConfig {
    fn default() -> Self {
        Self::new(STUN_SERVERS.clone(), Default::default(), Default::default())
    }
}
