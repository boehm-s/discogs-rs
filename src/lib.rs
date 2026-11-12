//! Rust client for the [Discogs API](https://www.discogs.com/developers).
//!
//! # Authentication
//!
//! Discogs requires a unique `User-Agent` for every application, passed to [`Client::builder`](client::Client::builder).
//! Then pick one of the authentication methods:
//!
//! ## Personal access token
//!
//! For scripts acting on your own account. Generate the token at
//! <https://www.discogs.com/settings/developers>.
//!
//! ```no_run
//! # async fn example() -> discogs_client::error::Result<()> {
//! use discogs_client::client::Client;
//!
//! let client = Client::builder("MyCollectionApp/1.0 +https://example.com")
//!   .personal_token("my-personal-access-token")
//!   .build()?;
//! let me = client.identity().await?;
//! println!("Authenticated as {}", me.username);
//! # Ok(())
//! # }
//! ```
//!
//! ## OAuth
//!
//! For applications acting on behalf of other users. Register an application at
//! <https://www.discogs.com/settings/developers> to get consumer credentials.
//!
//! ```no_run
//! # async fn example() -> discogs_client::error::Result<()> {
//! use discogs_client::auth::client::OAuthFlow;
//! use discogs_client::auth::model::credentials::ConsumerCredentials;
//! use discogs_client::client::Client;
//!
//! let user_agent = "MyCollectionApp/1.0 +https://example.com";
//! let consumer = ConsumerCredentials::new("consumer-key", "consumer-secret");
//!
//! let flow = OAuthFlow::new(user_agent, consumer.clone())?;
//! let request_token = flow.request_token(Some("https://example.com/discogs/callback")).await?;
//! // Send the user to this URL. Discogs then redirects to the callback with `oauth_verifier`.
//! println!("{}", request_token.authorize_url());
//! # let verifier = "";
//! let access_token = flow.access_token(&request_token, verifier).await?;
//! // Store `access_token` (it is serializable) to skip the flow next time.
//!
//! let client = Client::builder(user_agent).oauth(consumer, access_token).build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Application only
//!
//! Consumer key and secret without user: higher rate limit and image URLs, but no user data.
//!
//! ```no_run
//! # fn example() -> discogs_client::error::Result<()> {
//! use discogs_client::client::Client;
//! use discogs_client::auth::model::credentials::ConsumerCredentials;
//!
//! let client = Client::builder("MyCollectionApp/1.0 +https://example.com")
//!   .consumer(ConsumerCredentials::new("consumer-key", "consumer-secret"))
//!   .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Tracing
//!
//! Endpoint calls and HTTP requests emit [`tracing`](https://docs.rs/tracing) spans (method, URL,
//! status). Install a subscriber in your application to collect them.

pub mod auth;
pub mod client;
pub mod error;
pub mod helpers;
pub mod releases;

pub const API_BASE_URL: &str = "https://api.discogs.com";
