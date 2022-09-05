use anyhow::Result;
use crossterm::event;
use lazy_static::lazy_static;
use log::debug;
use log::info;
use rand::seq::SliceRandom;
use regex::Regex;
use reqwest::header::USER_AGENT;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs::File,
    io::{stdout, Read, Write},
    path::Path,
};
use time::{
    format_description::well_known::Rfc3339, macros::format_description, PrimitiveDateTime,
};
use url::Url;

use super::config::CURL_UA;
use crate::error::DeriveDateError;
use crate::twitch::models::CDN_URLS;

lazy_static! {
    static ref RE_UNIX: Regex = Regex::new(r"^\d*$").unwrap();
    static ref RE_UTC: Regex = Regex::new("UTC").unwrap();
}

#[derive(Debug, PartialEq)]
pub enum ProcessingType {
    Exact,
    Bruteforce,
}

#[derive(Debug, PartialEq)]
pub struct URLData {
    pub username: String,
    pub broadcast_id: String,
    pub start_date: String,
    pub end_date: Option<String>,
}

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

pub fn get_random_useragent() -> String {
    let resp = crate::HTTP_CLIENT
        .get("https://jnrbsn.github.io/user-agents/user-agents.json")
        .send();

    match resp {
        Ok(r) => match r.status().is_success() {
            true => {
                let mut useragent_vec: Vec<String> = match r.json() {
                    Ok(v) => v,
                    Err(_) => return CURL_UA.to_string(),
                };
                // apparently streamscharts doesnt like when the useragent has "X11" in it
                useragent_vec.retain(|a| !a.contains("X11;"));
                return match useragent_vec.choose(&mut rand::thread_rng()) {
                    Some(ua) => ua.to_owned(),
                    None => CURL_UA.to_string(),
                };
            }
            false => {}
        },
        Err(_) => {}
    }

    return CURL_UA.to_string();
}

fn process_url(url: &str) -> Result<Html> {
    let ua = get_random_useragent();
    debug!("Using UA - {}", ua);
    let init_resp = match crate::HTTP_CLIENT.get(url).header(USER_AGENT, ua).send() {
        Ok(r) => r,
        Err(e) => return Err(e)?,
    };

    let resp = match init_resp.error_for_status() {
        Ok(e) => e,
        Err(e) => return Err(e)?,
    };

    let body = match resp.text() {
        Ok(b) => b,
        Err(e) => return Err(e)?,
    };
    Ok(Html::parse_document(&body))
}

pub fn derive_date_from_url(url: &str) -> Result<(ProcessingType, URLData)> {
    match Url::parse(url) {
        Ok(resolved_url) => match resolved_url.domain() {
            Some(domain) => match domain.to_lowercase().as_str() {
                "twitchtracker.com" | "www.twitchtracker.com" => {
                    let segments = match resolved_url
                        .path_segments()
                        .map(|c| c.collect::<Vec<_>>())
                        .ok_or(DeriveDateError::SegmentMapError)
                    {
                        Ok(s) => s,
                        Err(e) => return Err(e)?,
                    };
                    if segments.len() == 3 {
                        if segments[1] == "streams" {
                            let username = segments[0];
                            let broadcast_id = segments[2];
                            let fragment = match process_url(url) {
                                Ok(f) => f,
                                Err(e) => Err(e)?,
                            };
                            let selector =
                                match Selector::parse(".stream-timestamp-dt.to-dowdatetime") {
                                    Ok(s) => s,
                                    Err(_) => return Err(DeriveDateError::SelectorError)?,
                                };

                            let date = match fragment
                                .select(&selector)
                                .nth(0)
                                .ok_or(DeriveDateError::ScraperElementError)
                            {
                                Ok(d) => d.text().collect::<String>(),
                                Err(e) => return Err(e)?,
                            };

                            return Ok((
                                ProcessingType::Exact,
                                URLData {
                                    username: username.to_string(),
                                    broadcast_id: broadcast_id.to_string(),
                                    start_date: date,
                                    end_date: None,
                                },
                            ));
                        } else {
                            return Err(DeriveDateError::WrongURLError(
                                "Not a valid TwitchTracker VOD URL".to_string(),
                            ))?;
                        };
                    } else {
                        return Err(DeriveDateError::WrongURLError(
                            "Not a valid TwitchTracker VOD URL".to_string(),
                        ))?;
                    };
                }
                "streamscharts.com" | "www.streamscharts.com" => {
                    let segments = match resolved_url
                        .path_segments()
                        .map(|c| c.collect::<Vec<_>>())
                        .ok_or(DeriveDateError::SegmentMapError)
                    {
                        Ok(s) => s,
                        Err(e) => return Err(e)?,
                    };
                    if segments.len() == 4 {
                        if segments[0] == "channels" && segments[2] == "streams" {
                            let username = segments[1];
                            let broadcast_id = segments[3];
                            let fragment = match process_url(url) {
                                Ok(f) => f,
                                Err(e) => Err(e)?,
                            };
                            let selector = match Selector::parse("time") {
                                Ok(s) => s,
                                Err(_) => Err(DeriveDateError::SelectorError)?,
                            };

                            let date_init = match fragment
                                .select(&selector)
                                .nth(0)
                                .ok_or(DeriveDateError::ScraperElementError)
                            {
                                Ok(d) => {
                                    match d
                                        .value()
                                        .attr("datetime")
                                        .ok_or(DeriveDateError::ScraperAttributeError)
                                    {
                                        Ok(s) => s.to_string(),
                                        Err(e) => return Err(e)?,
                                    }
                                }
                                Err(e) => return Err(e)?,
                            };

                            let date_parsed = match parse_timestamp(&date_init) {
                                Ok(d) => d,
                                Err(e) => return Err(e)?,
                            };
                            let start_date = date_parsed - 60;
                            let end_date = date_parsed + 60;

                            return Ok((
                                ProcessingType::Bruteforce,
                                URLData {
                                    username: username.to_string(),
                                    broadcast_id: broadcast_id.to_string(),
                                    start_date: start_date.to_string(),
                                    end_date: Some(end_date.to_string()),
                                },
                            ));
                        } else {
                            return Err(DeriveDateError::WrongURLError(
                                "Not a valid StreamsCharts VOD URL".to_string(),
                            ))?;
                        };
                    } else {
                        return Err(DeriveDateError::WrongURLError(
                            "Not a valid StreamsCharts VOD URL".to_string(),
                        ))?;
                    };
                }
                _ => {
                    return Err(DeriveDateError::WrongURLError(
                        "Only twitchtracker.com and streamscharts.com URLs are supported"
                            .to_string(),
                    ))?
                }
            },
            None => {
                return Err(DeriveDateError::WrongURLError(
                    "Only twitchtracker.com and streamscharts.com URLs are supported".to_string(),
                ))?
            }
        },
        Err(e) => return Err(e)?,
    }
}

