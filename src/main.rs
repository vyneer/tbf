use log::{info, debug};
use clap::{load_yaml, crate_authors, crate_description, crate_version, App};
use rayon::prelude::*;
use env_logger::Env;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::prelude::*;
use std::io::stdin;
use regex::Regex;
use reqwest::{blocking, header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT}};
use indicatif::{ParallelProgressIterator, ProgressBar};
use scraper::{Html, Selector};
use url::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use lazy_static::lazy_static;

#[derive(Debug)]
struct TwitchURL {
    full_url: String,
    hash: String,
    timestamp: i64,
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

lazy_static! {
    // HTTP client to share
    static ref HTTP_CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::new();
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
        variables: Vars {
            slug
        }
    };

    let request = HTTP_CLIENT
      .post(endpoint)
      .json(&query)
      .headers(header_map.clone());

    let re = request.send().unwrap();
    let data: Response = re.json().unwrap();
    (data.data.clip.broadcaster.login, data.data.clip.broadcast.id.parse::<i64>().unwrap())
}

fn derive_date_from_url(url: &str) -> (String, String, String) {
    if url.contains("twitchtracker.com") { } else { panic!("Only twitchtracker.com URLs are supported") }
    let resolved_url = Url::parse(url).unwrap();
    let segments = resolved_url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap();
    let username = segments[0];
    let broadcast_id = segments[2];

    let resp = HTTP_CLIENT.get(url)
                        .header(USER_AGENT, "curl/7.54.0")
                        .send()
                        .unwrap();
    match resp.status().is_success() {
        true => {},
        false => panic!("The URL provided is unavailable, please check your internet connection"),
    }

    let body = resp.text().unwrap();
    let fragment = Html::parse_document(&body);
    let selector = Selector::parse(".stream-timestamp-dt.to-dowdatetime").unwrap();

    let date = fragment.select(&selector).nth(0).unwrap().text().collect::<String>();
    (username.to_string(), broadcast_id.to_string(), date)
}

