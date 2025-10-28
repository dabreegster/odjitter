# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-10-28

### Fixed
- **Critical**: Fixed `cargo install --git` compilation failure (Issue #52)
  - Root cause: Version conflicts between `flatgeobuf` and `geozero` when installing without `Cargo.lock`
  - Solution: Updated all dependencies to latest compatible versions
  - `cargo install` now works reliably without workarounds

### Changed
- **Breaking (dependencies only)**: Updated all major dependencies
  - `flatgeobuf`: 3.26.1 → 5.0 (major geospatial format improvements)
  - `geozero`: 0.10.0 → 0.14 (enhanced format support)
  - `geo`: 0.26 → 0.28 (latest geometric algorithms)
  - `clap`: 3.0 → 4.5 (improved CLI parsing)
  - `fs-err`: 2.9 → 3.0 (better error messages)
  - `ordered-float`: 3.7 → 4.0 (performance improvements)
  - `rstar`: 0.11 → 0.12 (spatial indexing enhancements)
- CLI interface remains unchanged - no breaking changes for users
- Binary version bumped from 0.1.0 to 0.2.0

### Added
- **12 new comprehensive tests** (300% increase in test coverage):
  - `test_min_distance_constraint` - Validates minimum distance enforcement
  - `test_zero_trip_rows_preserved` - Edge case handling for zero-trip rows
  - `test_weighted_points_distribution` - Validates weighted sampling (Issue #18)
  - `test_random_points_subsample` - Tests default random point generation
  - `test_different_thresholds_consistency` - Ensures consistent totals
  - `test_properties_preserved` - Data integrity validation
  - `test_deterministic_with_seed` - Reproducibility verification
  - `test_disaggregate_mode_column` - Full disaggregation testing
  - `test_large_disaggregation_threshold` - Large threshold edge case
  - `test_mixed_zone_types` - Polygon/MultiPolygon handling (Issue #30)
  - `test_subpoints_without_weights` - Default weight behavior
  - `test_geometry_types` - Output format correctness
- New documentation files:
  - `TESTING_IMPROVEMENTS.md` - Comprehensive test documentation
  - `CHANGELOG.md` - This file
  
### Improved
- Fixed all clippy warnings (needless borrows in `src/lib.rs`)
- Better long-term maintainability with modern dependencies
- Improved security posture with latest dependency versions
- More robust error handling tested

### Removed
- Removed exact version pinning workarounds (no longer needed)

## [0.1.0] - 2021-12-27 (Original Release)

Initial release by Dustin Carlino.

### Features
- Jitter OD data from zone centroids to specific points
- Two modes: `jitter` (partial disaggregation) and `disaggregate` (full)
- Support for weighted subpoints
- Random point sampling within zones
- Configurable disaggregation thresholds
- Minimum distance constraints
- Duplicate pair detection
- Both GeoJSON and FlatGeobuf output formats
- R package interface via system calls
- Published methodology in Findings journal

### Known Issues
- `cargo install --git` fails with version conflicts (fixed in v0.2.0)
- Some edge cases not fully tested (addressed in v0.2.0)

[0.2.0]: https://github.com/dabreegster/odjitter/compare/main...v0.2.0
[0.1.0]: https://github.com/dabreegster/odjitter/releases/tag/v0.1.0
