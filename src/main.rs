mod config;
mod error;
mod twitch;
mod util;

use clap::{crate_name, crate_version, Parser};
use crossterm::{execute, terminal::SetTitle};
use env_logger::Env;
use lazy_static::lazy_static;
use log::{debug, error};
use std::{
    io::{stdin, stdout},
    panic,
};
use strum::{EnumMessage, IntoEnumIterator};

use config::{Cli, Commands};
use twitch::{
    clips::{clip_bruteforce, find_bid_from_clip},
    vods::{bruteforcer, exact, fix, live},
};
use util::{any_key_to_continue, derive_date_from_url, trim_newline, ProcessingType};

lazy_static! {
    // HTTP client to share
    static ref HTTP_CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::new();
}

fn interface(mut matches: Cli) {
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
        Some(sub) => match sub {
            Commands::Exact { .. } => {
                let mut username = String::new();
                let mut vod = String::new();
                let mut initial_stamp = String::new();

                println!("Please enter the streamer's username:");
                stdin()
                    .read_line(&mut username)
                    .expect("Failed to read line.");
                trim_newline(&mut username);
                println!("Please enter the VOD/broadcast ID:");
                stdin().read_line(&mut vod).expect("Failed to read line.");
                trim_newline(&mut vod);
                println!("Please enter the timestamp:");
                stdin()
                    .read_line(&mut initial_stamp)
                    .expect("Failed to read line.");
                trim_newline(&mut initial_stamp);

                let valid_urls = match exact(
                    username.as_str(),
                    match vod.parse::<i64>() {
                        Ok(v) => v,
                        Err(e) => {
                            error!("{}", e);
                            return;
                        }
                    },
                    initial_stamp.as_str(),
                    matches.clone(),
                ) {
                    Ok(u) => match u {
                        Some(u) => u,
                        None => Vec::new(),
                    },
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };
                if !valid_urls.is_empty() {
                    if valid_urls[0].muted {
                        let mut response = String::new();

                        println!("Do you want to download the fixed playlist? (Y/n)");
                        stdin()
                            .read_line(&mut response)
                            .expect("Failed to read line.");
                        trim_newline(&mut response);

                        match response.to_lowercase().as_str() {
                            "y" | "" => {
                                match fix(valid_urls[0].playlist.as_str(), None, false, matches) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("{}", e);
                                        return;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                return;
            }
            Commands::Bruteforce { .. } => {
                let mut username = String::new();
                let mut vod = String::new();
                let mut initial_from_stamp = String::new();
                let mut initial_to_stamp = String::new();

                println!("Please enter the streamer's username:");
                stdin()
                    .read_line(&mut username)
                    .expect("Failed to read line.");
                trim_newline(&mut username);
                println!("Please enter the VOD/broadcast ID:");
                stdin().read_line(&mut vod).expect("Failed to read line.");
                trim_newline(&mut vod);
                println!("Please enter the first timestamp:");
                stdin()
                    .read_line(&mut initial_from_stamp)
                    .expect("Failed to read line.");
                trim_newline(&mut initial_from_stamp);
                println!("Please enter the last timestamp:");
                stdin()
                    .read_line(&mut initial_to_stamp)
                    .expect("Failed to read line.");
                trim_newline(&mut initial_to_stamp);

                let valid_urls = match bruteforcer(
                    username.as_str(),
                    match vod.parse::<i64>() {
                        Ok(v) => v,
                        Err(e) => {
                            error!("{}", e);
                            return;
                        }
                    },
                    initial_from_stamp.as_str(),
                    initial_to_stamp.as_str(),
                    matches.clone(),
                ) {
                    Ok(u) => match u {
                        Some(u) => u,
                        None => Vec::new(),
                    },
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };
                if !valid_urls.is_empty() {
                    if valid_urls[0].muted {
                        let mut response = String::new();

                        println!("Do you want to download the fixed playlist? (Y/n)");
                        stdin()
                            .read_line(&mut response)
                            .expect("Failed to read line.");
                        trim_newline(&mut response);

                        match response.to_lowercase().as_str() {
                            "y" | "" => {
                                match fix(valid_urls[0].playlist.as_str(), None, false, matches) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("{}", e);
                                        return;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                return;
            }
            Commands::Link { .. } => {
                let mut url = String::new();

                println!("Please enter the TwitchTracker or StreamsCharts URL:");
                stdin().read_line(&mut url).expect("Failed to read line.");
                trim_newline(&mut url);

                let (proc, data) = match derive_date_from_url(&url, matches.clone()) {
                    Ok(a) => a,
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };

                let valid_urls = match proc {
                    ProcessingType::Exact => {
                        match exact(
                            data.username.as_str(),
                            match data.broadcast_id.parse::<i64>() {
                                Ok(b) => b,
                                Err(e) => {
                                    error!("{}", e);
                                    return;
                                }
                            },
                            data.start_date.as_str(),
                            matches.clone(),
                        ) {
                            Ok(u) => match u {
                                Some(u) => u,
                                None => Vec::new(),
                            },
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        }
                    }
                    ProcessingType::Bruteforce => {
                        let end_date = match data.end_date {
                            Some(d) => d,
                            None => {
                                error!("Couldn't get the end date for the bruteforce method");
                                return;
                            }
                        };
                        match bruteforcer(
                            data.username.as_str(),
                            match data.broadcast_id.parse::<i64>() {
                                Ok(b) => b,
                                Err(e) => {
                                    error!("{}", e);
                                    return;
                                }
                            },
                            data.start_date.as_str(),
                            end_date.as_str(),
                            matches.clone(),
                        ) {
                            Ok(u) => match u {
                                Some(u) => u,
                                None => Vec::new(),
                            },
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        }
                    }
                };
                if !valid_urls.is_empty() {
                    if valid_urls[0].muted {
                        let mut response = String::new();

                        println!("Do you want to download the fixed playlist? (Y/n)");
                        stdin()
                            .read_line(&mut response)
                            .expect("Failed to read line.");
                        trim_newline(&mut response);

                        match response.to_lowercase().as_str() {
                            "y" | "" => {
                                match fix(valid_urls[0].playlist.as_str(), None, false, matches) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("{}", e);
                                        return;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                return;
            }
            Commands::Live { .. } => {
                let mut username = String::new();

                println!("Please enter the streamer's username:");
                stdin()
                    .read_line(&mut username)
                    .expect("Failed to read line.");
                trim_newline(&mut username);

                let valid_urls = match live(username.as_str(), matches.clone()) {
                    Ok(u) => match u {
                        Some(u) => u,
                        None => Vec::new(),
                    },
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };
                if !valid_urls.is_empty() {
                    if valid_urls[0].muted {
                        let mut response = String::new();

                        println!("Do you want to download the fixed playlist? (Y/n)");
                        stdin()
                            .read_line(&mut response)
                            .expect("Failed to read line.");
                        trim_newline(&mut response);

                        match response.to_lowercase().as_str() {
                            "y" | "" => {
                                match fix(valid_urls[0].playlist.as_str(), None, false, matches) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!("{}", e);
                                        return;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                return;
            }
            Commands::Clip { .. } => {
                let mut clip = String::new();

                println!("Please enter the clip's URL (twitch.tv/%username%/clip/%slug% and clips.twitch.tv/%slug% are both supported) or the slug (\"GentleAthleticWombatHoneyBadger-ohJAsKzGinIgFUx2\" for example):");
                stdin().read_line(&mut clip).expect("Failed to read line.");
                trim_newline(&mut clip);

                match find_bid_from_clip(clip, matches.clone()) {
                    Ok(r) => match r {
                        Some((username, vod)) => {
                            let url =
                                format!("https://twitchtracker.com/{}/streams/{}", username, vod);
                            let (_, data) = match derive_date_from_url(&url, matches.clone()) {
                                Ok(a) => a,
                                Err(e) => {
                                    error!("{}", e);
                                    return;
                                }
                            };

                            let valid_urls = match exact(
                                username.as_str(),
                                vod,
                                data.start_date.as_str(),
                                matches.clone(),
                            ) {
                                Ok(u) => match u {
                                    Some(u) => u,
                                    None => Vec::new(),
                                },
                                Err(e) => {
                                    error!("{}", e);
                                    return;
                                }
                            };
                            if !valid_urls.is_empty() {
                                if valid_urls[0].muted {
                                    let mut response = String::new();

                                    println!("Do you want to download the fixed playlist? (Y/n)");
                                    stdin()
                                        .read_line(&mut response)
                                        .expect("Failed to read line.");
                                    trim_newline(&mut response);

                                    match response.to_lowercase().as_str() {
                                        "y" | "" => {
                                            match fix(
                                                valid_urls[0].playlist.as_str(),
                                                None,
                                                false,
                                                matches,
                                            ) {
                                                Ok(_) => {}
                                                Err(e) => {
                                                    error!("{}", e);
                                                    return;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        None => {}
                    },
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };

                return;
            }
            Commands::Clipforce { .. } => {
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

                let vod = match vod.parse::<i64>() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };
                let start = match start.parse::<i64>() {
                    Ok(s) => s,
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };
                let end = match end.parse::<i64>() {
                    Ok(e) => e,
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };

                clip_bruteforce(vod, start, end, matches);

                return;
            }
            Commands::Fix { .. } => {
                let mut url = String::new();

                println!("Please enter Twitch VOD m3u8 playlist URL (only twitch.tv and cloudfront.net URLs are supported):");
                stdin().read_line(&mut url).expect("Failed to read line.");
                trim_newline(&mut url);

                match fix(url.as_str(), None, false, matches) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("{}", e);
                        return;
                    }
                };

                return;
            }
        },
        None => {
            error!("Couldn't select the specified mode");
            return;
        }
    })();

    any_key_to_continue("Press any key to close...");
}

fn main() {
    execute!(
        stdout(),
        SetTitle(format!("{} v{}", crate_name!(), crate_version!()))
    )
    .unwrap();

    let matches = Cli::parse();

    let mut log_level = "info";
    if matches.verbose {
        log_level = "debug";
    }

    env_logger::Builder::from_env(Env::default().filter_or(
        env_logger::DEFAULT_FILTER_ENV,
        format!("{},html5ever=info,selectors=info", log_level),
    ))
    .format_timestamp_millis()
    .init();

    // making panics look nicer
    panic::set_hook(Box::new(move |panic_info| {
        debug!("{}", panic_info);
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            error!("{}", s);
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            error!("{}", s);
        } else {
            error!("{}", panic_info);
        }
    }));

    match matches.command {
        Some(Commands::Bruteforce {
            username,
            id,
            from,
            to,
        }) => {
            let username = username.as_str();
            let initial_from_stamp = from.as_str();
            let initial_to_stamp = to.as_str();

            let matches = Cli {
                command: None,
                ..matches
            };

            match bruteforcer(username, id, initial_from_stamp, initial_to_stamp, matches) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
        }
        Some(Commands::Exact {
            username,
            id,
            stamp,
        }) => {
            let username = username.as_str();
            let initial_stamp = stamp.as_str();

            let matches = Cli {
                command: None,
                ..matches
            };

            match exact(username, id, initial_stamp, matches) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
        }
        Some(Commands::Live { username }) => {
            let username = username.as_str();

            let matches = Cli {
                command: None,
                ..matches
            };

            match live(username, matches) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
        }
        Some(Commands::Link { url }) => {
            let url = url.as_str();

            let matches = Cli {
                command: None,
                ..matches
            };

            let (proc, data) = match derive_date_from_url(&url, matches.clone()) {
                Ok(a) => a,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };

            match proc {
                ProcessingType::Exact => {
                    match exact(
                        data.username.as_str(),
                        match data.broadcast_id.parse::<i64>() {
                            Ok(b) => b,
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        },
                        data.start_date.as_str(),
                        matches.clone(),
                    ) {
                        Ok(u) => match u {
                            Some(u) => u,
                            None => Vec::new(),
                        },
                        Err(e) => {
                            error!("{}", e);
                            return;
                        }
                    }
                }
                ProcessingType::Bruteforce => {
                    let end_date = match data.end_date {
                        Some(d) => d,
                        None => {
                            error!("Couldn't get the end date for the bruteforce method");
                            return;
                        }
                    };
                    match bruteforcer(
                        data.username.as_str(),
                        match data.broadcast_id.parse::<i64>() {
                            Ok(b) => b,
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        },
                        data.start_date.as_str(),
                        end_date.as_str(),
                        matches.clone(),
                    ) {
                        Ok(u) => match u {
                            Some(u) => u,
                            None => Vec::new(),
                        },
                        Err(e) => {
                            error!("{}", e);
                            return;
                        }
                    }
                }
            };
        }
        Some(Commands::Clip { clip }) => {
            let matches = Cli {
                command: None,
                ..matches
            };

            match find_bid_from_clip(clip, matches.clone()) {
                Ok(r) => match r {
                    Some((username, vod)) => {
                        let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
                        let (_, data) = match derive_date_from_url(&url, matches.clone()) {
                            Ok(a) => a,
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        };

                        match exact(&username, vod, &data.start_date, matches) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        };
                    }
                    None => {}
                },
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            }
        }
        Some(Commands::Clipforce { id, start, end }) => {
            clip_bruteforce(id, start, end, matches);
        }
        Some(Commands::Fix { url, output, slow }) => {
            let matches = Cli {
                command: None,
                ..matches
            };

            match fix(url.as_str(), output, slow, matches) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            }
        }
        _ => interface(matches),
    }
}
