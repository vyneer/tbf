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

use config::{Cli, Commands, Flags};
use twitch::{
    clips::{clip_bruteforce, find_bid_from_clip},
    vods::{bruteforcer, exact, fix, live},
};
use util::{any_key_to_continue, derive_date_from_url, trim_newline, ProcessingType};

lazy_static! {
    // HTTP client to share
    static ref HTTP_CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::new();
}

fn interface(matches: Cli) {
    let mut mode = String::new();

    println!("Please select the mode you want:");
    println!("[1] Exact mode - Combine all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL and check whether the VOD is available");
    println!("[2] Bruteforce mode - Go over a range of timestamps, looking for a usable/working m3u8 URL, and check whether the VOD is available");
    println!("[3] Link mode - Get the m3u8 from a TwitchTracker/StreamsCharts URL");
    println!("[4] Live mode - Get the m3u8 from a currently running stream");
    println!("[5] Clip mode - Get the m3u8 from a clip using TwitchTracker");
    println!(
        "[6] Clip bruteforce mode - Go over a range of timestamps, looking for clips in a VOD"
    );
    println!(
        "[7] Fix playlist - Download and convert an unplayable unmuted Twitch VOD playlist into a playable muted one"
    );

    stdin().read_line(&mut mode).expect("Failed to read line.");
    trim_newline(&mut mode);

    (|| match mode.as_str() {
        "1" => {
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

            let fl = Flags {
                verbose: false,
                simple: false,
                pbar: true,
                cdnfile: matches.cdnfile,
            };

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
                fl.clone(),
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
                        "y" | "" => match fix(valid_urls[0].playlist.as_str(), None, false, fl) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        },
                        _ => {}
                    }
                }
            }

            return;
        }
        "2" => {
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

            let fl = Flags {
                verbose: false,
                simple: false,
                pbar: true,
                cdnfile: matches.cdnfile,
            };

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
                fl.clone(),
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
                        "y" | "" => match fix(valid_urls[0].playlist.as_str(), None, false, fl) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        },
                        _ => {}
                    }
                }
            }

            return;
        }
        "3" => {
            let mut url = String::new();

            println!("Please enter the TwitchTracker or StreamsCharts URL:");
            stdin().read_line(&mut url).expect("Failed to read line.");
            trim_newline(&mut url);

            let (proc, data) = match derive_date_from_url(&url, None) {
                Ok(a) => a,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };

            let fl = Flags {
                verbose: false,
                simple: false,
                pbar: true,
                cdnfile: matches.cdnfile,
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
                        fl.clone(),
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
                        fl.clone(),
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
                        "y" | "" => match fix(valid_urls[0].playlist.as_str(), None, false, fl) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        },
                        _ => {}
                    }
                }
            }

            return;
        }
        "4" => {
            let mut username = String::new();

            println!("Please enter the streamer's username:");
            stdin()
                .read_line(&mut username)
                .expect("Failed to read line.");
            trim_newline(&mut username);

            let fl = Flags {
                verbose: false,
                simple: false,
                pbar: true,
                cdnfile: matches.cdnfile,
            };

            let valid_urls = match live(username.as_str(), fl.clone()) {
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
                        "y" | "" => match fix(valid_urls[0].playlist.as_str(), None, false, fl) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        },
                        _ => {}
                    }
                }
            }

            return;
        }
        "5" => {
            let mut clip = String::new();

            println!("Please enter the clip's URL (twitch.tv/%username%/clip/%slug% and clips.twitch.tv/%slug% are both supported) or the slug (\"GentleAthleticWombatHoneyBadger-ohJAsKzGinIgFUx2\" for example):");
            stdin().read_line(&mut clip).expect("Failed to read line.");
            trim_newline(&mut clip);

            let fl = Flags {
                verbose: false,
                simple: false,
                pbar: true,
                cdnfile: matches.cdnfile,
            };

            match find_bid_from_clip(clip, fl.clone()) {
                Ok(r) => match r {
                    Some((username, vod)) => {
                        let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
                        let (_, data) = match derive_date_from_url(&url, None) {
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
                            fl.clone(),
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
                                        match fix(valid_urls[0].playlist.as_str(), None, false, fl)
                                        {
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
        "6" => {
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

            clip_bruteforce(
                vod,
                start,
                end,
                Flags {
                    verbose: false,
                    simple: false,
                    pbar: true,
                    cdnfile: matches.cdnfile,
                },
            );

            return;
        }
        "7" => {
            let mut url = String::new();

            println!("Please enter Twitch VOD m3u8 playlist URL (only twitch.tv and cloudfront.net URLs are supported):");
            stdin().read_line(&mut url).expect("Failed to read line.");
            trim_newline(&mut url);

            let fl = Flags {
                verbose: false,
                simple: false,
                pbar: true,
                cdnfile: matches.cdnfile,
            };

            match fix(url.as_str(), None, false, fl) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };

            return;
        }
        _ => return,
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
            progressbar,
            username,
            id,
            from,
            to,
        }) => {
            let username = username.as_str();
            let initial_from_stamp = from.as_str();
            let initial_to_stamp = to.as_str();

            let flags = Flags {
                verbose: matches.verbose,
                simple: matches.simple,
                pbar: progressbar,
                cdnfile: matches.cdnfile,
            };

            match bruteforcer(username, id, initial_from_stamp, initial_to_stamp, flags) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
        }
        Some(Commands::Exact {
            progressbar,
            username,
            id,
            stamp,
        }) => {
            let username = username.as_str();
            let initial_stamp = stamp.as_str();

            match exact(
                username,
                id,
                initial_stamp,
                Flags {
                    verbose: matches.verbose,
                    simple: matches.simple,
                    pbar: progressbar,
                    cdnfile: matches.cdnfile,
                },
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
        }
        Some(Commands::Live {
            progressbar,
            username,
        }) => {
            let username = username.as_str();

            match live(
                username,
                Flags {
                    verbose: matches.verbose,
                    simple: matches.simple,
                    pbar: progressbar,
                    cdnfile: matches.cdnfile,
                },
            ) {
                Ok(_) => {}
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
        }
        Some(Commands::Link { progressbar, url }) => {
            let url = url.as_str();
            let (proc, data) = match derive_date_from_url(&url, None) {
                Ok(a) => a,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };

            let fl = Flags {
                verbose: matches.verbose,
                simple: matches.simple,
                pbar: progressbar,
                cdnfile: matches.cdnfile,
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
                        fl.clone(),
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
                        fl.clone(),
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
        Some(Commands::Clip { progressbar, clip }) => {
            let fl = Flags {
                verbose: matches.verbose,
                simple: matches.simple,
                pbar: progressbar,
                cdnfile: matches.cdnfile,
            };

            match find_bid_from_clip(clip, fl.clone()) {
                Ok(r) => match r {
                    Some((username, vod)) => {
                        let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
                        let (_, data) = match derive_date_from_url(&url, None) {
                            Ok(a) => a,
                            Err(e) => {
                                error!("{}", e);
                                return;
                            }
                        };

                        match exact(&username, vod, &data.start_date, fl) {
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
        Some(Commands::Clipforce {
            progressbar,
            id,
            start,
            end,
        }) => {
            clip_bruteforce(
                id,
                start,
                end,
                Flags {
                    verbose: matches.verbose,
                    simple: matches.simple,
                    pbar: progressbar,
                    cdnfile: matches.cdnfile,
                },
            );
        }
        Some(Commands::Fix {
            url,
            output,
            slow,
            progressbar,
        }) => match fix(
            url.as_str(),
            output,
            slow,
            Flags {
                verbose: matches.verbose,
                simple: matches.simple,
                pbar: progressbar,
                cdnfile: matches.cdnfile,
            },
        ) {
            Ok(_) => {}
            Err(e) => {
                error!("{}", e);
                return;
            }
        },
        _ => interface(matches),
    }
}
