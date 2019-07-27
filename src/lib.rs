//! # Spotify OAuth
//!
//! An implementation of the Spotify Authorization Code Flow in Rust.
//!
//! # Basic Example
//!
//! ```no_run
//! use std::io::stdin;
//! use std::str::FromStr;
//! use spotify_oauth::{SpotifyAuth, SpotifyCallback, SpotifyScope};
//!
//! fn main() -> Result<(), Box<std::error::Error>> {
//!
//!     // Setup Spotify Auth URL
//!     let auth = SpotifyAuth::new_from_env("code".into(), vec![SpotifyScope::Streaming], false);
//!     let auth_url = auth.authorize_url()?;
//!
//!     // Open the auth URL in the default browser of the user.
//!     open::that(auth_url)?;
//!
//!     println!("Input callback URL:");
//!     let mut buffer = String::new();
//!     stdin().read_line(&mut buffer)?;
//!
//!     let token = SpotifyCallback::from_str(buffer.trim())?.convert_into_token(auth.client_id, auth.client_secret, auth.redirect_uri)?;
//!
//!     println!("Token: {:#?}", token);
//!
//!     Ok(())
//! }
//! ```

use chrono::{DateTime, Utc};
use dotenv::dotenv;
use rand::{self, Rng};
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use strum_macros::{Display, EnumString};
use url::Url;

use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::str::FromStr;
use std::string::ToString;

mod error;
use crate::error::{ErrorKind, SpotifyError, SpotifyResult};

const SPOTIFY_AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const SPOTIFY_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Convert date and time to a unix timestamp.
///
/// # Example
///
/// ```no_run
/// // Uses elapsed seconds and the current timestamp to return a timestamp offset by the seconds.
/// use spotify_oauth::datetime_to_timestamp;
///
/// let timestamp = datetime_to_timestamp(3600);
/// ```
pub fn datetime_to_timestamp(elapsed: u32) -> i64 {
    let utc: DateTime<Utc> = Utc::now();
    utc.timestamp() + i64::from(elapsed)
}

/// Generate a random alphanumeric string with a given length.
///
/// # Example
///
/// ```no_run
/// // Uses elapsed seconds and the current timestamp to return a timestamp offset by the seconds.
/// use spotify_oauth::generate_random_string;
///
/// let timestamp = generate_random_string(20);
/// ```
pub fn generate_random_string(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(length)
        .collect()
}

/// Spotify Scopes for the API.
/// This enum implements FromStr and ToString / Display through strum.
///
/// All the Spotify API scopes can be found [here](https://developer.spotify.com/documentation/general/guides/scopes/ "Spotify Scopes").
///
/// # Example
///
/// ```
/// use spotify_oauth::SpotifyScope;
/// use std::str::FromStr;
///
/// // Convert string into scope.
/// let scope = SpotifyScope::from_str("streaming").unwrap();
/// assert_eq!(scope, SpotifyScope::Streaming);
///
/// // It can also convert the scope back into a string.
/// assert_eq!(scope.to_string(), "streaming");
///
/// // Or the enum can be used normally.
/// assert_eq!(SpotifyScope::Streaming, scope);
/// ```
#[derive(EnumString, Serialize, Deserialize, Display, Debug, Clone, PartialEq)]
pub enum SpotifyScope {
    #[strum(serialize = "user-read-recently-played")]
    UserReadRecentlyPlayed,
    #[strum(serialize = "user-top-read")]
    UserTopRead,

    #[strum(serialize = "user-library-modify")]
    UserLibraryModify,
    #[strum(serialize = "user-library-read")]
    UserLibraryRead,

    #[strum(serialize = "playlist-read-private")]
    PlaylistReadPrivate,
    #[strum(serialize = "playlist-modify-public")]
    PlaylistModifyPublic,
    #[strum(serialize = "playlist-modify-private")]
    PlaylistModifyPrivate,
    #[strum(serialize = "playlist-read-collaborative")]
    PlaylistReadCollaborative,

