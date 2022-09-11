use serde::{Deserialize, Serialize};

pub static CDN_URLS: [&str; 28] = [
    "vod-secure.twitch.tv",
    "vod-metro.twitch.tv",
    "vod-pop-secure.twitch.tv",
    "d2e2de1etea730.cloudfront.net",
    "dqrpb9wgowsf5.cloudfront.net",
    "ds0h3roq6wcgc.cloudfront.net",
    "d2nvs31859zcd8.cloudfront.net",
    "d2aba1wr3818hz.cloudfront.net",
    "d3c27h4odz752x.cloudfront.net",
    "dgeft87wbj63p.cloudfront.net",
    "d1m7jfoe9zdc1j.cloudfront.net",
    "d1ymi26ma8va5x.cloudfront.net",
    "d2vjef5jvl6bfs.cloudfront.net",
    "d3vd9lfkzbru3h.cloudfront.net",
    "d1mhjrowxxagfy.cloudfront.net",
    "ddacn6pr5v0tl.cloudfront.net",
    "d3aqoihi2n8ty8.cloudfront.net",
    "d1xhnb4ptk05mw.cloudfront.net",
    "d6tizftlrpuof.cloudfront.net",
    "d36nr0u3xmc4mm.cloudfront.net",
    "d1oca24q5dwo6d.cloudfront.net",
    "d2um2qdswy1tb0.cloudfront.net",
    "d1w2poirtb3as9.cloudfront.net",
    "d6d4ismr40iw.cloudfront.net",
    "d1g1f25tn8m2e6.cloudfront.net",
    "dykkng5hnh52u.cloudfront.net",
    "d2dylwb3shzel1.cloudfront.net",
    "d2xmjdvx03ij56.cloudfront.net",
];

#[derive(Debug)]
pub struct TwitchURL {
    pub full_url: String,
    pub hash: String,
    pub timestamp: i64,
}

#[derive(Debug)]
pub struct AvailabilityCheck {
    pub fragment: String,
    pub fragment_muted: String,
    pub playlist: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnURL {
    pub url: String,
    pub muted: bool,
}

#[derive(Deserialize, Debug)]
pub struct ClipResponse {
    pub data: ClipData,
}

#[derive(Deserialize, Debug)]
pub struct VodResponse {
    pub data: VodData,
}

#[derive(Deserialize, Debug)]
pub struct ClipData {
    pub clip: Clip,
}

#[derive(Deserialize, Debug)]
pub struct VodData {
    pub user: User,
}
#[derive(Deserialize, Debug)]
pub struct Clip {
    pub broadcaster: Broadcaster,
    pub broadcast: Broadcast,
}

#[derive(Deserialize, Debug)]
pub struct User {
    pub stream: Option<Stream>,
}

#[derive(Deserialize, Debug)]
pub struct Stream {
    pub id: String,
    #[serde(alias = "createdAt")]
    pub created_at: String,
}

#[derive(Deserialize, Debug)]
pub struct Broadcaster {
    pub login: String,
}

#[derive(Deserialize, Debug)]
pub struct Broadcast {
    pub id: String,
}

#[derive(Serialize, Debug)]
pub struct ClipVars {
    pub slug: String,
}

#[derive(Serialize, Debug)]
pub struct VodVars {
    pub login: String,
}

#[derive(Serialize, Debug)]
pub struct ClipQuery {
    pub query: String,
    pub variables: ClipVars,
}

#[derive(Serialize, Debug)]
pub struct VodQuery {
    pub query: String,
    pub variables: VodVars,
}
