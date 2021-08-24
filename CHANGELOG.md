# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- markdownlint-disable MD022 -->

<!-- next-header -->
## [Unreleased] - ReleaseDate
## [0.1.1] - 2021-08-24
### Fixed
- [PR#9](https://github.com/Jake-Shadle/xwin/pull/9) resolved [#8](https://github.com/Jake-Shadle/xwin/pull/9) by adding support for additional symlinks for each `.lib` in `SCREAMING` case, since [some crates](https://github.com/microsoft/windows-rs/blob/a27a74784ccf304ab362bf2416f5f44e98e5eecd/src/bindings.rs) link them that way.

## [0.1.0] - 2021-08-22
### Added
- Initial implementation if downloading, unpacking, and splatting of the CRT and Windows SDK. This first pass focused on targeting x86_64 Desktop, so targeting the Windows Store or other architectures is not guaranteed to work.

<!-- next-url -->
[Unreleased]: https://github.com/Jake-Shadle/xwin/compare/xwin-0.1.1...HEAD
[0.1.1]: https://github.com/Jake-Shadle/xwin/compare/0.1.0...xwin-0.1.1
[0.1.0]: https://github.com/Jake-Shadle/xwin/releases/tag/0.1.0