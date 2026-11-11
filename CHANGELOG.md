# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0]

### Added

- Authentication, with three methods on `Client`: personal access token, OAuth 1.0a
  (`OAuthFlow`, `AccessToken`) and application-only consumer key/secret
- `Client::identity` (`GET /oauth/identity`)
- `DiscogsClientError` type, mapping Discogs error responses to `DiscogsClientError::Api`
- `tracing` spans: one per endpoint call, and a `Calling Discogs` span per HTTP request (method,
  URL, status)

## [0.1.0]

### Added

- Project skeleton
