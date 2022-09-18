use anyhow::Result;
use clap::crate_version;
use serde::Deserialize;
use semver::Version;
use reqwest::header::USER_AGENT;
use guess_host_triple::guess_host_triple;

use crate::config::{Cli, CURL_UA};

#[derive(Debug, Deserialize)]
struct GithubUpdate {
	tag_name: String,
	assets: Vec<GithubAssets>,
}

#[derive(Debug, Deserialize)]
struct GithubAssets {
	browser_download_url: String,
}

pub fn update(matches: Cli) -> Result<()> {
	let target_triple = guess_host_triple();
    let current_version = crate_version!();
	let cur_version_parsed = Version::parse(current_version).unwrap();

    let resp = crate::HTTP_CLIENT
		.get("https://api.github.com/repos/vyneer/tbf/releases/latest")
		.header(USER_AGENT, CURL_UA)
		.send();

	let mut gh = match resp {
		Ok(r) => {
			match r.status().is_success() {
				true => {
					let gh: GithubUpdate = match r.json() {
						Ok(v) => v,
						Err(e) => return Err(e)?,
					};

					gh
				}
				false => GithubUpdate { tag_name: "".to_string(), assets: vec![] }
			}
		},
		Err(e) => return Err(e)?
	};

	if gh.tag_name != "" && !gh.assets.is_empty() {
		gh.tag_name.remove(0);
		let new_version_parsed = Version::parse(&gh.tag_name).unwrap();

		if new_version_parsed > cur_version_parsed {
			if !matches.simple {
				println!("New version available ({}):", gh.tag_name);
			}
			for url in gh.assets {
				match target_triple {
					Some(triple) => {
						if url.browser_download_url.contains(triple) {
							println!("{}", url.browser_download_url)
						}
					}
					None => println!("{}", url.browser_download_url)
				}
			}
		} else {
			if !matches.simple {
				println!("No updates available");
			}
		}
	}

    Ok(())
}
