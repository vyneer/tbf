use chrono::prelude::*;
use clap::{Parser, Subcommand};
use colored::*;
use crypto::{digest::Digest, sha1::Sha1};
use env_logger::Env;
use indicatif::{ParallelProgressIterator, ProgressBar};
use lazy_static::lazy_static;
use log::{debug, error, info};
use rayon::prelude::*;
use regex::Regex;
use reqwest::{
    blocking,
    header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT},
    StatusCode,
};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::stdin,
    panic,
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};
use url::Url;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Shows less info
    #[clap(short, long)]
    simple: bool,

    /// Shows more info
    #[clap(short, long)]
    verbose: bool,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Goes over a range of timestamps, looking for a usable/working m3u8 URL
    Bruteforce {
        /// Shows the progress bar if enabled (the progress bar slightly slows down the processing)
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

    /// Combines all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL
    Exact {
        /// Streamer's username (string)
        username: String,

        /// VOD/broadcast ID (integer)
        id: i64,

        /// A timestamp - either an integer (Unix time or whatever the fuck Twitch was using before) or a string (can be like "2020-11-12 20:02:13" or RFC 3339)
        stamp: String,
    },

    /// The same as the Exact mode, but gets all the info from a TwitchTracker URL
    Link {
        /// TwitchTracker URL
        url: String,
    },

    /// Gets the m3u8 from a clip with TwitchTracker's help
    Clip {
        /// TwitchTracker URL
        slug: String,
    },

    /// Goes over a range of timestamps, looking for clips in a VOD
    Clipforce {
        /// Shows the progress bar if enabled (the progress bar slightly slows down the processing)
        #[clap(short, long)]
        progressbar: bool,

        /// VOD/broadcast ID (integer)
        id: i64,

        /// First timestamp (integer)
        start: i64,

        /// Last timestamp (integer)
        end: i64,
    },
}

#[derive(Debug)]
struct TwitchURL {
    full_url: String,
    hash: String,
    timestamp: i64,
}

#[derive(Debug)]
struct AvailabilityCheck {
    fragment: String,
    fragment_muted: String,
    playlist: String,
}

#[derive(Deserialize, Debug)]
pub struct Response {
    data: Data,
}

#[derive(Deserialize, Debug)]
pub struct Data {
    clip: Clip,
}
#[derive(Deserialize, Debug)]
pub struct Clip {
    broadcaster: Broadcaster,
    broadcast: Broadcast,
}

#[derive(Deserialize, Debug)]
pub struct Broadcaster {
    login: String,
}

#[derive(Deserialize, Debug)]
pub struct Broadcast {
    id: String,
}

#[derive(Serialize, Debug)]
pub struct Vars {
    slug: String,
}

#[derive(Serialize, Debug)]
pub struct Query {
    query: String,
    variables: Vars,
}

static CDN_URLS: [&str; 28] = [
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

lazy_static! {
    // HTTP client to share
    static ref HTTP_CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::new();
}

fn info(text: String, simple: bool) {
    if simple {
        println!("{}", text);
    } else {
        info!("{}", text);
    }
}

fn trim_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
}

fn find_bid_from_clip(slug: String) -> (String, i64) {
    let endpoint = "https://gql.twitch.tv/gql";
    let mut headers = HashMap::new();
    headers.insert("Client-ID", "kimne78kx3ncx6brgo4mv6wki5h1ko");

    let mut header_map = HeaderMap::new();

    for (str_key, str_value) in headers {
        let key = HeaderName::from_str(str_key).unwrap();
        let val = HeaderValue::from_str(str_value).unwrap();

        header_map.insert(key, val);
    }

    let query = Query {
        query: "query($slug:ID!){clip(slug: $slug){broadcaster{login}broadcast{id}}}".to_string(),
        variables: Vars { slug },
    };

    let request = HTTP_CLIENT
        .post(endpoint)
        .json(&query)
        .headers(header_map.clone());

    let re = request.send().unwrap();
    let data: Response = re.json().unwrap();
    (
        data.data.clip.broadcaster.login,
        data.data.clip.broadcast.id.parse::<i64>().unwrap(),
    )
}

