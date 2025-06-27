# Changelog

## [0.3.0] - 2025-04-13
### Added
- Comprehensive testing across diverse images with multiple target captions.

### Changed
- Improved CLIP scoring system: removed special character penalties in favor of a simpler baseline adjustment.
- Simplified text validation to only check for empty strings and maximum length.
- Updated tests to better match real-world CLIP model performance.

### Fixed
- Fixed dependency management with proper categorization in `requirements.txt`.
- Removed unused 'clip' import and updated `requirements.txt`.

### Security
- Improved CLIP scoring to prevent exploits and increase accuracy.

[0.3.0]: https://github.com/grasslandnetwork/cliptions/releases/tag/0.3.0 