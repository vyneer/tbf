pub mod clips;
pub mod models;
pub mod vods;

use indicatif::{ParallelProgressIterator, ProgressBar};
use rayon::prelude::*;

use crate::config::Flags;
use crate::util::compile_cdn_list;
use models::{AvailabilityCheck, ReturnURL};

pub fn check_availability(
    hash: &String,
    username: &str,
    broadcast_id: i64,
    timestamp: &i64,
    flags: Flags,
) -> Vec<ReturnURL> {
    let mut urls: Vec<AvailabilityCheck> = Vec::new();
    let valid_urls: Vec<ReturnURL>;
    let cdn_urls_compiled = compile_cdn_list(flags.cdnfile);
    for cdn in cdn_urls_compiled {
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

    let pb = ProgressBar::new(urls.len() as u64);
    let urls_iter = urls.par_iter();
    let urls_iter_pb = urls.par_iter().progress_with(pb);

    match flags.pbar {
        false => {
            valid_urls = urls_iter
                .filter_map(|url| {
                    let unmuted = match crate::HTTP_CLIENT.get(url.fragment.as_str()).send() {
                        Ok(r) => r.status(),
                        Err(_) => return None,
                    };
                    let muted = match crate::HTTP_CLIENT.get(url.fragment_muted.as_str()).send() {
                        Ok(r) => r.status(),
                        Err(_) => return None,
                    };
                    if unmuted == 200 {
                        Some(ReturnURL {
                            playlist: url.playlist.clone(),
                            muted: false,
                        })
                    } else if muted == 200 {
                        Some(ReturnURL {
                            playlist: url.playlist.clone(),
                            muted: true,
                        })
                    } else {
                        None
                    }
                })
                .collect();
        }
        true => {
            valid_urls = urls_iter_pb
                .filter_map(|url| {
                    let unmuted = match crate::HTTP_CLIENT.get(url.fragment.as_str()).send() {
                        Ok(r) => r.status(),
                        Err(_) => return None,
                    };
                    let muted = match crate::HTTP_CLIENT.get(url.fragment_muted.as_str()).send() {
                        Ok(r) => r.status(),
                        Err(_) => return None,
                    };
                    if unmuted == 200 {
                        Some(ReturnURL {
                            playlist: url.playlist.clone(),
                            muted: false,
                        })
                    } else if muted == 200 {
                        Some(ReturnURL {
                            playlist: url.playlist.clone(),
                            muted: true,
                        })
                    } else {
                        None
                    }
                })
                .collect();
        }
    }

    valid_urls
}

#[cfg(test)]
mod tests {
    use crate::{config::Flags, twitch::models::ReturnURL};

    use super::check_availability as ca;

    #[test]
    fn check_availability() {
        // https://twitchtracker.com/dansgaming/streams/42218705421 - d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217 - 2021-06-05 00:50:17
        let ca_working: Vec<ReturnURL> = ca(
            &"d3dcbaf880c9e36ed8c8".to_string(),
            "dansgaming",
            42218705421,
            &1622854217,
            Flags::default(),
        );

        let comp_working: Vec<ReturnURL> = vec![ReturnURL {
            playlist: "https://d1m7jfoe9zdc1j.cloudfront.net/d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217/chunked/index-dvr.m3u8".to_string(),
            muted: false,
        }, ReturnURL {
            playlist: "https://d2vjef5jvl6bfs.cloudfront.net/d3dcbaf880c9e36ed8c8_dansgaming_42218705421_1622854217/chunked/index-dvr.m3u8".to_string(),
            muted: false,
        }];

        assert_eq!(
            ca_working, comp_working,
            "testing valid vod (dansgaming - 2021)"
        );

        // https://twitchtracker.com/forsen/streams/23722143840 - d45bc961583725d59867_forsen_23722143840_1479745189 - 2016-11-21 16:19:49
        let ca_not_working: Vec<ReturnURL> = ca(
            &"d45bc961583725d59867".to_string(),
            "forsen",
            23722143840,
            &1479745189,
            Flags::default(),
        );

        let comp_not_working: Vec<ReturnURL> = vec![];

        assert_eq!(
            ca_not_working, comp_not_working,
            "testing invalid vod (forsen - 2016)"
        );
    }
}