fn derive_date_from_url(url: &str) -> (String, String, String) {
    if !url.contains("twitchtracker.com") {
        panic!("Only twitchtracker.com URLs are supported");
    }
    let resolved_url = Url::parse(url).unwrap();
    let segments = resolved_url
        .path_segments()
        .map(|c| c.collect::<Vec<_>>())
        .unwrap();
    let username = segments[0];
    let broadcast_id = segments[2];

    let resp = HTTP_CLIENT
        .get(url)
        .header(USER_AGENT, "curl/7.54.0")
        .send()
        .unwrap();
    match resp.status().is_success() {
        true => {}
        false => panic!(
            "The URL provided is unavailable, please check your internet connection - {}: {}",
            resp.status().as_str(),
            resp.status().canonical_reason().unwrap()
        ),
    }

    let body = resp.text().unwrap();
    let fragment = Html::parse_document(&body);
    let selector = Selector::parse(".stream-timestamp-dt.to-dowdatetime").unwrap();

    let date = fragment
        .select(&selector)
        .nth(0)
        .unwrap()
        .text()
        .collect::<String>();
    (username.to_string(), broadcast_id.to_string(), date)
}

fn check_availability(
    hash: &String,
    username: &str,
    broadcast_id: i64,
    timestamp: &i64,
) -> Vec<String> {
    let mut urls: Vec<AvailabilityCheck> = Vec::new();
    let mut valid_urls: Vec<String> = Vec::new();
    for cdn in CDN_URLS {
        urls.push(AvailabilityCheck {
            fragment: (format!(
                "https://{cdn}/{hash}_{username}_{broadcast_id}_{timestamp}/chunked/1.ts",
                cdn = cdn,
                hash = hash,
                username = username,
                broadcast_id = broadcast_id,
                timestamp = timestamp
            )),
            fragment_muted: (format!(
                "https://{cdn}/{hash}_{username}_{broadcast_id}_{timestamp}/chunked/1-muted.ts",
                cdn = cdn,
                hash = hash,
                username = username,
                broadcast_id = broadcast_id,
                timestamp = timestamp
            )),
            playlist: (format!(
                "https://{cdn}/{hash}_{username}_{broadcast_id}_{timestamp}/chunked/index-dvr.m3u8",
                cdn = cdn,
                hash = hash,
                username = username,
                broadcast_id = broadcast_id,
                timestamp = timestamp
            )),
        });
    }
    for url in urls {
        if blocking::get(url.fragment.as_str()).unwrap().status() == 200
            || blocking::get(url.fragment_muted.as_str()).unwrap().status() == 200
        {
            valid_urls.push(url.playlist);
        }
    }
    valid_urls
}

fn parse_timestamp(timestamp: &str) -> i64 {
    let re_unix = Regex::new(r"^\d*$").unwrap();
    let re_utc = Regex::new("UTC").unwrap();

    if re_unix.is_match(timestamp) {
        timestamp.parse::<i64>().unwrap()
    } else {
        if re_utc.is_match(timestamp) {
            NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S UTC")
                .unwrap()
                .timestamp()
        } else {
            let meme = DateTime::parse_from_rfc3339(timestamp);
            match meme {
                Ok(result) => result.timestamp(),
                Err(_) => NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S")
                    .unwrap()
                    .timestamp(),
            }
        }
    }
}

