# spotify-oauth
[![Docs](https://docs.rs/spotify-oauth/badge.svg)](https://docs.rs/spotify-oauth/)

## Description
spotify-oauth is a library for [Spotify Authorization](https://developer.spotify.com/documentation/general/guides/authorization-guide/).
It features a full implementation of the Authorization Code Flow that Spotify requires a user to undergo before using the web API.

## Basic Example
This example shows how the library can be used to create a full authorization flow for retrieving the token required to use the web API.
```rust
use std::{io::stdin, str::FromStr, error::Error};
use spotify_oauth::{SpotifyAuth, SpotifyCallback, SpotifyScope};

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {

    // Setup Spotify Auth URL
    let auth = SpotifyAuth::new_from_env("code".into(), vec![SpotifyScope::Streaming], false);
    let auth_url = auth.authorize_url()?;

    // Open the auth URL in the default browser of the user.
    open::that(auth_url)?;

    println!("Input callback URL:");
    let mut buffer = String::new();
    stdin().read_line(&mut buffer)?;

    // Convert the given callback URL into a token.
    let token = SpotifyCallback::from_str(buffer.trim())?
        .convert_into_token(auth.client_id, auth.client_secret, auth.redirect_uri).await?;

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

