# Bug Fixes

This document tracks significant bug fixes made to the odjitter codebase.

## Issue #52: Latest version does not compile (Fixed)

**Problem**: Running `cargo install --git https://github.com/dabreegster/odjitter` would fail with:
```
error[E0277]: the trait bound `geozero::geojson::GeoJson<'_>: 
flatgeobuf::geozero::GeozeroDatasource` is not satisfied
```

**Root Cause**: 
- `cargo install` ignores `Cargo.lock` and resolves dependencies fresh
- `flatgeobuf = "3.26.1"` with caret requirements allows cargo to install 3.27.0
- `flatgeobuf 3.27.0` depends on a different version of `geozero` than what odjitter specifies
- This creates a version mismatch where two different `geozero` versions exist in the dependency tree
- The `geozero::geojson::GeoJson` type from version 0.10.0 doesn't implement the trait required by the `geozero` re-exported from `flatgeobuf` 3.27.0

**Solution**: 
Pin exact versions using `=` in Cargo.toml:
```toml
flatgeobuf = { version = "=3.26.1", default-features = false }
geozero = { version = "=0.10.0", default-features = false, features = ["with-geojson"] }
```

This ensures that `cargo install` will use the exact same versions that work locally.

**Testing**:
```bash
# Test that it builds without Cargo.lock
cd /tmp && cargo new test && cd test
# Copy src and Cargo.toml (but NOT Cargo.lock)
cargo build --release  # Should succeed
```

**Alternative Solutions Considered**:
1. Update to latest `flatgeobuf` and `geozero` - Would require code changes
2. Use `~` version requirements - Less strict, might still have issues
3. Convert Feature differently for FlatGeobuf - More complex code change

**Workaround (before fix)**:
```bash
# Install from specific commit that worked
cargo install --git https://github.com/dabreegster/odjitter --rev 32fb58bf7f0d68afd3b76b88cf6b1272c5c66828
```

Or build locally:
```bash
git clone https://github.com/dabreegster/odjitter
cd odjitter
cargo build --release
cp ./target/release/odjitter /usr/local/bin/
```

**Impact**: 
- ✅ `cargo install --git` now works reliably
- ✅ Docker builds more reliable
- ✅ Fresh checkouts compile consistently
- ✅ No code changes required, only dependency pinning

**Trade-offs**:
- ⚠️ Won't automatically get patch updates to dependencies
- ⚠️ Need to explicitly update and test when bumping versions
- ✅ But ensures stability and consistent builds

## Clippy Warnings (Fixed)

**Problem**: Two needless borrow warnings in `src/lib.rs`

**Solution**: Removed unnecessary `&` references in calls to `Subsampler::new()`

**Files Changed**: `src/lib.rs` lines 202 and 206
