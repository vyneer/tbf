use anyhow::Result;
use colored::*;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator};
use lazy_static::lazy_static;
use log::{debug, error, info};
use m3u8_rs::{parse_media_playlist_res, MediaPlaylist, MediaSegment};
use rayon::prelude::*;
use regex::Regex;
use reqwest::StatusCode;
use sha1::{Digest, Sha1};

use crate::config::Flags;
use crate::error::PlaylistFixError;
use crate::twitch::{
    check_availability,
    models::{ReturnURL, TwitchURL},
};
use crate::util::{compile_cdn_list, info, parse_timestamp};

lazy_static! {
    static ref FIX_REGEX: Regex = Regex::new(r"[^/]+").unwrap();
}

pub fn bruteforcer(
    username: &str,
    vod: i64,
    initial_from_stamp: &str,
    initial_to_stamp: &str,
    flags: Flags,
) -> Result<Option<Vec<ReturnURL>>> {
    let number1 = match parse_timestamp(initial_from_stamp) {
        Ok(d) => d,
        Err(e) => return Err(e)?,
    };
    let number2 = match parse_timestamp(initial_to_stamp) {
        Ok(d) => d,
        Err(e) => return Err(e)?,
    };

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

    let final_url: Option<&TwitchURL>;
    if flags.pbar {
        final_url = iter_pb.find_any( |url| {
            let res = crate::HTTP_CLIENT.get(&url.full_url.clone()).send();
            match res {
                Ok(res) => {
                    match res.status() {
                        StatusCode::OK => {
                            if flags.verbose {
                                cloned_pb.println(format!("Got it! - {:?}", url));
                            }
                            return true
                        }
                        StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                            if flags.verbose {
                                cloned_pb.println(format!("Still going - {:?}", url));
                            }
                            return false
                        }
                        _ => {
                            cloned_pb.println(format!("You might be getting throttled (or your connection is dead)! Status code: {} - URL: {}", res.status(), res.url()));
                            return false
                        }
                    }
                },
                Err(e) => {
                    cloned_pb.println(format!("Reqwest error: {}", e));
                    return false
                }
            }
        })
    } else {
        final_url = iter.find_any( |url| {
            let res = crate::HTTP_CLIENT.get(&url.full_url.clone()).send();
            match res {
                Ok(res) => {
                    match res.status() {
                        StatusCode::OK => {
                            debug!("Got it! - {:?}", url);
                            return true
                        }
                        StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => {
                            debug!("Still going - {:?}", url);
                            return false
                        }
                        _ => {
                            info(format!("You might be getting throttled (or your connection is dead)! Status code: {} - URL: {}", res.status(), res.url()), flags.simple);
                            return false
                        }
                    }
                },
                Err(e) => {
                    info(format!("Reqwest error: {}", e), flags.simple);
                    return false
                }
            }
        });
    }

    match final_url {
        Some(final_url) => {
            let valid_urls = check_availability(
                &final_url.hash,
                username,
                vod,
                &final_url.timestamp,
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
                Ok(Some(valid_urls))
            } else {
                if !flags.simple {
                    info!(
                        "Got the URL and it {} on Twitch servers :(",
                        "was NOT available".red()
                    );
                    info!("Here's the URL for debug purposes - {}", final_url.full_url);
                }
                Ok(None)
            }
        }
        None => {
            if !flags.simple {
                info!("{}", "Couldn't find anything :(".red());
            }
            Ok(None)
        }
    }
}

pub fn exact(
    username: &str,
    vod: i64,
    initial_stamp: &str,
    flags: Flags,
) -> Result<Option<Vec<ReturnURL>>> {
    let number = match parse_timestamp(initial_stamp) {
        Ok(d) => d,
        Err(e) => return Err(e)?,
    };

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
        Ok(Some(valid_urls))
    } else {
        if !flags.simple {
            info!(
                "Got the URL and it {} on Twitch servers :(",
                "was NOT available".red()
            );
            info!("Here's the URL for debug purposes - https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20].to_string(), username, vod, &number);
        }
        Ok(None)
    }
}

pub fn fix(url: &str, output: Option<String>, old_method: bool, flags: Flags) -> Result<()> {
    if !(url.contains("twitch.tv") || url.contains("cloudfront.net")) {
        error!("Only twitch.tv and cloudfront.net URLs are supported");
        return Err(PlaylistFixError::URLError)?;
    }

    let mut base_url_parts: Vec<String> = Vec::new();
    for elem in FIX_REGEX.captures_iter(&url) {
        base_url_parts.push(elem[0].to_string());
    }
    let base_url = format!(
        "https://{}/{}/{}/",
        base_url_parts[1], base_url_parts[2], base_url_parts[3]
    );

    let res = match crate::HTTP_CLIENT.get(url).send() {
        Ok(r) => r,
        Err(e) => return Err(e)?,
    };
    let body = match res.text() {
        Ok(r) => r,
        Err(e) => return Err(e)?,
    };

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

    let mut file = match std::fs::File::create(path) {
        Ok(e) => e,
        Err(e) => return Err(e)?,
    };
    match playlist.write_to(&mut file) {
        Ok(_) => {}
        Err(e) => return Err(e)?,
    };
    Ok(())
}

