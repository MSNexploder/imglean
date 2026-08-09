use std::time::Duration;

pub const LIMITS_VERSION: &str = "v4";

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
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;
pub const MAX_RECONSTRUCTED_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_CHUNK_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 4_096;
pub const MAX_ANCILLARY_BYTES: usize = 16 * 1024 * 1024;

pub const VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);
pub const PROVIDER_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
pub const OXIPNG_TIMEOUT: Duration = Duration::from_secs(55);
pub const EMBEDDED_WORKER_TIMEOUT: Duration = Duration::from_secs(60);
pub const OPTIPNG_TIMEOUT: Duration = Duration::from_secs(60);
pub const PNGQUANT_TIMEOUT: Duration = Duration::from_secs(60);
pub const INVOCATION_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_timeout_leaves_controller_cleanup_time() {
        assert!(OXIPNG_TIMEOUT < EMBEDDED_WORKER_TIMEOUT);
        assert!(EMBEDDED_WORKER_TIMEOUT < INVOCATION_TIMEOUT);
        assert!(OPTIPNG_TIMEOUT < INVOCATION_TIMEOUT);
        assert!(PNGQUANT_TIMEOUT < INVOCATION_TIMEOUT);
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
}
