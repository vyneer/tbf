# tbf
Finds VOD playlists on Twitch.

## How to install

```cargo install --git https://github.com/vyneer/tbf```

## Subcommands

### None

```tbf```

Will ask you what mode you want and proceed from there.

### exact

```tbf exact [FLAGS] <username> <id> <stamp>```

```tbf exact destiny 39700667438 1605781794```

Combines all the parts (streamer's username, VOD/broadcast ID and a timestamp) into a proper m3u8 URL and checks whether the VOD is available.

### bruteforce

```tbf bruteforce [FLAGS] <username> <id> <from> <to>```

```tbf bruteforce destiny 39700667438 1605781694 1605781894```

Goes over a range of timestamps, looking for a usable/working m3u8 URL and checks whether the VOD is available.

### link

```tbf link [FLAGS] <url>```

```tbf link https://twitchtracker.com/destiny/streams/41402441870```

The same as the Exact mode, but gets all the info from a TwitchTracker URL.

### clip

```tbf clip [FLAGS] <slug>```

```tbf clip SmokyHomelyIguanaAllenHuhu```

Gets the m3u8 from a clip with TwitchTracker's help.

## Flags

### -h, --help

Prints help information.

### -v, --verbose

Self-explainatory.
