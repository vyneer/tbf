mod config;
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
    vods::{bruteforcer, exact},
};
use util::{any_key_to_continue, derive_date_from_url, trim_newline};

lazy_static! {
    // HTTP client to share
    static ref HTTP_CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::new();
}

fn interface(matches: Cli) {
    let mut mode = String::new();

    println!("Please select the mode you want:");
    println!("[1] Exact mode - Combine all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL and check whether the VOD is available");
    println!("[2] Bruteforce mode - Go over a range of timestamps, looking for a usable/working m3u8 URL, and check whether the VOD is available");
    println!("[3] TwitchTracker mode - Get the m3u8 from a TwitchTracker URL");
    println!("[4] Clip mode - Get the m3u8 from a clip using TwitchTracker");
    println!(
        "[5] Clip bruteforce mode - Go over a range of timestamps, looking for clips in a VOD"
    );

    stdin().read_line(&mut mode).expect("Failed to read line.");
    trim_newline(&mut mode);

    match mode.as_str() {
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

            exact(
                username.as_str(),
                vod.parse::<i64>().unwrap(),
                initial_stamp.as_str(),
                Flags {
                    verbose: false,
                    simple: false,
                    pbar: true,
                    cdnfile: matches.cdnfile,
                },
            );
            any_key_to_continue("Press any key to close...");
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

            bruteforcer(
                username.as_str(),
                vod.parse::<i64>().unwrap(),
                initial_from_stamp.as_str(),
                initial_to_stamp.as_str(),
                Flags {
                    verbose: false,
                    simple: false,
                    pbar: true,
                    cdnfile: matches.cdnfile,
                },
            );
            any_key_to_continue("Press any key to close...");
        }
        "3" => {
            let mut url = String::new();

            println!("Please enter the TwitchTracker URL:");
            stdin().read_line(&mut url).expect("Failed to read line.");
            trim_newline(&mut url);

            let (username, vod, initial_stamp) = derive_date_from_url(&url);

            exact(
                username.as_str(),
                vod.parse::<i64>().unwrap(),
                initial_stamp.as_str(),
                Flags {
                    verbose: false,
                    simple: false,
                    pbar: true,
                    cdnfile: matches.cdnfile,
                },
            );
            any_key_to_continue("Press any key to close...");
        }
        "4" => {
            let mut slug = String::new();

            println!("Please enter the clip's slug (that's the EncouragingTallDragonSpicyBoy or w.e. part):");
            stdin().read_line(&mut slug).expect("Failed to read line.");
            trim_newline(&mut slug);

            let (username, vod) = find_bid_from_clip(slug);
            let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
            let (_, _, initial_stamp) = derive_date_from_url(&url);

            exact(
                username.as_str(),
                vod,
                initial_stamp.as_str(),
                Flags {
                    verbose: false,
                    simple: false,
                    pbar: false,
                    cdnfile: matches.cdnfile,
                },
            );
            any_key_to_continue("Press any key to close...");
        }
        "5" => {
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

            let vod = vod.parse::<i64>().unwrap();
            let start = start.parse::<i64>().unwrap();
            let end = end.parse::<i64>().unwrap();

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
            any_key_to_continue("Press any key to close...");
        }
        _ => {}
    }
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

    env_logger::Builder::from_env(
        Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_level),
    )
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

            bruteforcer(username, id, initial_from_stamp, initial_to_stamp, flags);
        }
        Some(Commands::Exact {
            progressbar,
            username,
            id,
            stamp,
        }) => {
            let username = username.as_str();
            let initial_stamp = stamp.as_str();

            exact(
                username,
                id,
                initial_stamp,
                Flags {
                    verbose: matches.verbose,
                    simple: matches.simple,
                    pbar: progressbar,
                    cdnfile: matches.cdnfile,
                },
            );
        }
        Some(Commands::Link { progressbar, url }) => {
            let url = url.as_str();
            let (username, vod, initial_stamp) = derive_date_from_url(&url);

            exact(
                &username,
                vod.parse::<i64>().unwrap(),
                &initial_stamp,
                Flags {
                    verbose: matches.verbose,
                    simple: matches.simple,
                    pbar: progressbar,
                    cdnfile: matches.cdnfile,
                },
            );
        }
        Some(Commands::Clip { progressbar, slug }) => {
            let slug = slug.as_str();
            let (username, vod) = find_bid_from_clip(slug.to_string());
            let url = format!("https://twitchtracker.com/{}/streams/{}", username, vod);
            let (_, _, initial_stamp) = derive_date_from_url(&url);

            exact(
                &username,
                vod,
                &initial_stamp,
                Flags {
                    verbose: matches.verbose,
                    simple: matches.simple,
                    pbar: progressbar,
                    cdnfile: matches.cdnfile,
                },
            );
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
        _ => interface(matches),
    }
}