    #[strum(serialize = "user-read-email")]
    UserReadEmail,
    #[strum(serialize = "user-read-birthdate")]
    UserReadBirthDate,
    #[strum(serialize = "user-read-private")]
    UserReadPrivate,

    #[strum(serialize = "user-read-playback-state")]
    UserReadPlaybackState,
    #[strum(serialize = "user-modify-playback-state")]
    UserModifyPlaybackState,
    #[strum(serialize = "user-read-currently-playing")]
    UserReadCurrentlyPlaying,

    #[strum(serialize = "app-remote-control")]
    AppRemoteControl,
    #[strum(serialize = "streaming")]
    Streaming,

    #[strum(serialize = "user-follow-read")]
    UserFollowRead,
    #[strum(serialize = "user-follow-modify")]
    UserFollowModify,
}

/// Spotify Authentication
///
/// This struct follows the parameters given at [this](https://developer.spotify.com/documentation/general/guides/authorization-guide/ "Spotify Auth Documentation") link.
///
/// # Example
///
/// ```no_run
/// use spotify_oauth::{SpotifyAuth, SpotifyScope};
///
/// // Create a new spotify auth object with the scope "Streaming" using the ``new_from_env`` function.
/// // This object can then be converted into the auth url needed to gain a callback for the token.
/// let auth = SpotifyAuth::new_from_env("code".into(), vec![SpotifyScope::Streaming], false);
/// ```
pub struct SpotifyAuth {
    /// The Spotify Application Client ID
    pub client_id: String,
    /// The Spotify Application Client Secret
    pub client_secret: String,
    /// Required by the Spotify API.
    pub response_type: String,
    /// The URI to redirect to after the user grants or denies permission.
    pub redirect_uri: Url,
    /// A random generated string that can be useful for correlating requests and responses.
    pub state: String,
    /// Vec of Spotify Scopes.
    pub scope: Vec<SpotifyScope>,
    /// Whether or not to force the user to approve the app again if they’ve already done so.
    pub show_dialog: bool,
}

/// Implementation of Default for SpotifyAuth.
///
/// If ``CLIENT_ID`` is not found in the ``.env`` in the project directory it will default to ``INVALID_ID``.
/// If ``REDIRECT_ID`` is not found in the ``.env`` in the project directory it will default to ``http://localhost:8000/callback``.
///
/// This implementation automatically generates a state value of length 20 using a random string generator.
///
impl Default for SpotifyAuth {
    fn default() -> Self {
        // Load local .env file.
        dotenv().ok();

        Self {
            client_id: match env::var("SPOTIFY_CLIENT_ID") {
                Ok(x) => x,
                Err(_) => "INVALID_ID".to_string(),
            },
            client_secret: env::var("SPOTIFY_CLIENT_SECRET")
                .map_err(|e| {
                    SpotifyError::new(ErrorKind::ParsingFailed)
                        .set_cause(e)
                        .set_context("Spotify Client Redirect URI failed to parse into a URL.")
                })
                .unwrap(),
            response_type: "code".to_owned(),
            redirect_uri: Url::parse(
                &env::var("REDIRECT_URI")
                    .unwrap_or_else(|_| "http://localhost:8000/callback".to_string()),
            )
            .unwrap(),
            state: generate_random_string(20),
            scope: vec![],
            show_dialog: false,
        }
    }
}

