use colored::*;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator};
use log::{debug, error, info};
use m3u8_rs::{parse_media_playlist_res, MediaPlaylist, MediaSegment};
use rayon::prelude::*;
use regex::Regex;
use reqwest::StatusCode;
use sha1::{Digest, Sha1};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::Flags;
use crate::twitch::{
    check_availability,
    models::{ReturnURL, TwitchURL},
};
use crate::util::{compile_cdn_list, info, parse_timestamp};

pub fn bruteforcer(
    username: &str,
    vod: i64,
    initial_from_stamp: &str,
    initial_to_stamp: &str,
    flags: Flags,
) -> Option<Vec<ReturnURL>> {
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
                let res = crate::HTTP_CLIENT.get(&url.full_url.clone()).send();
                match res {
                    Ok(res) => {
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
                    },
                    Err(e) => {
                        cloned_pb.println(format!("Reqwest error: {}", e));
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
                let res = crate::HTTP_CLIENT.get(&url.full_url.clone()).send();
                match res {
                    Ok(res) => {
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
                    },
                    Err(e) => {
                        info(format!("Reqwest error: {}", e), flags.simple);
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
            for url in &valid_urls {
                info(url.playlist.clone(), flags.simple);
            }
            Some(valid_urls)
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
            None
        }
    } else {
        if !flags.simple {
            info!("{}", "Couldn't find anything :(".red());
        }
        None
    }
}

pub fn exact(
    username: &str,
    vod: i64,
    initial_stamp: &str,
    flags: Flags,
) -> Option<Vec<ReturnURL>> {
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
        for url in &valid_urls {
            info(url.playlist.clone(), flags.simple);
        }
        Some(valid_urls)
    } else {
        if !flags.simple {
            info!(
                "Got the URL and it {} on Twitch servers :(",
                "was NOT available".red()
            );
            info!("Here's the URL for debug purposes - https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20].to_string(), username, vod, &number);
        }
        None
    }
}

pub fn fix(url: &str, output: Option<String>, old_method: bool, flags: Flags) {
    if !(url.contains("twitch.tv") || url.contains("cloudfront.net")) {
        panic!("Only twitch.tv and cloudfront.net URLs are supported");
    }

    let re = Regex::new(r"[^/]+").unwrap();

    let mut base_url_parts: Vec<String> = Vec::new();
    for elem in re.captures_iter(&url) {
        base_url_parts.push(elem[0].to_string());
    }
    let base_url = format!(
        "https://{}/{}/{}/",
        base_url_parts[1], base_url_parts[2], base_url_parts[3]
    );

    let res = crate::HTTP_CLIENT.get(url).send().unwrap();
    let body = res.text().unwrap();

    let bytes = body.into_bytes();

    let mut playlist = MediaPlaylist {
        ..Default::default()
    };

    match parse_media_playlist_res(&bytes) {
        Ok(pl) => {
            playlist = MediaPlaylist {
                version: pl.version,
                target_duration: pl.target_duration,
                media_sequence: pl.media_sequence,
                discontinuity_sequence: pl.discontinuity_sequence,
                end_list: pl.end_list,
                playlist_type: pl.playlist_type,
                ..Default::default()
            };
            if old_method {
                let mut initial_url_vec: Vec<String> = Vec::new();
                let segments = pl.segments.clone();
                for segment in segments {
                    let url = format!("{}{}", base_url, segment.uri);
                    initial_url_vec.push(url);
                }
                if flags.pbar {
                    let pb = ProgressBar::new(initial_url_vec.len() as u64);
                    let cloned_pb = pb.clone();
                    initial_url_vec
                        .par_iter_mut()
                        .progress_with(pb)
                        .for_each(|url| {
                            let mut remove_chars = 3;
                            let res = crate::HTTP_CLIENT.get(&url.clone()).send().expect("Error");
                            if res.status() == 403 {
                                if url.contains("unmuted") {
                                    remove_chars = 11;
                                }
                                *url = format!(
                                    "{}-muted.ts",
                                    &url.clone()[..url.len() - remove_chars]
                                );
                                if flags.verbose {
                                    cloned_pb.println(format!(
                                        "Found the muted version of this .ts file - {:?}",
                                        url
                                    ))
                                }
                            } else if res.status() == 200 {
                                if flags.verbose {
                                    cloned_pb.println(format!(
                                        "Found the unmuted version of this .ts file - {:?}",
                                        url
                                    ))
                                }
                            }
                        });
                } else {
                    initial_url_vec.par_iter_mut().for_each(|url| {
                        let mut remove_chars = 3;
                        let res = crate::HTTP_CLIENT.get(&url.clone()).send().expect("Error");
                        if res.status() == 403 {
                            if url.contains("unmuted") {
                                remove_chars = 11;
                            }
                            *url = format!("{}-muted.ts", &url.clone()[..url.len() - remove_chars]);
                            debug!("Found the muted version of this .ts file - {:?}", url)
                        } else if res.status() == 200 {
                            debug!("Found the unmuted version of this .ts file - {:?}", url)
                        }
                    });
                }
                let initial_url_vec = &mut initial_url_vec[..];
                alphanumeric_sort::sort_str_slice(initial_url_vec);
                for (i, segment) in pl.segments.iter().enumerate() {
                    playlist.segments.push(MediaSegment {
                        uri: initial_url_vec[i].clone(),
                        duration: segment.duration,
                        ..Default::default()
                    });
                    debug!("Added this .ts file - {:?}", initial_url_vec[i])
                }
            } else {
                if flags.pbar {
                    let pb = ProgressBar::new(pl.segments.len() as u64);
                    let cloned_pb = pb.clone();
                    for segment in pl.segments.iter().progress_with(pb) {
                        let url = format!("{}{}", base_url, segment.uri);
                        if segment.uri.contains("unmuted") {
                            let muted_url = format!("{}-muted.ts", &url.clone()[..url.len() - 11]);
                            playlist.segments.push(MediaSegment {
                                uri: muted_url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            if flags.verbose {
                                cloned_pb.println(format!(
                                    "Found the muted version of this .ts file - {:?}",
                                    url
                                ))
                            }
                        } else {
                            playlist.segments.push(MediaSegment {
                                uri: url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            if flags.verbose {
                                cloned_pb.println(format!(
                                    "Found the unmuted version of this .ts file - {:?}",
                                    url
                                ))
                            }
                        }
                    }
                } else {
                    for segment in pl.segments {
                        let url = format!("{}{}", base_url, segment.uri);
                        if segment.uri.contains("unmuted") {
                            let muted_url = format!("{}-muted.ts", &url.clone()[..url.len() - 11]);
                            playlist.segments.push(MediaSegment {
                                uri: muted_url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            debug!("Found the muted version of this .ts file - {:?}", muted_url)
                        } else {
                            playlist.segments.push(MediaSegment {
                                uri: url.clone(),
                                duration: segment.duration,
                                ..Default::default()
                            });
                            debug!("Found the unmuted version of this .ts file - {:?}", url)
                        }
                    }
                }
            }
        }
        Err(e) => error!("Error in unmute(): {:?}", e),
    }

    let path = match output {
        Some(path) => path,
        None => {
            format!("muted_{}.m3u8", base_url_parts[2])
        }
    };

    let mut file = std::fs::File::create(path).unwrap();
    playlist.write_to(&mut file).unwrap();
}