pub fn live(username: &str, flags: Flags) -> Result<Option<Vec<ReturnURL>>> {
    match util::find_bid_from_username(username, flags.clone()) {
        Ok(res) => match res {
            Some((bid, stamp)) => exact(username, bid, stamp.as_str(), flags),
            None => Ok(None),
        },
        Err(e) => return Err(e)?,
    }
}

mod util {
    use anyhow::Result;
    use log::error;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::{collections::HashMap, str::FromStr};

    use crate::config::Flags;
    use crate::twitch::models::{VodQuery, VodResponse, VodVars};

    pub fn find_bid_from_username(username: &str, flags: Flags) -> Result<Option<(i64, String)>> {
        let endpoint = "https://gql.twitch.tv/gql";
        let mut headers = HashMap::new();
        headers.insert("Client-ID", "kimne78kx3ncx6brgo4mv6wki5h1ko");

        let mut header_map = HeaderMap::new();

        for (str_key, str_value) in headers {
            let key = match HeaderName::from_str(str_key) {
                Ok(h) => h,
                Err(e) => return Err(e)?,
            };
            let val = match HeaderValue::from_str(str_value) {
                Ok(h) => h,
                Err(e) => return Err(e)?,
            };

            header_map.insert(key, val);
        }

        let query = VodQuery {
            query: "query($login:String){user(login: $login){stream{id createdAt}}}".to_string(),
            variables: VodVars {
                login: username.to_string(),
            },
        };

        let request = crate::HTTP_CLIENT
            .post(endpoint)
            .json(&query)
            .headers(header_map.clone());

        let re = match request.send() {
            Ok(r) => r,
            Err(e) => return Err(e)?,
        };
        let data: VodResponse = match re.json() {
            Ok(d) => d,
            Err(e) => {
                if !flags.simple {
                    error!("Couldn't get the info from the username: {}", e);
                }
                return Ok(None);
            }
        };
        match data.data.user.stream {
            Some(d) => Ok(Some((
                match d.id.parse::<i64>() {
                    Ok(i) => i,
                    Err(e) => return Err(e)?,
                },
                d.created_at,
            ))),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufRead, io::BufReader};

    use tempfile::tempdir;

    use crate::{config::Flags, twitch::models::ReturnURL};

    use super::{bruteforcer, exact as ex, fix};

    #[test]
    fn bruteforce() {
        let bf = bruteforcer(
            &"dansgaming",
            42218705421,
            &"2021-06-05 00:50:16",
            &"2021-06-05 00:50:18",
            Flags::default(),
        )
        .unwrap()
        .unwrap();
        let bf_comp: Vec<ReturnURL> = vec![ReturnURL {
            playlist: "https://d1m7jfoe9zdc1j.cloudfront.net/d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217/chunked/index-dvr.m3u8".to_string(),
            muted: false,
        }, ReturnURL {
            playlist: "https://d2vjef5jvl6bfs.cloudfront.net/d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217/chunked/index-dvr.m3u8".to_string(),
            muted: false,
        }];

        assert_eq!(bf, bf_comp, "testing bruteforce with results");

        let bf_wrong = bruteforcer(
            &"dansgming",
            42218705421,
            &"2021-06-05 00:50:16",
            &"2021-06-05 00:50:18",
            Flags::default(),
        )
        .unwrap();

        assert_eq!(bf_wrong, None, "testing bruteforce with no results");

        let bf_err = bruteforcer(
            &"mrmouton",
            39905263305,
            &"2022-07-12 1200",
            &"2022-07-12 12:00:41",
            Flags::default(),
        );

        assert!(bf_err.is_err(), "testing invalid bruteforce");
    }

    #[test]
    fn exact() {
        let e = ex(
            &"dansgaming",
            42218705421,
            &"2021-06-05 00:50:17",
            Flags::default(),
        )
        .unwrap()
        .unwrap();
        let e_comp: Vec<ReturnURL> = vec![ReturnURL {
            playlist: "https://d1m7jfoe9zdc1j.cloudfront.net/d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217/chunked/index-dvr.m3u8".to_string(),
            muted: false,
        }, ReturnURL {
            playlist: "https://d2vjef5jvl6bfs.cloudfront.net/d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217/chunked/index-dvr.m3u8".to_string(),
            muted: false,
        }];

        assert_eq!(e, e_comp, "testing exact with results");

        let e_wrong = ex(
            &"dansgming",
            42218705421,
            &"2021-06-05 00:50:17",
            Flags::default(),
        )
        .unwrap();

        assert_eq!(e_wrong, None, "testing exact with no results");

        let e_err = ex(
            &"mrmouton",
            39905263305,
            &"2022-07-12 1200",
            Flags::default(),
        );

        assert!(e_err.is_err(), "testing invalid exact");
    }

    #[test]
    fn fix_playlist() {
        let dir = tempdir().unwrap();

        let path = dir.path().join("test.m3u8");

        fix(&"https://d1m7jfoe9zdc1j.cloudfront.net/d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217/chunked/index-dvr.m3u8", Some(path.to_str().unwrap().to_string()), false, Flags::default()).unwrap();

        let r = BufReader::new(File::open(path).unwrap());
        let mut count = 0;

        for _ in r.lines() {
            count = count + 1;
        }

        assert_eq!(count, 2081);
    }
}