pub fn parse_timestamp(timestamp: &str) -> Result<i64> {
    let format_with_utc = format_description!("[year]-[month]-[day] [hour]:[minute]:[second] UTC");
    let format_wo_utc = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let format_wo_sec = format_description!("[day]-[month]-[year] [hour]:[minute]");

    if RE_UNIX.is_match(timestamp) {
        match timestamp.parse::<i64>() {
            Ok(i) => Ok(i),
            Err(e) => Err(e)?,
        }
    } else {
        if RE_UTC.is_match(timestamp) {
            match PrimitiveDateTime::parse(timestamp, format_with_utc) {
                Ok(d) => Ok(d.assume_utc().unix_timestamp()),
                Err(e) => Err(e)?,
            }
        } else {
            let parsed_rfc = PrimitiveDateTime::parse(timestamp, &Rfc3339);
            match parsed_rfc {
                Ok(result) => Ok(result.assume_utc().unix_timestamp()),
                Err(_) => {
                    let parsed_wo_utc = PrimitiveDateTime::parse(timestamp, format_wo_utc);
                    match parsed_wo_utc {
                        Ok(result) => Ok(result.assume_utc().unix_timestamp()),
                        Err(_) => match PrimitiveDateTime::parse(timestamp, format_wo_sec) {
                            Ok(result) => Ok(result.assume_utc().unix_timestamp()),
                            Err(e) => Err(e)?,
                        },
                    }
                }
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

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    use crate::twitch::models::CDN_URLS;

    use super::{compile_cdn_list, derive_date_from_url, parse_timestamp, ProcessingType, URLData};

    #[test]
    fn compile_cdns() {
        let dir = tempdir().unwrap();
        let mut cdn_urls_string: Vec<String> = CDN_URLS.iter().map(|s| s.to_string()).collect();
        cdn_urls_string.push("test.cloudflare.net".to_string());
        cdn_urls_string.sort();

        let path_txt = dir.path().join("cdn_test.txt");
        let mut file_txt = File::create(path_txt.clone()).unwrap();

        writeln!(file_txt, "test.cloudflare.net").unwrap();

        let mut res_txt = compile_cdn_list(Some(path_txt.to_str().unwrap().to_string()));
        res_txt.sort();

        assert_eq!(res_txt, cdn_urls_string, "testing txt file");

        let path_json = dir.path().join("cdn_test.json");
        let mut file_json = File::create(path_json.clone()).unwrap();

        writeln!(file_json, "{{").unwrap();
        writeln!(file_json, "\"cdns\": [\"test.cloudflare.net\"]").unwrap();
        writeln!(file_json, "}}").unwrap();

        let mut res_json = compile_cdn_list(Some(path_json.to_str().unwrap().to_string()));
        res_json.sort();

        assert_eq!(res_json, cdn_urls_string, "testing json file");

        let path_toml = dir.path().join("cdn_test.toml");
        let mut file_toml = File::create(path_toml.clone()).unwrap();

        writeln!(file_toml, "cdns = [\"test.cloudflare.net\"]").unwrap();

        let mut res_toml = compile_cdn_list(Some(path_toml.to_str().unwrap().to_string()));
        res_toml.sort();

        assert_eq!(res_toml, cdn_urls_string, "testing toml file");

        let path_yaml1 = dir.path().join("cdn_test.yaml");
        let mut file_yaml1 = File::create(path_yaml1.clone()).unwrap();

        writeln!(file_yaml1, "\"cdns\": [\"test.cloudflare.net\"]").unwrap();

        let path_yaml2 = dir.path().join("cdn_test.yml");
        let mut file_yaml2 = File::create(path_yaml2.clone()).unwrap();

        writeln!(file_yaml2, "\"cdns\": [\"test.cloudflare.net\"]").unwrap();

        let mut res_yaml1 = compile_cdn_list(Some(path_yaml1.to_str().unwrap().to_string()));
        res_yaml1.sort();

        assert_eq!(res_yaml1, cdn_urls_string, "testing yaml file");

        let mut res_yaml2 = compile_cdn_list(Some(path_yaml2.to_str().unwrap().to_string()));
        res_yaml2.sort();

        assert_eq!(res_yaml2, cdn_urls_string, "testing yml file");

        let path_png = dir.path().join("cdn_test.png");

        let mut res_png = compile_cdn_list(Some(path_png.to_str().unwrap().to_string()));
        res_png.sort();

        assert_ne!(
            res_png, cdn_urls_string,
            "testing unsupported extension (should be unequal)"
        );

        let mut cdn_urls_string_init: Vec<String> =
            CDN_URLS.iter().map(|s| s.to_string()).collect();
        cdn_urls_string_init.sort();

        assert_eq!(
            res_png, cdn_urls_string_init,
            "testing unsupported extension (should be equal)"
        );
    }

    #[test]
    fn parse_timestamps() {
        assert_eq!(
            parse_timestamp("1657871396").unwrap(),
            1657871396,
            "testing unix timestamp parsing"
        );
        assert_eq!(
            parse_timestamp("2022-07-15T07:49:56+00:00").unwrap(),
            1657871396,
            "testing rfc parsing"
        );
        assert_eq!(
            parse_timestamp("2022-07-15 07:49:56 UTC").unwrap(),
            1657871396,
            "testing parsing time with the UTC tag"
        );
        assert_eq!(
            parse_timestamp("2022-07-15 07:49:56").unwrap(),
            1657871396,
            "testing parsing time w/o the UTC tag"
        );
        assert_eq!(
            parse_timestamp("15-07-2022 07:49").unwrap(),
            1657871340,
            "testing parsing time w/o seconds"
        );
        assert!(
            parse_timestamp("2022-07-15 0749").is_err(),
            "testing parsing wrong timestamps"
        );
    }

    #[test]
    fn derive_date() {
        assert_eq!(
            derive_date_from_url("https://twitchtracker.com/forsen/streams/39619965384").unwrap(),
            (
                ProcessingType::Exact,
                URLData {
                    username: "forsen".to_string(),
                    broadcast_id: "39619965384".to_string(),
                    start_date: "2022-07-12 17:05:08".to_string(),
                    end_date: None
                }
            ),
            "testing twitchtracker - https://twitchtracker.com/forsen/streams/39619965384"
        );

        assert_eq!(
            derive_date_from_url("https://streamscharts.com/channels/forsen/streams/39619965384")
                .unwrap(),
            (
                ProcessingType::Bruteforce,
                URLData {
                    username: "forsen".to_string(),
                    broadcast_id: "39619965384".to_string(),
                    start_date: "1657645440".to_string(),
                    end_date: Some("1657645560".to_string())
                }
            ),
            "testing streamscharts - https://streamscharts.com/channels/forsen/streams/39619965384"
        );

        assert!(
            derive_date_from_url("https://google.com").is_err(),
            "testing wrong link - https://google.com"
        );
        assert!(derive_date_from_url("https://twitchtracker.com/forsen/streams/3961965384").is_err(), "testing wrong twitchtracker link 1 - https://twitchtracker.com/forsen/streams/3961965384");
        assert!(derive_date_from_url("https://streamscharts.com/channels/forsen/streams/3961965384").is_err(), "testing wrong streamscharts link 1 - https://streamscharts.com/channels/forsen/streams/3961965384");
        assert!(derive_date_from_url("https://twitchtracker.com/forsen/sreams/39619965384").is_err(), "testing wrong twitchtracker link 2 - https://twitchtracker.com/forsen/sreams/39619965384");
        assert!(derive_date_from_url("https://streamscharts.com/channels/forsen/sreams/39619965384").is_err(), "testing wrong streamscharts link 2 - https://streamscharts.com/channels/forsen/sreams/39619965384");
    }
}
