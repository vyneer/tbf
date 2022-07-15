use cssparser::ParseError;
use reqwest::header::{InvalidHeaderName, InvalidHeaderValue};
use selectors::parser::SelectorParseErrorKind;
use std::{error::Error, fmt::Display, num::ParseIntError};
use time::error::Parse;
use url::ParseError as UrlPError;

#[derive(Debug)]
pub enum PlaylistFixError {
    ReqwestError(reqwest::Error),
    IoError(std::io::Error),
    URLError,
}

impl From<reqwest::Error> for PlaylistFixError {
    fn from(e: reqwest::Error) -> Self {
        Self::ReqwestError(e)
    }
}

impl From<std::io::Error> for PlaylistFixError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl Display for PlaylistFixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReqwestError(e) => write!(f, "couldn't process the url: {}", e),
            Self::IoError(e) => write!(f, "io error: {}", e),
            Self::URLError => write!(f, "only twitch.tv and cloudfront.net URLs are supported"),
        }
    }
}

impl Error for PlaylistFixError {}

#[derive(Debug)]
pub enum TimestampParserError {
    IntegerParseError(ParseIntError),
    StringParseError(Parse),
}

impl From<ParseIntError> for TimestampParserError {
    fn from(e: ParseIntError) -> Self {
        Self::IntegerParseError(e)
    }
}

impl From<Parse> for TimestampParserError {
    fn from(e: Parse) -> Self {
        Self::StringParseError(e)
    }
}

impl Display for TimestampParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntegerParseError(e) => write!(f, "couldn't parse the unix timestamp: {}", e),
            Self::StringParseError(e) => write!(f, "couldn't parse the string timestamp: {}", e),
        }
    }
}

impl Error for TimestampParserError {}

#[derive(Debug)]
pub enum DeriveDateError {
    SegmentMapError,
    ScraperElementError,
    ScraperAttributeError,
    SelectorError,
    TimestampParserError(TimestampParserError),
    UrlProcessError(reqwest::Error),
    UrlParseError(UrlPError),
    WrongURLError(String),
}

impl<'a> From<ParseError<'a, SelectorParseErrorKind<'a>>> for DeriveDateError {
    fn from(_: ParseError<'a, SelectorParseErrorKind<'a>>) -> Self {
        Self::SelectorError
    }
}

impl From<TimestampParserError> for DeriveDateError {
    fn from(e: TimestampParserError) -> Self {
        Self::TimestampParserError(e)
    }
}

impl From<UrlPError> for DeriveDateError {
    fn from(e: UrlPError) -> Self {
        Self::UrlParseError(e)
    }
}

impl From<reqwest::Error> for DeriveDateError {
    fn from(e: reqwest::Error) -> Self {
        Self::UrlProcessError(e)
    }
}

impl Display for DeriveDateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SegmentMapError => write!(f, "couldn't map the URL segments"),
            Self::ScraperElementError => write!(f, "couldn't find the nth html element"),
            Self::ScraperAttributeError => write!(f, "couldn't find the html attribute"),
            Self::SelectorError => write!(f, "couldn't parse the selector"),
            Self::TimestampParserError(e) => write!(f, "{}", e),
            Self::UrlProcessError(e) => write!(f, "couldn't process the url: {}", e),
            Self::WrongURLError(e) => write!(f, "{}", e),
            Self::UrlParseError(e) => write!(f, "couldn't parse the url: {}", e),
        }
    }
}

impl Error for DeriveDateError {}

#[derive(Debug)]
pub enum ClipError {
    IntegerParseError(ParseIntError),
    SegmentMapError,
    HeaderNameError(InvalidHeaderName),
    HeaderValueError(InvalidHeaderValue),
    WrongURLError(String),
    UrlProcessError(reqwest::Error),
}

impl From<ParseIntError> for ClipError {
    fn from(e: ParseIntError) -> Self {
        Self::IntegerParseError(e)
    }
}

impl From<InvalidHeaderName> for ClipError {
    fn from(e: InvalidHeaderName) -> Self {
        Self::HeaderNameError(e)
    }
}

impl From<InvalidHeaderValue> for ClipError {
    fn from(e: InvalidHeaderValue) -> Self {
        Self::HeaderValueError(e)
    }
}

impl From<reqwest::Error> for ClipError {
    fn from(e: reqwest::Error) -> Self {
        Self::UrlProcessError(e)
    }
}

impl Display for ClipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntegerParseError(e) => write!(f, "couldn't parse the broadcast id: {}", e),
            Self::SegmentMapError => write!(f, "couldn't map the URL segments"),
            Self::HeaderNameError(e) => write!(f, "invalid header name: {}", e),
            Self::HeaderValueError(e) => write!(f, "invalid header value: {}", e),
            Self::WrongURLError(e) => write!(f, "{}", e),
            Self::UrlProcessError(e) => write!(f, "couldn't process the url: {}", e),
        }
    }
}

impl Error for ClipError {}