/// Conversion and helper functions for SpotifyAuth.
impl SpotifyAuth {
    /// Generate a new SpotifyAuth structure from values in memory.
    ///
    /// This function loads ``SPOTIFY_CLIENT_ID`` and ``SPOTIFY_REDIRECT_ID`` from values given in
    /// function parameters.
    ///
    /// This function also automatically generates a state value of length 20 using a random string generator.
    ///
    /// # Example
    ///
    /// ```
    /// use spotify_oauth::{SpotifyAuth, SpotifyScope};
    ///
    /// // SpotifyAuth with the scope "Streaming".
    /// let auth = SpotifyAuth::new("00000000000".into(), "secret".into(), "code".into(), "http://localhost:8000/callback".into(), vec![SpotifyScope::Streaming], false);
    ///
    /// assert_eq!(auth.scope_into_string(), "streaming");
    /// ```
    pub fn new(
        client_id: String,
        client_secret: String,
        response_type: String,
        redirect_uri: String,
        scope: Vec<SpotifyScope>,
        show_dialog: bool,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            response_type,
            redirect_uri: Url::parse(&redirect_uri)
                .map_err(|e| {
                    SpotifyError::new(ErrorKind::ParsingFailed)
                        .set_cause(e)
                        .set_context("Spotify Client Redirect URI failed to parse into a URL.")
                })
                .unwrap(),
            state: generate_random_string(20),
            scope,
            show_dialog,
        }
    }

    /// Generate a new SpotifyAuth structure from values in the environment.
    ///
    /// This function loads ``SPOTIFY_CLIENT_ID`` and ``SPOTIFY_REDIRECT_ID`` from the environment.
    ///
    /// This function also automatically generates a state value of length 20 using a random string generator.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use spotify_oauth::{SpotifyAuth, SpotifyScope};
    ///
    /// // SpotifyAuth with the scope "Streaming".
    /// let auth = SpotifyAuth::new_from_env("code".into(), vec![SpotifyScope::Streaming], false);
    ///
    /// assert_eq!(auth.scope_into_string(), "streaming");
    /// ```
    pub fn new_from_env(
        response_type: String,
        scope: Vec<SpotifyScope>,
        show_dialog: bool,
    ) -> Self {
        // Load local .env file.
        dotenv().ok();

        Self {
            client_id: env::var("SPOTIFY_CLIENT_ID")
                .map_err(|e| {
                    SpotifyError::new(ErrorKind::ParsingFailed)
                        .set_cause(e)
                        .set_context("Spotify Client ID failed to load from the environment.")
                })
                .unwrap(),
            client_secret: env::var("SPOTIFY_CLIENT_SECRET")
                .map_err(|e| {
                    SpotifyError::new(ErrorKind::ParsingFailed)
                        .set_cause(e)
                        .set_context("Spotify Client Secre failed to load from the environment.")
                })
                .unwrap(),
            response_type,
            redirect_uri: Url::parse(
                &env::var("SPOTIFY_REDIRECT_URI")
                    .map_err(|e| {
                        SpotifyError::new(ErrorKind::ParsingFailed)
                            .set_cause(e)
                            .set_context(
                                "Spotify Client Redirect URL failed to load from the environment.",
                            )
                    })
                    .unwrap(),
            )
            .unwrap(),
            state: generate_random_string(20),
            scope,
            show_dialog,
        }
    }

    /// Concatenate the scope vector into a string needed for the authorization URL.
    ///
    /// # Example
    ///
    /// ```
    /// use spotify_oauth::{SpotifyAuth, SpotifyScope};
    ///
    /// // Default SpotifyAuth with the scope "Streaming".
    /// let auth = SpotifyAuth::new("00000000000".into(), "secret".into(), "code".into(), "http://localhost:8000/callback".into(), vec![SpotifyScope::Streaming], false);
    ///
    /// assert_eq!(auth.scope_into_string(), "streaming");
    /// ```
    pub fn scope_into_string(&self) -> String {
        self.scope
            .iter()
            .map(|x| x.clone().to_string())
            .collect::<Vec<String>>()
            .join(" ")
    }

    /// Convert the SpotifyAuth struct into the authorization URL.
    ///
    /// More information on this URL can be found [here](https://developer.spotify.com/documentation/general/guides/authorization-guide/ "Spotify Auth Documentation").
    ///
    /// # Example
    ///
    /// ```
    /// use spotify_oauth::{SpotifyAuth, SpotifyScope};
    ///
    /// // Default SpotifyAuth with the scope "Streaming" converted into the authorization URL.
    /// let auth = SpotifyAuth::new("00000000000".into(), "secret".into(), "code".into(), "http://localhost:8000/callback".into(), vec![SpotifyScope::Streaming], false)
    ///     .authorize_url().unwrap();
    /// ```
    pub fn authorize_url(&self) -> SpotifyResult<String> {
        let mut url = Url::parse(SPOTIFY_AUTH_URL).map_err(|e| {
            SpotifyError::new(ErrorKind::ParsingFailed)
                .set_cause(e)
                .set_context("Spotify Auth URL failed to parse.")
        })?;

        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", &self.response_type)
            .append_pair("redirect_uri", self.redirect_uri.as_str())
            .append_pair("state", &self.state)
            .append_pair("scope", &self.scope_into_string())
            .append_pair("show_dialog", &self.show_dialog.to_string());

        Ok(url.to_string())
    }
}

