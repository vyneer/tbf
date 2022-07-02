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
                    let unmuted = crate::HTTP_CLIENT
                        .get(url.fragment.as_str())
                        .send()
                        .unwrap()
                        .status();
                    let muted = crate::HTTP_CLIENT
                        .get(url.fragment_muted.as_str())
                        .send()
                        .unwrap()
                        .status();
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
                    let unmuted = crate::HTTP_CLIENT
                        .get(url.fragment.as_str())
                        .send()
                        .unwrap()
                        .status();
                    let muted = crate::HTTP_CLIENT
                        .get(url.fragment_muted.as_str())
                        .send()
                        .unwrap()
                        .status();
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
