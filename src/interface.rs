use anyhow::Result;
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use log::error;
use std::io::{stdin, stdout, Write};
use strum::{EnumMessage, IntoEnumIterator};

use crate::config::{Cli, Commands, ProcessingType};
use crate::twitch::{
    clips::{clip_bruteforce, find_bid_from_clip},
    models::ReturnURL,
    vods::{bruteforcer, exact, fix, live},
};
use crate::util::derive_date_from_url;

impl Commands {
    fn fill_out_values(&mut self) -> Result<()> {
        match self {
            Self::Exact {
                username,
                id,
                stamp,
            } => {
                let mut vod = String::new();

                ask_for_value("Please enter the streamer's username:", username);

                ask_for_value("Please enter the VOD/broadcast ID:", &mut vod);
                *id = match vod.parse::<i64>() {
                    Ok(v) => v,
                    Err(e) => Err(e)?,
                };

                ask_for_value("Please enter the timestamp:", stamp);

                Ok(())
            }
            Self::Bruteforce {
                username,
                id,
                from,
                to,
            } => {
                let mut vod = String::new();

                ask_for_value("Please enter the streamer's username:", username);

                ask_for_value("Please enter the VOD/broadcast ID:", &mut vod);
                *id = match vod.parse::<i64>() {
                    Ok(v) => v,
                    Err(e) => Err(e)?,
                };

                ask_for_value("Please enter the first timestamp:", from);
                ask_for_value("Please enter the last timestamp:", to);

                Ok(())
            }
            Self::Link { url } => {
                ask_for_value("Please enter the TwitchTracker or StreamsCharts URL:", url);
                Ok(())
            }
            Self::Live { username } => {
                ask_for_value("Please enter the streamer's username:", username);
                Ok(())
            }
            Self::Clip { clip } => {
                ask_for_value("Please enter the clip's URL (twitch.tv/%username%/clip/%slug% and clips.twitch.tv/%slug% are both supported) or the slug (\"GentleAthleticWombatHoneyBadger-ohJAsKzGinIgFUx2\" for example):", clip);
                Ok(())
            }
            Self::Clipforce { id, start, end } => {
                let mut id_string = String::new();
                let mut start_string = String::new();
                let mut end_string = String::new();

                ask_for_value("Please enter the VOD/broadcast ID:", &mut id_string);
                *id = match id_string.parse::<i64>() {
                    Ok(v) => v,
                    Err(e) => Err(e)?,
                };

                ask_for_value(
                    "Please enter the starting timestamp (in seconds):",
                    &mut start_string,
                );
                *start = match start_string.parse::<i64>() {
                    Ok(v) => v,
                    Err(e) => Err(e)?,
                };

                ask_for_value(
                    "Please enter the end timestamp (in seconds):",
                    &mut end_string,
                );
                *end = match end_string.parse::<i64>() {
                    Ok(v) => v,
                    Err(e) => Err(e)?,
                };

                Ok(())
            }
            Self::Fix { url, .. } => {
                ask_for_value("Please enter Twitch VOD m3u8 playlist URL (only twitch.tv and cloudfront.net URLs are supported):", url);
                Ok(())
            }
        }
    }