/// The Spotify Callback URL
///
/// This struct follows the parameters given at [this](https://developer.spotify.com/documentation/general/guides/authorization-guide/ "Spotify Auth Documentation") link.
///
/// The main use of this object is to convert the callback URL into an object that can be used to generate a token.
/// If needed you can also create this callback object using the ``new`` function in the struct.
///
/// # Example
///
/// ```
/// use spotify_oauth::SpotifyCallback;
/// use std::str::FromStr;
///
/// // Create a new spotify callback object using the callback url given by the authorization process.
/// // This object can then be converted into the token needed for the application.
/// let callback = SpotifyCallback::from_str("https://example.com/callback?code=NApCCgBkWtQ&state=test").unwrap();
///
/// assert_eq!(callback, SpotifyCallback::new(Some("NApCCgBkWtQ".to_string()), None, String::from("test")));
/// ```
#[derive(Debug, PartialEq)]
pub struct SpotifyCallback {
    /// An authorization code that can be exchanged for an access token.
    code: Option<String>,
    /// The reason authorization failed.
    error: Option<String>,
    /// The value of the ``state`` parameter supplied in the request.
    state: String,
}

/// Implementation of FromStr for Spotify Callback URLs.
///
/// # Example
///
/// ```
/// use spotify_oauth::SpotifyCallback;
/// use std::str::FromStr;
///
/// // Create a new spotify callback object using the callback url given by the authorization process.
/// // This object can then be converted into the token needed for the application.
/// let callback = SpotifyCallback::from_str("https://example.com/callback?code=NApCCgBkWtQ&state=test").unwrap();
///
/// assert_eq!(callback, SpotifyCallback::new(Some("NApCCgBkWtQ".to_string()), None, String::from("test")));
/// ```
impl FromStr for SpotifyCallback {
    type Err = error::SpotifyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s).map_err(|e| {
            SpotifyError::new(ErrorKind::ParsingFailed)
                .set_cause(e)
                .set_context("Spotify Callback URL failed to parse.")
        })?;
        let parsed: Vec<(String, String)> = url
            .query_pairs()
            .map(|x| (x.0.into_owned(), x.1.into_owned()))
            .collect();

        let has_state = parsed.iter().any(|x| x.0 == "state");
        let has_response = parsed.iter().any(|x| x.0 == "error" || x.0 == "code");

        if !has_state && !has_response {
            return Err(SpotifyError::new(ErrorKind::InvalidCallbackURL)
                .set_context("Does not contain any state or response type query parameters."));
        } else if !has_state {
            return Err(SpotifyError::new(ErrorKind::InvalidCallbackURL)
                .set_context("Does not contain any state type query parameters."));
        } else if !has_response {
            return Err(SpotifyError::new(ErrorKind::InvalidCallbackURL)
                .set_context("Does not contain any response type query parameters."));
        }

        let state = match parsed.iter().find(|x| x.0 == "state") {
            None => ("state".to_string(), "".to_string()),
            Some(x) => x.clone(),
        };

        let response = match parsed.iter().find(|x| x.0 == "error" || x.0 == "code") {
            None => ("error".to_string(), "access_denied".to_string()),
            Some(x) => x.clone(),
        };

        if response.0 == "code" {
            return Ok(Self {
                code: Some(response.to_owned().1),
                error: None,
                state: state.1,
            });
        } else if response.0 == "error" {
            return Ok(Self {
                code: None,
                error: Some(response.to_owned().1),
                state: state.1,
            });
        }

        Err(SpotifyError::new(ErrorKind::InvalidCallbackURL)
            .set_context("Does not contain any state or response type query parameters."))
    }
}

