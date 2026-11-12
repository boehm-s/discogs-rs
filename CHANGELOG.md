# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0]

### Added

- Releases: `Client::get_release`, `get_release_rating_and_stats` (`/rating` and `/stats` combined),
  `get_master_release`, `list_master_versions` (filters, sorting)
- Links between models: `Release::get_master_release`, `Release::get_rating_and_stats`,
  `MasterRelease::list_versions`, `MasterRelease::get_main_release`, `MasterVersion::get_release`
- Pagination: `Pagination`, and `PaginatedResponse<T>` for any `T: ApiResource`
- Typed ids (`ReleaseId`, `MasterReleaseId`, `ArtistId`, `LabelId`, `UserId`)

### Changed

- `Identity::id` is a `UserId`

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
