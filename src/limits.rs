use std::time::Duration;

pub const LIMITS_VERSION: &str = "v9";

pub const MAX_INPUTS: usize = 128;
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_AGGREGATE_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_CANDIDATE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_TEMPORARY_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;
pub const DEFAULT_STRATEGY_WORKERS: usize = 2;
pub const MAX_STRATEGY_WORKERS: usize = 3;

pub const MAX_WIDTH: u32 = 32_768;
pub const MAX_HEIGHT: u32 = 32_768;
pub const MAX_AVIF_DIMENSION: u32 = 8_192;
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;
pub const MAX_RECONSTRUCTED_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_CHUNK_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 4_096;
pub const MAX_ANCILLARY_BYTES: usize = 16 * 1024 * 1024;

pub const VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);
pub const PROVIDER_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
pub const DEFAULT_STRATEGY_TIMEOUT: Duration = Duration::from_secs(60);
pub const MIN_STRATEGY_TIMEOUT_SECONDS: u64 = 6;
pub const MAX_STRATEGY_TIMEOUT_SECONDS: u64 = 10 * 60;
pub const OXIPNG_CLEANUP_RESERVE: Duration = Duration::from_secs(5);
pub const MIN_OXIPNG_TIMEOUT: Duration = Duration::from_secs(1);
pub const INVOCATION_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_timeout_range_preserves_oxipng_and_invocation_cleanup_time() {
        assert!(Duration::from_secs(MIN_STRATEGY_TIMEOUT_SECONDS) > OXIPNG_CLEANUP_RESERVE);
        assert_eq!(
            Duration::from_secs(MIN_STRATEGY_TIMEOUT_SECONDS) - OXIPNG_CLEANUP_RESERVE,
            MIN_OXIPNG_TIMEOUT
        );
        assert!(DEFAULT_STRATEGY_TIMEOUT > OXIPNG_CLEANUP_RESERVE);
        assert!(Duration::from_secs(MAX_STRATEGY_TIMEOUT_SECONDS) < INVOCATION_TIMEOUT);
    }

    #[test]
    fn temporary_budget_holds_largest_live_artifacts() {
        assert!(
            MAX_SOURCE_BYTES
                .checked_add(MAX_CANDIDATE_BYTES)
                .and_then(|bytes| bytes.checked_add(MAX_SOURCE_BYTES))
                .is_some_and(|bytes| bytes <= MAX_TEMPORARY_BYTES)
        );
        assert_eq!(
            MAX_TEMPORARY_BYTES * MAX_STRATEGY_WORKERS as u64,
            768 * 1024 * 1024
        );
    }

    #[test]
    fn avif_dimension_limit_matches_the_pixel_limit() {
        assert_eq!(
            u64::from(MAX_AVIF_DIMENSION) * u64::from(MAX_AVIF_DIMENSION),
            MAX_PIXELS
        );
    }
}