/// Conversion and helper functions for SpotifyCallback.
impl SpotifyCallback {
    /// Create a new Spotify Callback object with given values.
    ///
    /// # Example
    ///
    /// ```
    /// use spotify_oauth::SpotifyCallback;
    ///
    /// // Create a new spotify callback object using the new function.
    /// // This object can then be converted into the token needed for the application.
    /// let callback = SpotifyCallback::new(Some("NApCCgBkWtQ".to_string()), None, String::from("test"));
    /// ```
    pub fn new(code: Option<String>, error: Option<String>, state: String) -> Self {
        Self { code, error, state }
    }

    /// Converts the Spotify Callback object into a Spotify Token object.
    ///
    /// # Example
    ///
    /// ```
    /// use spotify_oauth::{SpotifyAuth, SpotifyCallback, SpotifyScope};
    /// use std::str::FromStr;
    ///
    /// // Create a new Spotify auth object.
    /// let auth = SpotifyAuth::new("00000000000".into(), "secret".into(), "code".into(), "http://localhost:8000/callback".into(), vec![SpotifyScope::Streaming], false);
    ///
    /// // Create a new spotify callback object using the callback url given by the authorization process and convert it into a token.
    /// let token = SpotifyCallback::from_str("https://example.com/callback?code=NApCCgBkWtQ&state=test").unwrap()
    ///     .convert_into_token(auth.client_id, auth.client_secret, auth.redirect_uri);
    /// ```
    pub fn convert_into_token(
        self,
        client_id: String,
        client_secret: String,
        redirect_uri: Url,
    ) -> SpotifyResult<SpotifyToken> {
        let client = Client::new();
        let mut payload: HashMap<String, String> = HashMap::new();
        payload.insert("grant_type".to_owned(), "authorization_code".to_owned());
        payload.insert(
            "code".to_owned(),
            match self.code {
                None => {
                    return Err(SpotifyError::new(ErrorKind::ParsingFailed)
                        .set_context("Spotify Callback Code failed to parse."))
                }
                Some(x) => x,
            },
        );
        payload.insert("redirect_uri".to_owned(), redirect_uri.to_string());

        let mut response = client
            .post(SPOTIFY_TOKEN_URL)
            .basic_auth(client_id, Some(client_secret))
            .form(&payload)
            .send()
            .map_err(|e| {
                SpotifyError::new(ErrorKind::RequestFailed)
                    .set_cause(e)
                    .set_context("Spotify Auth Request failed.")
            })?;

        let mut buf = String::new();
        response.read_to_string(&mut buf).map_err(|e| {
            SpotifyError::new(ErrorKind::ParsingFailed)
                .set_cause(e)
                .set_context("Failed to read the response into the string buffer.")
        })?;

        if response.status().is_success() {
            let mut token: SpotifyToken = serde_json::from_str(&buf).map_err(|e| {
                SpotifyError::new(ErrorKind::ParsingFailed)
                    .set_cause(e)
                    .set_context("Spotify Auth JSON Response failed to be parsed.")
            })?;
            token.expires_at = Some(datetime_to_timestamp(token.expires_in));

            return Ok(token);
        }

        Err(SpotifyError::new(ErrorKind::ParsingFailed)
            .set_context("Failed to convert callback into token."))
    }
}

