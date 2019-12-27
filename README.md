# spotify-oauth
[![Docs](https://docs.rs/spotify-oauth/badge.svg)](https://docs.rs/spotify-oauth/)

## Description
spotify-oauth is a library for [Spotify Authorization](https://developer.spotify.com/documentation/general/guides/authorization-guide/).
It features a full implementation of the Authorization Code Flow that Spotify requires a user to undergo before using the web API.

## Basic Example
This example shows how the library can be used to create a full authorization flow for retrieving the token required to use the web API.
```rust
use std::io::stdin;
use std::str::FromStr;
use spotify_oauth::{SpotifyAuth, SpotifyCallback};

fn main() -> Result<(), Box<std::error::Error>> {

    // Setup Spotify Auth URL
    let auth_url = SpotifyAuth::default().authorize_url()?;

    // Open the auth URL in the default browser of the user.
    open::that(auth_url)?;

    println!("Input callback URL:");
    let mut buffer = String::new();
    stdin().read_line(&mut buffer)?;

    let token = SpotifyCallback::from_str(buffer.trim())?.convert_into_token()?;

    println!("Token: {:#?}", token);

    Ok(())
}
```

### API Documentation
More API information can be located [here](https://docs.rs/spotify-oauth/).

### Contribution
If you have any suggestions or issues towards this library, please submit an
issue. Pull requests, code reviewing and feedback are welcome.

### License
[MIT](LICENSE)

