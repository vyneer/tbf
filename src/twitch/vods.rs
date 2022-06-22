use colored::*;
use indicatif::{ParallelProgressIterator, ProgressBar};
use log::{debug, info};
use rayon::prelude::*;
use reqwest::StatusCode;
use sha1::{Digest, Sha1};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::Flags;
use crate::twitch::{check_availability, models::TwitchURL};
use crate::util::{compile_cdn_list, info, parse_timestamp};

pub fn bruteforcer(
    username: &str,
    vod: i64,
    initial_from_stamp: &str,
    initial_to_stamp: &str,
    flags: Flags,
) {
    let number1 = parse_timestamp(initial_from_stamp);
    let number2 = parse_timestamp(initial_to_stamp);

    let final_url_check = AtomicBool::new(false);
    let mut all_formats_vec: Vec<TwitchURL> = Vec::new();
    if !flags.simple {
        info!("Starting!");
    }
    for number in number1..number2 + 1 {
        let mut hasher = Sha1::new();
        hasher.update(format!("{}_{}_{}", username, vod, number).as_str());
        let hex_vec = hasher.finalize();
        let hex = format!("{:x}", hex_vec);
        let cdn_urls_compiled = compile_cdn_list(flags.cdnfile.clone());
        for cdn in cdn_urls_compiled {
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
    if flags.pbar {
        final_url = iter_pb.filter_map( |url| {
            if !final_url_check.load(Ordering::SeqCst) {
                let res = crate::HTTP_CLIENT.get(&url.full_url.clone()).send().expect("Error");
                match res.status() {
                    StatusCode::OK => {
                        final_url_check.store(true, Ordering::SeqCst);
                        if flags.verbose {
                            cloned_pb.println(format!("Got it! - {:?}", url));
                        }
                        Some(url)
                    }
                    StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                        if flags.verbose {
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
                let res = crate::HTTP_CLIENT.get(&url.full_url.clone()).send().expect("Error");
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
                        info(format!("You might be getting throttled (or your connection is dead)! Status code: {} - URL: {}", res.status(), res.url()), flags.simple);
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
            flags.clone(),
        );
        if !valid_urls.is_empty() {
            if !flags.simple {
                info!(
                    "Got the URL and it {} on Twitch servers. Here are the valid URLs:",
                    "was available".green()
                );
            }
            for url in valid_urls {
                info(url, flags.simple);
            }
        } else {
            if !flags.simple {
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
        if !flags.simple {
            info!("{}", "Couldn't find anything :(".red());
        }
    }
}

pub fn exact(username: &str, vod: i64, initial_stamp: &str, flags: Flags) {
    let number = parse_timestamp(initial_stamp);

    let mut hasher = Sha1::new();
    hasher.update(format!("{}_{}_{}", username, vod, number).as_str());
    let hex_vec = hasher.finalize();
    let hex = format!("{:x}", hex_vec);
    let valid_urls = check_availability(
        &hex[0..20].to_string(),
        username,
        vod,
        &number,
        flags.clone(),
    );
    if !valid_urls.is_empty() {
        if !flags.simple {
            info!(
                "Got the URL and it {} on Twitch servers. Here are the valid URLs:",
                "was available".green()
            );
        }
        for url in valid_urls {
            info(url, flags.simple);
        }
    } else {
        if !flags.simple {
            info!(
                "Got the URL and it {} on Twitch servers :(",
                "was NOT available".red()
            );
            info!("Here's the URL for debug purposes - https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20].to_string(), username, vod, &number);
        }
    }
}
