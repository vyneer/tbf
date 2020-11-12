use error_chain::error_chain;
use log::{info, debug};
use clap::{load_yaml, crate_authors, crate_description, crate_version, App};
use rayon::prelude::*;
use env_logger::Env;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use std::sync::{Arc, Mutex};

error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
    }
}

fn main() -> Result<()> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml)
        .version(crate_version!())
        .about(crate_description!())
        .author(crate_authors!())
        .get_matches();

    let mut log_level = "info";
    if matches.is_present("verbose") {
        log_level = "debug";
    }

    env_logger::init_from_env(
        Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, log_level));

    let streamer = matches.value_of("streamer").unwrap();
    let vod = matches.value_of("id").unwrap().parse::<i32>().unwrap();
    let number1 = matches.value_of("from").unwrap().parse::<i32>().unwrap();
    let number2 = matches.value_of("to").unwrap().parse::<i32>().unwrap();
    let final_url_atomic = Arc::new(Mutex::new(String::new()));
    let mut initial_url_vec: Vec<String> = Vec::new();
    let client = reqwest::blocking::Client::new();
    for number in number1..number2 {
        let mut hasher = Sha1::new();
        hasher.input_str(format!("{}_{}_{}", streamer, vod, number).as_str());
        let hex = hasher.result_str();
        initial_url_vec.push(format!("https://vod-secure.twitch.tv/{}_{}_{}_{}/chunked/index-dvr.m3u8", &hex[0..20], streamer, vod, number));
    }
    debug!("Finished making urls.");
    initial_url_vec.par_iter().for_each( |url| {
        let final_url_atomic = Arc::clone(&final_url_atomic);
        let res = client.get(&url.clone()).send().expect("Error");
        if res.status() == 200 {
            let mut final_url = final_url_atomic.lock().unwrap();
            *final_url = url.to_string();
            debug!("Got it! - {:?}", url);
        } else {
            debug!("Still going - {:?}", url);
        }
    });

    if *final_url_atomic.lock().unwrap() != "" {
        info!("Got the url! - {}", *final_url_atomic.lock().unwrap())
    } else {
        info!("Couldn't find anything :(")
    }
    
    Ok(())
}