    pub fn execute(self, matches: Cli) -> Result<Option<Vec<ReturnURL>>> {
        match self {
            Self::Exact {
                username,
                id,
                stamp,
            } => exact(username.as_str(), id, stamp.as_str(), matches),
            Self::Bruteforce {
                username,
                id,
                from,
                to,
            } => bruteforcer(username.as_str(), id, from.as_str(), to.as_str(), matches),
            Self::Link { url } => {
                let (proc, data) = match derive_date_from_url(&url, matches.clone()) {
                    Ok(a) => a,
                    Err(e) => {
                        return Err(e)?;
                    }
                };

                match proc {
                    ProcessingType::Exact => exact(
                        data.username.as_str(),
                        match data.broadcast_id.parse::<i64>() {
                            Ok(b) => b,
                            Err(e) => {
                                return Err(e)?;
                            }
                        },
                        data.start_date.as_str(),
                        matches.clone(),
                    ),
                    ProcessingType::Bruteforce => {
                        let end_date = match data.end_date {
                            Some(d) => d,
                            None => {
                                error!("Couldn't get the end date for the bruteforce method");
                                return Ok(None);
                            }
                        };
                        bruteforcer(
                            data.username.as_str(),
                            match data.broadcast_id.parse::<i64>() {
                                Ok(b) => b,
                                Err(e) => return Err(e)?,
                            },
                            data.start_date.as_str(),
                            end_date.as_str(),
                            matches.clone(),
                        )
                    }
                }
            }
            Self::Live { username } => live(username.as_str(), matches),
            Self::Clip { clip } => match find_bid_from_clip(clip, matches.clone()) {
                Ok(r) => match r {
                    Some((username, vod)) => {
                        let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
                        let (_, data) = match derive_date_from_url(&url, matches.clone()) {
                            Ok(a) => a,
                            Err(e) => Err(e)?,
                        };

                        exact(&username, vod, &data.start_date, matches)
                    }
                    None => Ok(None),
                },
                Err(e) => Err(e)?,
            },
            Self::Clipforce { id, start, end } => clip_bruteforce(id, start, end, matches),
            Self::Fix { url, output, slow } => {
                fix(url.as_str(), output, slow, matches).expect("fix - shouldn't happen");

                // this might not be the right way to this
                // but i want to combine everything into one method
                Ok(None)
            }
        }
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

fn any_key_to_continue(text: &str) {
    enable_raw_mode().unwrap();
    print!("\n{}", text);
    stdout().flush().unwrap_or(());
    loop {
        match event::read().unwrap() {
            Event::Key(..) => break,
            _ => (),
        };
    }
    disable_raw_mode().unwrap();
}

fn ask_for_value(desc: &str, buf: &mut String) {
    println!("{}", desc);
    stdin().read_line(buf).expect("Failed to read line.");
    trim_newline(buf);
}

fn try_to_fix(valid_urls: Vec<ReturnURL>, matches: Cli) {
    if !valid_urls.is_empty() {
        if valid_urls[0].muted {
            let mut response = String::new();

            ask_for_value(
                "Do you want to download the fixed playlist? (Y/n)",
                &mut response,
            );

            match response.to_lowercase().as_str() {
                "y" | "" => Commands::Fix {
                    url: valid_urls[0].url.clone(),
                    output: None,
                    slow: false,
                }
                .execute(matches)
                .expect("fix - shouldn't happen"),
                _ => None,
            };
        }
    }
}

pub fn main_interface(mut matches: Cli) {
    // forcing the progress bar option on
    matches = Cli {
        progressbar: true,
        ..matches
    };

    let mut mode = String::new();

    println!("Select the application mode:");
    for (i, com) in Commands::iter().enumerate() {
        println!(
            "[{}] {} - {}",
            i + 1,
            com.to_short_desc(),
            com.get_documentation()
                .unwrap_or("<error - couldn't get mode description>")
        )
    }

    stdin().read_line(&mut mode).expect("Failed to read line.");
    trim_newline(&mut mode);
    let mode = match mode.parse::<usize>() {
        Ok(res) => res,
        Err(_) => {
            error!("Couldn't select the specified mode");
            any_key_to_continue("Press any key to close...");
            return;
        }
    };

    (|| match Commands::from_selector(mode) {
        Some(mut sub) => {
            match sub.fill_out_values() {
                Err(e) => {
                    error!("{}", e);
                    return;
                }
                _ => (),
            }
            let valid_urls = match sub.execute(matches.clone()) {
                Ok(u) => match u {
                    Some(u) => u,
                    None => Vec::new(),
                },
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
            try_to_fix(valid_urls, matches);
            return;
        }
        None => {
            error!("Couldn't select the specified mode");
            return;
        }
    })();

    any_key_to_continue("Press any key to close...");
}