fn bruteforcer(
    username: &str,
    vod: i64,
    initial_from_stamp: &str,
    initial_to_stamp: &str,
    verbose: bool,
    simple: bool,
    pbar: bool,
) {
    let number1 = parse_timestamp(initial_from_stamp);
    let number2 = parse_timestamp(initial_to_stamp);

    let final_url_check = AtomicBool::new(false);
    let mut all_formats_vec: Vec<TwitchURL> = Vec::new();
    if !simple {
        info!("Starting!");
    }
    for number in number1..number2 + 1 {
        let mut hasher = Sha1::new();
        hasher.input_str(format!("{}_{}_{}", username, vod, number).as_str());
        let hex = hasher.result_str();
        for cdn in CDN_URLS {
            all_formats_vec.push(TwitchURL {
                full_url: format!(
                    "https://{cdn}/{hex}_{username}_{vod}_{number}/chunked/index-dvr.m3u8",
                    cdn = cdn,
                    hex = &hex[0..20],
                    username = username,
                    vod = vod,
                    number = number
                ),
                hash: hex[0..20].to_string(),
                timestamp: number,
            });
        }
    }
    debug!("Finished making urls.");
    let pb = ProgressBar::new(all_formats_vec.len() as u64);
    let cloned_pb = pb.clone();
    let iter = all_formats_vec.par_iter();
    let iter_pb = all_formats_vec.par_iter().progress_with(pb);

    let final_url: Vec<_>;
    if pbar {
        final_url = iter_pb.filter_map( |url| {
            if !final_url_check.load(Ordering::SeqCst) {
                let res = HTTP_CLIENT.get(&url.full_url.clone()).send().expect("Error");
                match res.status() {
                    StatusCode::OK => {
                        final_url_check.store(true, Ordering::SeqCst);
                        if verbose {
                            cloned_pb.println(format!("Got it! - {:?}", url));
                        }
                        Some(url)
                    }
                    StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                        if verbose {
                            cloned_pb.println(format!("Still going - {:?}", url));
                        }
                        None
                    }
                    _ => {
                        cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {} - URL: {}", res.status(), res.url()));
                        None
                    }
                }
            } else {
                None
            }
        }).collect();
    } else {
        final_url = iter.filter_map( |url| {
            if !final_url_check.load(Ordering::SeqCst) {
                let res = HTTP_CLIENT.get(&url.full_url.clone()).send().expect("Error");
                match res.status() {
                    StatusCode::OK => {
                        final_url_check.store(true, Ordering::SeqCst);
                        debug!("Got it! - {:?}", url);
                        Some(url)
                    }
                    StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                        debug!("Still going - {:?}", url);
                        None
                    }
                    _ => {
                        info(format!("You might be getting throttled (or your connection is dead)! Status code: {} - URL: {}", res.status(), res.url()), simple);
                        None
                    }
                }
            } else {
                None
            }
        }).collect();
    }

    if !final_url.is_empty() {
        let valid_urls = check_availability(
            &final_url.get(0).unwrap().hash,
            username,
            vod,
            &final_url.get(0).unwrap().timestamp,
        );
        if !valid_urls.is_empty() {
            if !simple {
                info!(
                    "Got the URL and it {} on Twitch servers. Here are the valid URLs:",
                    "was available".green()
                );
            }
            for url in valid_urls {
                info(url, simple);
            }
        } else {
            if !simple {
                info!(
                    "Got the URL and it {} on Twitch servers :(",
                    "was NOT available".red()
                );
                info!(
                    "Here's the URL for debug purposes - {}",
                    final_url.get(0).unwrap().full_url
                );
            }
        }
    } else {
        if !simple {
            info!("{}", "Couldn't find anything :(".red());
        }
    }
}