fn check_availability(hash: &String, username: &str, broadcast_id: i64, timestamp: &i64) -> Vec<String> {
    let mut urls: Vec<(String, String)> = Vec::new();
    let mut valid_urls: Vec<String> = Vec::new();
    urls.push((format!("https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://vod-metro.twitch.tv/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://vod-metro.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://vod-metro.twitch.tv/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://vod-metro.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://vod-pop-secure.twitch.tv/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://vod-pop-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://vod-pop-secure.twitch.tv/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://vod-pop-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d2e2de1etea730.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://d2e2de1etea730.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d2e2de1etea730.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://d2e2de1etea730.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://dqrpb9wgowsf5.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://dqrpb9wgowsf5.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://dqrpb9wgowsf5.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://dqrpb9wgowsf5.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://ds0h3roq6wcgc.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://ds0h3roq6wcgc.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://ds0h3roq6wcgc.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://ds0h3roq6wcgc.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d2nvs31859zcd8.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://d2nvs31859zcd8.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d2nvs31859zcd8.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://d2nvs31859zcd8.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d2aba1wr3818hz.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://d2aba1wr3818hz.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d2aba1wr3818hz.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://d2aba1wr3818hz.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d3c27h4odz752x.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://d3c27h4odz752x.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d3c27h4odz752x.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://d3c27h4odz752x.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://dgeft87wbj63p.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://dgeft87wbj63p.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://dgeft87wbj63p.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://dgeft87wbj63p.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d1m7jfoe9zdc1j.cloudfront.net/{}_{}_{}_{}/chunked/1.ts", hash, username, broadcast_id, timestamp), format!("https://d1m7jfoe9zdc1j.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    urls.push((format!("https://d1m7jfoe9zdc1j.cloudfront.net/{}_{}_{}_{}/chunked/1-muted.ts", hash, username, broadcast_id, timestamp), format!("https://d1m7jfoe9zdc1j.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", hash, username, broadcast_id, timestamp)));
    for url in urls {
        if blocking::get(url.0.as_str()).unwrap().status() == 200 {
            valid_urls.push(url.1);
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
            NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S UTC").unwrap().timestamp()
        } else {
            let meme = DateTime::parse_from_rfc3339(timestamp);
            match meme {
                Ok(result) => result.timestamp(),
                Err(_) => NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S").unwrap().timestamp(),
            }
        }
    }
}

fn bruteforcer(username: &str, vod: i64, initial_from_stamp: &str, initial_to_stamp: &str, verbose: bool, pbar: bool) {
    let mut log_level = "info";
    if verbose { log_level = "debug" };

    env_logger::init_from_env(
        Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_level));

    let number1 = parse_timestamp(initial_from_stamp);
    let number2 = parse_timestamp(initial_to_stamp);

    let final_url_check = AtomicBool::new(false);
    let mut initial_url_vec_vodsecure: Vec<TwitchURL> = Vec::new();
    let mut initial_url_vec_cloudfront1: Vec<TwitchURL> = Vec::new();
    let mut initial_url_vec_cloudfront2: Vec<TwitchURL> = Vec::new();
    info!("Starting!");
    for number in number1..number2+1 {
        let mut hasher = Sha1::new();
        hasher.input_str(format!("{}_{}_{}", username, vod, number).as_str());
        let hex = hasher.result_str();
        initial_url_vec_vodsecure.push(TwitchURL {
            full_url: format!("https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_vodsecure.push(TwitchURL {
            full_url: format!("https://vod-metro.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_vodsecure.push(TwitchURL {
            full_url: format!("https://vod-pop-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront1.push(TwitchURL {
            full_url: format!("https://d2e2de1etea730.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront2.push(TwitchURL {
            full_url: format!("https://dqrpb9wgowsf5.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront2.push(TwitchURL {
            full_url: format!("https://ds0h3roq6wcgc.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront2.push(TwitchURL {
            full_url: format!("https://d2nvs31859zcd8.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront2.push(TwitchURL {
            full_url: format!("https://d2aba1wr3818hz.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront2.push(TwitchURL {
            full_url: format!("https://d3c27h4odz752x.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront2.push(TwitchURL {
            full_url: format!("https://dgeft87wbj63p.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
        initial_url_vec_cloudfront2.push(TwitchURL {
            full_url: format!("https://d1m7jfoe9zdc1j.cloudfront.net/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], username, vod, number),
            hash: hex[0..20].to_string(),
            timestamp: number
        });
    }
    let all_formats_vec: Vec<Vec<TwitchURL>> = vec![initial_url_vec_vodsecure, initial_url_vec_cloudfront1, initial_url_vec_cloudfront2];
    let all_formats_vec: Vec<TwitchURL> = all_formats_vec.into_iter().flatten().collect();
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
                if res.status() == 200 {
                    final_url_check.store(true, Ordering::SeqCst);
                    if verbose {
                        if pbar {
                            cloned_pb.println(format!("Got it! - {:?}", url));
                        } else {
                            println!("Got it! - {:?}", url);
                        }
                    }
                    Some(url)
                } else if res.status() == 403 {
                    if verbose {
                        if pbar {
                            cloned_pb.println(format!("Still going - {:?}", url));
                        } else {
                            println!("Still going - {:?}", url);
                        }
                    }
                    None
                } else {
                    if pbar {
                        cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {}", res.status()));
                    } else {
                        println!("You might be getting throttled (or your connection is dead)! Status code: {}", res.status());
                    }
                    None
                }
            } else {
                None
            }
        }).collect();
    } else {
        final_url = iter.filter_map( |url| {
            if !final_url_check.load(Ordering::SeqCst) {
                let res = HTTP_CLIENT.get(&url.full_url.clone()).send().expect("Error");
                if res.status() == 200 {
                    final_url_check.store(true, Ordering::SeqCst);
                    if verbose {
                        if pbar {
                            cloned_pb.println(format!("Got it! - {:?}", url));
                        } else {
                            println!("Got it! - {:?}", url);
                        }
                    }
                    Some(url)
                } else if res.status() == 403 {
                    if verbose {
                        if pbar {
                            cloned_pb.println(format!("Still going - {:?}", url));
                        } else {
                            println!("Still going - {:?}", url);
                        }
                    }
                    None
                } else {
                    if pbar {
                        cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {}", res.status()));
                    } else {
                        println!("You might be getting throttled (or your connection is dead)! Status code: {}", res.status());
                    }
                    None
                }
            } else {
                None
            }
        }).collect();
    }
    
    if !final_url.is_empty() {
        let valid_urls = check_availability(&final_url.get(0).unwrap().hash, username, vod, &final_url.get(0).unwrap().timestamp);
        if !valid_urls.is_empty() {
            info!("Got the URL and it was available on Twitch servers. Here are the valid URLs:");
            for url in valid_urls {
                info!("{}", url);
            }
        } else {
            info!("Got the URL and it was NOT available on Twitch servers :(");
            info!("Here's the URL for debug purposes - {}", final_url.get(0).unwrap().full_url);
        }
    } else {
        info!("Couldn't find anything :(");
    }
}

fn exact(username: &str, vod: i64, initial_stamp: &str, verbose: bool) {
    let mut log_level = "info";
    if verbose { log_level = "debug" };

    env_logger::init_from_env(
        Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_level));

    let number = parse_timestamp(initial_stamp);
    
    let mut hasher = Sha1::new();
    hasher.input_str(format!("{}_{}_{}", username, vod, number).as_str());
    let hex = hasher.result_str();
    let valid_urls = check_availability(&hex[0..20].to_string(), username, vod, &number);
    if !valid_urls.is_empty() {
        info!("Got the URL and it was available on Twitch servers. Here are the valid URLs:");
        for url in valid_urls {
            info!("{}", url);
        }
    } else {
        info!("Got the URL and it was NOT available on Twitch servers :(");
        info!("Here's the URL for debug purposes - https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20].to_string(), username, vod, &number);
    }
}

fn clip_bruteforce(vod: String, start: i64, end: i64, verbose: bool, pbar: bool) {
    let mut log_level = "info";
    if verbose { log_level = "debug" };

    env_logger::init_from_env(
        Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_level));

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
                cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {}", res.status()));
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
                cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {}", res.status()));
                None
            }
        }).collect();
    }

    if !res.is_empty() {
        info!("Got some clips! Here are the URLs:");
        for line in res {
            info!("{}", line);
        }
    } else {
        info!("Couldn't find anything :(");
    }
}

fn interface() {
    let mut mode = String::new();

    println!("Please select the mode you want:");
    println!("[1] Exact mode - Combines all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL");
    println!("[2] Bruteforce mode - Goes over a range of timestamps, looking for a usable/working m3u8 URL");
    println!("[3] TwitchTracker mode - The same as the Exact mode, but gets all the info from a TwitchTracker URL");
    println!("[4] Clip mode - Gets the m3u8 from a clip with TwitchTracker's help");
    println!("[5] Clip bruteforce mode - Goes over a range of timestamps, looking for clips in a VOD");

    stdin().read_line(&mut mode).expect("Failed to read line.");
    trim_newline(&mut mode);

    match mode.as_str() {
        "1" => {
            let mut username = String::new();
            let mut vod = String::new();
            let mut initial_stamp = String::new();

            println!("Please enter the streamer's username:");
            stdin().read_line(&mut username).expect("Failed to read line.");
            trim_newline(&mut username);
            println!("Please enter the VOD/broadcast ID:");
            stdin().read_line(&mut vod).expect("Failed to read line.");
            trim_newline(&mut vod);
            println!("Please enter the timestamp:");
            stdin().read_line(&mut initial_stamp).expect("Failed to read line.");
            trim_newline(&mut initial_stamp);

            exact(username.as_str(), vod.parse::<i64>().unwrap(), initial_stamp.as_str(), false);
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        },
        "2" => {
            let mut username = String::new();
            let mut vod = String::new();
            let mut initial_from_stamp = String::new();
            let mut initial_to_stamp = String::new();

            println!("Please enter the streamer's username:");
            stdin().read_line(&mut username).expect("Failed to read line.");
            trim_newline(&mut username);
            println!("Please enter the VOD/broadcast ID:");
            stdin().read_line(&mut vod).expect("Failed to read line.");
            trim_newline(&mut vod);
            println!("Please enter the first timestamp:");
            stdin().read_line(&mut initial_from_stamp).expect("Failed to read line.");
            trim_newline(&mut initial_from_stamp);
            println!("Please enter the last timestamp:");
            stdin().read_line(&mut initial_to_stamp).expect("Failed to read line.");
            trim_newline(&mut initial_to_stamp);

            bruteforcer(username.as_str(), vod.parse::<i64>().unwrap(), initial_from_stamp.as_str(), initial_to_stamp.as_str(), false, true);
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        }
        "3" => {
            let mut url = String::new();

            println!("Please enter the TwitchTracker URL:");
            stdin().read_line(&mut url).expect("Failed to read line.");
            trim_newline(&mut url);

            let (username, vod, initial_stamp) = derive_date_from_url(&url);

            exact(username.as_str(), vod.parse::<i64>().unwrap(), initial_stamp.as_str(), false);
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

            clip_bruteforce(vod, start, end, false, true);
            dont_disappear::any_key_to_continue::custom_msg("Press any key to close...");
        }
        _ => {}
    }
}

fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml)
        .version(crate_version!())
        .about(crate_description!())
        .author(crate_authors!())
        .get_matches();

    match matches.subcommand_name() {
        Some("bruteforce") => {
            if let Some(matches) = matches.subcommand_matches("bruteforce") {
                let username = matches.value_of("username").unwrap();
                let vod = matches.value_of("id").unwrap().parse::<i64>().unwrap();
                let initial_from_stamp = matches.value_of("from").unwrap();
                let initial_to_stamp = matches.value_of("to").unwrap();

                let mut verbose = false;
                if matches.is_present("verbose") {
                    verbose = true;
                }

                let mut pbar = false;
                if matches.is_present("progressbar") {
                    pbar = true;
                }

                bruteforcer(username, vod, initial_from_stamp, initial_to_stamp, verbose, pbar);
            }
        },
        Some("exact") => {
            if let Some(matches) = matches.subcommand_matches("exact") {
                let username = matches.value_of("username").unwrap();
                let vod = matches.value_of("id").unwrap().parse::<i64>().unwrap();
                let initial_stamp = matches.value_of("stamp").unwrap();

                let mut verbose = false;
                if matches.is_present("verbose") {
                    verbose = true;
                }

                exact(username, vod, initial_stamp, verbose);
            }
        },
        Some("link") => {
            if let Some(matches) = matches.subcommand_matches("link") {
                let url = matches.value_of("url").unwrap();

                let mut verbose = false;
                if matches.is_present("verbose") {
                    verbose = true;
                }

                let (username, vod, initial_stamp) = derive_date_from_url(&url);

                exact(&username, vod.parse::<i64>().unwrap(), &initial_stamp, verbose);
            }
        },
        Some("clip") => {
            if let Some(matches) = matches.subcommand_matches("clip") {
                let slug = matches.value_of("slug").unwrap();

                let mut verbose = false;
                if matches.is_present("verbose") {
                    verbose = true;
                }

                let (username, vod) = find_bid_from_clip(slug.to_string());
                let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
                let (_, _, initial_stamp) = derive_date_from_url(&url);

                exact(&username, vod, &initial_stamp, verbose);
            }
        },
        Some("clipforce") => {
            if let Some(matches) = matches.subcommand_matches("clipforce") {
                let vod = matches.value_of("id").unwrap();
                let start = matches.value_of("start").unwrap().parse::<i64>().unwrap();
                let end = matches.value_of("end").unwrap().parse::<i64>().unwrap();

                let mut verbose = false;
                if matches.is_present("verbose") {
                    verbose = true;
                }

                let mut pbar = false;
                if matches.is_present("progressbar") {
                    pbar = true;
                }

                clip_bruteforce(vod.to_string(), start, end, verbose, pbar);
            }
        },
        _ => interface()
    }
}