/// The Spotify Token object.
///
/// This struct follows the parameters given at [this](https://developer.spotify.com/documentation/general/guides/authorization-guide/ "Spotify Auth Documentation") link.
///
/// This object can only be formed from a correct Spotify Callback object.
///
/// # Example
///
/// ```
/// use spotify_oauth::{SpotifyAuth, SpotifyScope, SpotifyCallback};
/// use std::str::FromStr;
///
/// // Create a new Spotify auth object.
/// let auth = SpotifyAuth::new("00000000000".into(), "secret".into(), "code".into(), "http://localhost:8000/callback".into(), vec![SpotifyScope::Streaming], false);   
///
/// // Create a new Spotify token object using the callback object given by the authorization process.
/// let token = SpotifyCallback::from_str("https://example.com/callback?code=NApCCgBkWtQ&state=test").unwrap()
///     .convert_into_token(auth.client_id, auth.client_secret, auth.redirect_uri);
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SpotifyToken {
    /// An access token that can be provided in subsequent calls, for example to Spotify Web API services.
    pub access_token: String,
    /// How the access token may be used.
    pub token_type: String,
    /// A Vec of scopes which have been granted for this ``access_token``.
    #[serde(deserialize_with = "deserialize_scope_field")]
    pub scope: Vec<SpotifyScope>,
    /// The time period (in seconds) for which the access token is valid.
    pub expires_in: u32,
    /// The timestamp for which the token will expire at.
    pub expires_at: Option<i64>,
    /// A token that can be sent to the Spotify Accounts service in place of an authorization code to request a new ``access_token``.
    pub refresh_token: String,
}

/// Custom parsing function for converting a vector of string scopes into SpotifyScope Enums using Serde.
/// If scope is empty it will return an empty vector.
fn deserialize_scope_field<'de, D>(de: D) -> Result<Vec<SpotifyScope>, D::Error>
where
    D: Deserializer<'de>,
{
    let result: Value = Deserialize::deserialize(de)?;
    match result {
        Value::String(ref s) => {
            let split: Vec<&str> = s.split_whitespace().collect();
            let mut parsed: Vec<SpotifyScope> = Vec::new();

            for x in split {
                parsed.push(SpotifyScope::from_str(x).unwrap());
            }

            Ok(parsed)
        }
        _ => Ok(vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Callback Testing

    #[test]
    fn test_parse_callback_code() {
        let url = String::from("http://localhost:8888/callback?code=AQD0yXvFEOvw&state=sN");

        assert_eq!(
            SpotifyCallback::from_str(&url).unwrap(),
            SpotifyCallback::new(Some("AQD0yXvFEOvw".to_string()), None, "sN".to_string())
        );
    }

    #[test]
    fn test_parse_callback_error() {
        let url = String::from("http://localhost:8888/callback?error=access_denied&state=sN");

        assert_eq!(
            SpotifyCallback::from_str(&url).unwrap(),
            SpotifyCallback::new(None, Some("access_denied".to_string()), "sN".to_string())
        );
    }

    #[test]
    fn test_invalid_response_parse() {
        let url = String::from("http://localhost:8888/callback?state=sN");

        assert_eq!(
            SpotifyCallback::from_str(&url).unwrap_err().kind(),
            SpotifyError::new(ErrorKind::InvalidCallbackURL).kind()
        );
    }

    #[test]
    fn test_invalid_parse() {
        let url = String::from("http://localhost:8888/callback");

        assert_eq!(
            SpotifyCallback::from_str(&url).unwrap_err().kind(),
            SpotifyError::new(ErrorKind::InvalidCallbackURL).kind()
        );
    }

    // Token Testing

    #[test]
    fn test_token_parse() {
        let token_json = r#"{
           "access_token": "NgCXRKDjGUSKlfJODUjvnSUhcOMzYjw",
           "token_type": "Bearer",
           "scope": "user-read-private user-read-email",
           "expires_in": 3600,
           "refresh_token": "NgAagAHfVxDkSvCUm_SHo"
        }"#;

        let mut token: SpotifyToken = serde_json::from_str(token_json).unwrap();
        let timestamp = datetime_to_timestamp(token.expires_in);
        token.expires_at = Some(timestamp);

        assert_eq!(
            SpotifyToken {
                access_token: "NgCXRKDjGUSKlfJODUjvnSUhcOMzYjw".to_string(),
                token_type: "Bearer".to_string(),
                scope: vec![SpotifyScope::UserReadPrivate, SpotifyScope::UserReadEmail],
                expires_in: 3600,
                expires_at: Some(timestamp),
                refresh_token: "NgAagAHfVxDkSvCUm_SHo".to_string()
            },
            token
        );
    }
}