fn exact(username: &str, vod: i64, initial_stamp: &str, simple: bool) {
    let number = parse_timestamp(initial_stamp);

    let mut hasher = Sha1::new();
    hasher.input_str(format!("{}_{}_{}", username, vod, number).as_str());
    let hex = hasher.result_str();
    let valid_urls = check_availability(&hex[0..20].to_string(), username, vod, &number);
    if !valid_urls.is_empty() {
        if !simple {
            info!(
                "Got the URL and it {} on Twitch servers. Here are the valid URLs:",
                "was available".green()
            );
        }
        for url in valid_urls {
            info(url, simple);
        }
    } else {
        if !simple {
            info!(
                "Got the URL and it {} on Twitch servers :(",
                "was NOT available".red()
            );
            info!("Here's the URL for debug purposes - https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20].to_string(), username, vod, &number);
        }
    }
}

fn clip_bruteforce(vod: String, start: i64, end: i64, verbose: bool, simple: bool, pbar: bool) {
    let pb = ProgressBar::new((end - start) as u64);
    let cloned_pb = pb.clone();

    let iter = (start..end).into_par_iter();
    let iter_pb = (start..end).into_par_iter().progress_with(pb);
    let res: Vec<String>;

    if pbar {
        res = iter_pb.filter_map( |number| {
            let url = format!("https://clips-media-assets2.twitch.tv/AT-cm%7C{}-offset-{}-360.mp4", vod, number);
            let res = HTTP_CLIENT.get(url.as_str()).send().unwrap();
            if res.status() == 200 {
                if verbose {
                    cloned_pb.println(format!("Got a clip! - {}", url));
                }
                Some(url)
            } else if res.status() == 403 {
                if verbose {
                    cloned_pb.println(format!("Still going! - {}", url));
                }
                None
            } else {
                cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {} - URL: {}", res.status(), res.url()));
                None
            }
        }).collect();
    } else {
        res = iter.filter_map( |number| {
            let url = format!("https://clips-media-assets2.twitch.tv/AT-cm%7C{}-offset-{}-360.mp4", vod, number);
            let res = HTTP_CLIENT.get(url.as_str()).send().unwrap();
            if res.status() == 200 {
                if verbose {
                    cloned_pb.println(format!("Got a clip! - {}", url));
                }
                Some(url)
            } else if res.status() == 403 {
                if verbose {
                    cloned_pb.println(format!("Still going! - {}", url));
                }
                None
            } else {
                cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {} - URL: {}", res.status(), res.url()));
                None
            }
        }).collect();
    }

    if !res.is_empty() {
        if !simple {
            info!("{}! Here are the URLs:", "Got some clips".green());
        }
        for line in res {
            info(line, simple);
        }
    } else {
        if !simple {
            info!("{}", "Couldn't find anything :(".red());
        }
    }
}

fn interface() {
    let mut mode = String::new();

    println!("Please select the mode you want:");
    println!("[1] Exact mode - Combines all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL");
    println!("[2] Bruteforce mode - Goes over a range of timestamps, looking for a usable/working m3u8 URL");
    println!("[3] TwitchTracker mode - The same as the Exact mode, but gets all the info from a TwitchTracker URL");
    println!("[4] Clip mode - Gets the m3u8 from a clip with TwitchTracker's help");
    println!(
        "[5] Clip bruteforce mode - Goes over a range of timestamps, looking for clips in a VOD"
    );

    stdin().read_line(&mut mode).expect("Failed to read line.");
    trim_newline(&mut mode);

    match mode.as_str() {
        "1" => {
            let mut username = String::new();
            let mut vod = String::new();
            let mut initial_stamp = String::new();

            println!("Please enter the streamer's username:");
            stdin()
                .read_line(&mut username)
                .expect("Failed to read line.");
            trim_newline(&mut username);
            println!("Please enter the VOD/broadcast ID:");
            stdin().read_line(&mut vod).expect("Failed to read line.");
            trim_newline(&mut vod);
            println!("Please enter the timestamp:");
            stdin()
                .read_line(&mut initial_stamp)
                .expect("Failed to read line.");
            trim_newline(&mut initial_stamp);

            exact(
                username.as_str(),
                vod.parse::<i64>().unwrap(),
                initial_stamp.as_str(),
                false,
            );
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        }
        "2" => {
            let mut username = String::new();
            let mut vod = String::new();
            let mut initial_from_stamp = String::new();
            let mut initial_to_stamp = String::new();

            println!("Please enter the streamer's username:");
            stdin()
                .read_line(&mut username)
                .expect("Failed to read line.");
            trim_newline(&mut username);
            println!("Please enter the VOD/broadcast ID:");
            stdin().read_line(&mut vod).expect("Failed to read line.");
            trim_newline(&mut vod);
            println!("Please enter the first timestamp:");
            stdin()
                .read_line(&mut initial_from_stamp)
                .expect("Failed to read line.");
            trim_newline(&mut initial_from_stamp);
            println!("Please enter the last timestamp:");
            stdin()
                .read_line(&mut initial_to_stamp)
                .expect("Failed to read line.");
            trim_newline(&mut initial_to_stamp);

            bruteforcer(
                username.as_str(),
                vod.parse::<i64>().unwrap(),
                initial_from_stamp.as_str(),
                initial_to_stamp.as_str(),
                false,
                false,
                true,
            );
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        }
        "3" => {
            let mut url = String::new();

            println!("Please enter the TwitchTracker URL:");
            stdin().read_line(&mut url).expect("Failed to read line.");
            trim_newline(&mut url);

            let (username, vod, initial_stamp) = derive_date_from_url(&url);

            exact(
                username.as_str(),
                vod.parse::<i64>().unwrap(),
                initial_stamp.as_str(),
                false,
            );
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        }
        "4" => {
            let mut slug = String::new();

            println!("Please enter the clip's slug (that's the EncouragingTallDragonSpicyBoy or w.e. part):");
            stdin().read_line(&mut slug).expect("Failed to read line.");
            trim_newline(&mut slug);

            let (username, vod) = find_bid_from_clip(slug);
            let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
            let (_, _, initial_stamp) = derive_date_from_url(&url);

            exact(username.as_str(), vod, initial_stamp.as_str(), false);
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        }
        "5" => {
            let mut vod = String::new();
            let mut start = String::new();
            let mut end = String::new();

            println!("Please enter the VOD/broadcast ID:");
            stdin().read_line(&mut vod).expect("Failed to read line.");
            trim_newline(&mut vod);
            println!("Please enter the starting timestamp (in seconds):");
            stdin().read_line(&mut start).expect("Failed to read line.");
            trim_newline(&mut start);
            println!("Please enter the end timestamp (in seconds):");
            stdin().read_line(&mut end).expect("Failed to read line.");
            trim_newline(&mut end);

            let start = start.parse::<i64>().unwrap();
            let end = end.parse::<i64>().unwrap();

            clip_bruteforce(vod, start, end, false, false, true);
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        }
        _ => {}
    }
}

fn main() {
    let matches = Cli::parse();

    let mut log_level = "info";
    if matches.verbose {
        log_level = "debug";
    }

    env_logger::Builder::from_env(
        Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_level),
    )
    .format_timestamp_millis()
    .init();

    // making panics look nicer
    panic::set_hook(Box::new(move |panic_info| {
        debug!("{}", panic_info);
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            error!("{}", s);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            error!("{}", s);
        } else {
            error!("{}", panic_info);
        }
    }));

    match matches.command {
        Some(Commands::Bruteforce {
            progressbar,
            username,
            id,
            from,
            to,
        }) => {
            let username = username.as_str();
            let initial_from_stamp = from.as_str();
            let initial_to_stamp = to.as_str();

            bruteforcer(
                username,
                id,
                initial_from_stamp,
                initial_to_stamp,
                matches.verbose,
                matches.simple,
                progressbar,
            );
        }
        Some(Commands::Exact {
            username,
            id,
            stamp,
        }) => {
            let username = username.as_str();
            let initial_stamp = stamp.as_str();

            exact(username, id, initial_stamp, matches.simple);
        }
        Some(Commands::Link { url }) => {
            let url = url.as_str();
            let (username, vod, initial_stamp) = derive_date_from_url(&url);

            exact(
                &username,
                vod.parse::<i64>().unwrap(),
                &initial_stamp,
                matches.simple,
            );
        }
        Some(Commands::Clip { slug }) => {
            let slug = slug.as_str();
            let (username, vod) = find_bid_from_clip(slug.to_string());
            let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
            let (_, _, initial_stamp) = derive_date_from_url(&url);

            exact(&username, vod, &initial_stamp, matches.simple);
        }
        Some(Commands::Clipforce {
            progressbar,
            id,
            start,
            end,
        }) => {
            clip_bruteforce(
                id.to_string(),
                start,
                end,
                matches.verbose,
                matches.simple,
                progressbar,
            );
        }
        _ => interface(),
    }
}
