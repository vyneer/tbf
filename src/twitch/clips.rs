use crate::config::Flags;
use crate::twitch::models;
use crate::util::info;

use colored::*;
use indicatif::{ParallelProgressIterator, ProgressBar};
use log::info;
use rayon::prelude::*;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::{collections::HashMap, str::FromStr};

pub fn find_bid_from_clip(slug: String) -> (String, i64) {
    let endpoint = "https://gql.twitch.tv/gql";
    let mut headers = HashMap::new();
    headers.insert("Client-ID", "kimne78kx3ncx6brgo4mv6wki5h1ko");

    let mut header_map = HeaderMap::new();

    for (str_key, str_value) in headers {
        let key = HeaderName::from_str(str_key).unwrap();
        let val = HeaderValue::from_str(str_value).unwrap();

        header_map.insert(key, val);
    }

    let query = models::Query {
        query: "query($slug:ID!){clip(slug: $slug){broadcaster{login}broadcast{id}}}".to_string(),
        variables: models::Vars { slug },
    };

    let request = crate::HTTP_CLIENT
        .post(endpoint)
        .json(&query)
        .headers(header_map.clone());

    let re = request.send().unwrap();
    let data: models::Response = re.json().unwrap();
    (
        data.data.clip.broadcaster.login,
        data.data.clip.broadcast.id.parse::<i64>().unwrap(),
    )
}

pub fn clip_bruteforce(vod: i64, start: i64, end: i64, flags: Flags) {
    let vod = vod.to_string();
    let pb = ProgressBar::new((end - start) as u64);
    let cloned_pb = pb.clone();

    let iter = (start..end).into_par_iter();
    let iter_pb = (start..end).into_par_iter().progress_with(pb);
    let res: Vec<String>;

    if flags.pbar {
        res = iter_pb.filter_map( |number| {
            let url = format!("https://clips-media-assets2.twitch.tv/AT-cm%7C{}-offset-{}-360.mp4", vod, number);
            let res = crate::HTTP_CLIENT.get(url.as_str()).send().unwrap();
            if res.status() == 200 {
                if flags.verbose {
                    cloned_pb.println(format!("Got a clip! - {}", url));
                }
                Some(url)
            } else if res.status() == 403 {
                if flags.verbose {
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
            let res = crate::HTTP_CLIENT.get(url.as_str()).send().unwrap();
            if res.status() == 200 {
                if flags.verbose {
                    cloned_pb.println(format!("Got a clip! - {}", url));
                }
                Some(url)
            } else if res.status() == 403 {
                if flags.verbose {
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
        if !flags.simple {
            info!("{}! Here are the URLs:", "Got some clips".green());
        }
        for line in res {
            info(line, flags.simple);
        }
    } else {
        if !flags.simple {
            info!("{}", "Couldn't find anything :(".red());
        }
    }
}
