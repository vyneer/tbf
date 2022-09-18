use clap::{Parser, Subcommand, ValueEnum};
use std::{str::FromStr, string::ToString};
use strum::{Display, EnumIter, EnumMessage, EnumString, EnumVariantNames, VariantNames};

pub const CURL_UA: &str = "curl/7.54.0";

#[derive(Clone, Debug, PartialEq, ValueEnum)]
pub enum ProcessingType {
    Exact,
    Bruteforce,
}

#[derive(Parser, Clone, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// Provide minimal output
    #[clap(short, long)]
    pub simple: bool,

    /// Show more info
    #[clap(short, long)]
    pub verbose: bool,

    /// Import more CDN urls via a config file (TXT/JSON/YAML/TOML)
    #[clap(short, long)]
    pub cdnfile: Option<String>,

    /// Enable a progress bar (could slightly slow down the processing)
    #[clap(short, long)]
    pub progressbar: bool,

    /// Select the preferred processing mode for StreamsCharts
    #[clap(short, long, arg_enum)]
    pub mode: Option<ProcessingType>,

    #[clap(subcommand)]
    pub command: Option<Commands>,
}

impl Default for Cli {
    fn default() -> Self {
        Cli {
            simple: false,
            verbose: false,
            cdnfile: None,
            progressbar: false,
            mode: None,
            command: None,
        }
    }
}

#[derive(
    Subcommand, Clone, Debug, EnumMessage, EnumIter, Display, EnumVariantNames, EnumString,
)]
pub enum Commands {
    /// Combine all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL and check whether the VOD is available
    Exact {
        /// Streamer's username (string)
        username: String,

        /// VOD/broadcast ID (integer)
        id: i64,

        /// A timestamp - either an integer (Unix time or whatever the fuck Twitch was using before) or a string (can be like "2020-11-12 20:02:13" or RFC 3339)
        stamp: String,
    },

    /// Go over a range of timestamps, looking for a usable/working m3u8 URL, and check whether the VOD is available
    Bruteforce {
        /// Streamer's username (string)
        username: String,

        /// VOD/broadcast ID (integer)
        id: i64,

        /// First timestamp - either an integer (Unix time or whatever the fuck Twitch was using before) or a string (can be like "2020-11-12 20:02:13" or RFC 3339)
        from: String,

        /// Last timestamp - either an integer (Unix time or whatever the fuck Twitch was using before) or a string (can be like "2020-11-12 20:02:13" or RFC 3339)
        to: String,
    },

    /// Get the m3u8 from a TwitchTracker/StreamsCharts URL
    Link {
        /// TwitchTracker/StreamsCharts URL
        url: String,
    },

    /// Get the m3u8 from a currently running stream
    Live {
        /// Streamer's username (string)
        username: String,
    },

    /// Get the m3u8 from a clip using TwitchTracker
    Clip {
        /// Clip's URL (twitch.tv/%username%/clip/%slug% and clips.twitch.tv/%slug% are both supported) or slug ("GentleAthleticWombatHoneyBadger-ohJAsKzGinIgFUx2" for example)
        clip: String,
    },

    /// Go over a range of timestamps, looking for clips in a VOD
    Clipforce {
        /// VOD/broadcast ID (integer)
        id: i64,

        /// First timestamp (integer)
        start: i64,

        /// Last timestamp (integer)
        end: i64,
    },

    /// Download and convert an unplayable unmuted Twitch VOD playlist into a playable muted one
    Fix {
        /// Twitch VOD m3u8 playlist URL (only twitch.tv and cloudfront.net URLs are supported)
        url: String,

        /// Set the output path (default is current folder)
        #[clap(short, long)]
        output: Option<String>,

        /// Use the old (slow, but more reliable) method of checking for segments
        #[clap(short, long)]
        slow: bool,
    },

    /// Check for updates
    Update,
}

impl Commands {
    pub fn show_description(&self) -> bool {
        match self {
            Self::Update => false,
            _ => true,
        }
    }

    pub fn to_short_desc(&self) -> String {
        match self {
            Self::Exact { .. } => "Exact mode".to_string(),
            Self::Bruteforce { .. } => "Bruteforce mode".to_string(),
            Self::Link { .. } => "Link mode".to_string(),
            Self::Live { .. } => "Live mode".to_string(),
            Self::Clip { .. } => "Clip mode".to_string(),
            Self::Clipforce { .. } => "Clip bruteforce mode".to_string(),
            Self::Fix { .. } => "Fix playlist".to_string(),
            Self::Update => "Check for updates".to_string(),
        }
    }

    pub fn to_selector(&self) -> Option<String> {
        match self {
            Self::Update => Some("u".to_string()),
            _ => None,
        }
    }

    pub fn from_selector(s: String) -> Option<Self> {
        match s.parse::<usize>() {
            Ok(s) => {
                if s > 0 {
                    let s = s - 1;
                    match Self::VARIANTS.get(s) {
                        Some(a) => match Self::from_str(a) {
                            Ok(e) => Some(e),
                            Err(_) => None,
                        },
                        None => None,
                    }
                } else {
                    None
                }
            }
            Err(_) => match s.as_str() {
                "u" => Some(Self::Update),
                _ => None,
            },
        }
    }
}
