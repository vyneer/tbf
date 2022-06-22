use crossterm::event;
use log::debug;
use log::info;
use regex::Regex;
use reqwest::header::USER_AGENT;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs::File,
    io::{stdout, Read, Write},
    panic,
    path::Path,
};
use time::{
    format_description::well_known::Rfc3339, macros::format_description, PrimitiveDateTime,
};
use url::Url;

use crate::twitch::models::CDN_URLS;

#[derive(Debug, Deserialize)]
pub struct CDNFile {
    cdns: Vec<String>,
}

pub fn any_key_to_continue(text: &str) {
    print!("{}", text);
    stdout().flush().unwrap_or(());
    event::read().unwrap();
}

pub fn info(text: String, simple: bool) {
    if simple {
        println!("{}", text);
    } else {
        info!("{}", text);
    }
}

pub fn trim_newline(s: &mut String) {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
}

pub fn derive_date_from_url(url: &str) -> (String, String, String) {
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

    let resp = crate::HTTP_CLIENT
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

pub fn parse_timestamp(timestamp: &str) -> i64 {
    let re_unix = Regex::new(r"^\d*$").unwrap();
    let re_utc = Regex::new("UTC").unwrap();
    let format_with_utc = format_description!("[year]-[month]-[day] [hour]:[minute]:[second] UTC");
    let format_wo_utc = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

    if re_unix.is_match(timestamp) {
        timestamp.parse::<i64>().unwrap()
    } else {
        if re_utc.is_match(timestamp) {
            PrimitiveDateTime::parse(timestamp, format_with_utc)
                .unwrap()
                .assume_utc()
                .unix_timestamp()
        } else {
            let parsed_rfc = PrimitiveDateTime::parse(timestamp, &Rfc3339);
            match parsed_rfc {
                Ok(result) => result.assume_utc().unix_timestamp(),
                Err(_) => PrimitiveDateTime::parse(timestamp, format_wo_utc)
                    .unwrap()
                    .assume_utc()
                    .unix_timestamp(),
            }
        }
    }
}

pub fn compile_cdn_list(cdn_file_path: Option<String>) -> Vec<String> {
    let cdn_urls_string: Vec<String> = CDN_URLS.iter().map(|s| s.to_string()).collect();
    let mut return_vec: Vec<String> = Vec::new();
    return_vec.extend(cdn_urls_string);

    let mut cdn_file: Option<File> = None;
    let mut file_extension: Option<&OsStr> = None;

    let actual_path: String;

    match cdn_file_path {
        Some(s) => {
            actual_path = s.clone();
            let cdn_file_init = File::open(s);
            match cdn_file_init {
                Ok(f) => {
                    cdn_file = Some(f);
                    file_extension = Path::new(&actual_path).extension();
                }
                Err(e) => {
                    info!("Couldn't open the CDN config file - {:#?}", e);
                }
            }
        }
        None => return return_vec,
    }

    match file_extension {
        Some(ext) => match cdn_file {
            Some(mut f) => {
                let mut cdn_string = String::new();

                f.read_to_string(&mut cdn_string).unwrap();

                match ext.to_str().unwrap() {
                    "json" => {
                        let json_init: Result<CDNFile, serde_json::Error> =
                            serde_json::from_str(cdn_string.as_str());
                        match json_init {
                            Ok(j) => {
                                return_vec.extend(j.cdns);

                                return_vec.sort_unstable();
                                return_vec.dedup();
                            }
                            Err(e) => {
                                info!("Couldn't parse the CDN list file: invalid JSON - {:#?}", e);
                            }
                        }
                    }
                    "toml" => {
                        let toml_init: Result<CDNFile, toml::de::Error> =
                            toml::from_str(cdn_string.as_str());
                        match toml_init {
                            Ok(t) => {
                                return_vec.extend(t.cdns);

                                return_vec.sort_unstable();
                                return_vec.dedup();
                            }
                            Err(e) => {
                                info!("Couldn't parse the CDN list file: invalid TOML - {:#?}", e);
                            }
                        }
                    }
                    "yaml" | "yml" => {
                        let yaml_init: Result<CDNFile, serde_yaml::Error> =
                            serde_yaml::from_str(cdn_string.as_str());
                        match yaml_init {
                            Ok(y) => {
                                return_vec.extend(y.cdns);

                                return_vec.sort_unstable();
                                return_vec.dedup();
                            }
                            Err(e) => {
                                info!("Couldn't parse the CDN list file: invalid YAML - {:#?}", e);
                            }
                        }
                    }
                    "txt" => {
                        let mut cdn_string_split: Vec<String> =
                            cdn_string.lines().map(|l| l.to_string()).collect();

                        return_vec.append(&mut cdn_string_split);

                        return_vec.sort_unstable();
                        return_vec.dedup();
                    }
                    _ => {
                        info!("Couldn't parse the CDN list file: it must either be a text file, a JSON file, a TOML file or a YAML file.");
                    }
                }
            }
            None => {}
        },
        None => match cdn_file {
            Some(mut f) => {
                let mut cdn_string = String::new();

                f.read_to_string(&mut cdn_string).unwrap();

                cdn_string.retain(|c| !c.is_whitespace());
                let mut cdn_string_split: Vec<String> =
                    cdn_string.lines().map(|l| l.to_string()).collect();

                return_vec.append(&mut cdn_string_split);

                return_vec.sort_unstable();
                return_vec.dedup();
            }
            None => {
                info!("Couldn't parse the CDN list file: it must either be a text file, a JSON file, a TOML file or a YAML file.");
            }
        },
    }

    if return_vec.len() != CDN_URLS.len() {
        debug!(
            "Compiled the new CDN list - initial length: {}, new length: {}",
            CDN_URLS.len(),
            return_vec.len()
        );
    } else {
        debug!(
            "No new CDNs added - initial length: {}, new length: {}",
            CDN_URLS.len(),
            return_vec.len()
        );
    }

    return_vec
}
