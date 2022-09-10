use clap::{Parser, Subcommand};

pub const CURL_UA: &str = "curl/7.54.0";

#[derive(Debug, Clone)]
pub struct Flags {
    pub verbose: bool,
    pub simple: bool,
    pub pbar: bool,
    pub cdnfile: Option<String>,
    pub bruteforce: Option<bool>,
}

impl Default for Flags {
    fn default() -> Self {
        Flags {
            verbose: false,
            simple: false,
            pbar: false,
            cdnfile: None,
            bruteforce: None,
        }
    }
}

#[derive(Parser)]
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

    #[clap(subcommand)]
    pub command: Option<Commands>,

    /// Explicitly use bruteforce mode for StreamsCharts 
    #[clap(short, long)]
    pub bruteforce: Option<bool>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Go over a range of timestamps, looking for a usable/working m3u8 URL, and check whether the VOD is available
    Bruteforce {
        /// Enable a progress bar (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,

        /// Streamer's username (string)
        username: String,

        /// VOD/broadcast ID (integer)
        id: i64,

        /// First timestamp - either an integer (Unix time or whatever the fuck Twitch was using before) or a string (can be like "2020-11-12 20:02:13" or RFC 3339)
        from: String,

        /// Last timestamp - either an integer (Unix time or whatever the fuck Twitch was using before) or a string (can be like "2020-11-12 20:02:13" or RFC 3339)
        to: String,
    },

    /// Combine all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL and check whether the VOD is available
    Exact {
        /// Enable a progress bar (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,

        /// Streamer's username (string)
        username: String,

        /// VOD/broadcast ID (integer)
        id: i64,

        /// A timestamp - either an integer (Unix time or whatever the fuck Twitch was using before) or a string (can be like "2020-11-12 20:02:13" or RFC 3339)
        stamp: String,
    },

    /// Get the m3u8 from a TwitchTracker/StreamsCharts URL
    Link {
        /// Enable a progress bar (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,

        /// TwitchTracker/StreamsCharts URL
        url: String,
    },

    /// Get the m3u8 from a currently running stream
    Live {
        /// Enable a progress bar (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,

        /// Streamer's username (string)
        username: String,
    },

    /// Get the m3u8 from a clip using TwitchTracker
    Clip {
        /// Enable a progress bar (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,

        /// Clip's URL (twitch.tv/%username%/clip/%slug% and clips.twitch.tv/%slug% are both supported) or slug ("GentleAthleticWombatHoneyBadger-ohJAsKzGinIgFUx2" for example)
        clip: String,
    },

    /// Go over a range of timestamps, looking for clips in a VOD
    Clipforce {
        /// Enable a progress bar (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,

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

        /// Enable a progress bar (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,
    },
}
