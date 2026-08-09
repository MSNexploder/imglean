use std::time::Duration;

pub const LIMITS_VERSION: &str = "v1";

pub const MAX_INPUTS: usize = 128;
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_AGGREGATE_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_CANDIDATE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_TEMPORARY_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;

pub const MAX_WIDTH: u32 = 32_768;
pub const MAX_HEIGHT: u32 = 32_768;
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;
pub const MAX_RECONSTRUCTED_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_CHUNK_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 4_096;
pub const MAX_ANCILLARY_BYTES: usize = 16 * 1024 * 1024;

pub const VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);
pub const PROVIDER_TIMEOUT: Duration = Duration::from_secs(55);
pub const WORKER_TIMEOUT: Duration = Duration::from_secs(60);
pub const INVOCATION_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_timeout_leaves_controller_cleanup_time() {
        assert!(PROVIDER_TIMEOUT < WORKER_TIMEOUT);
        assert!(WORKER_TIMEOUT < INVOCATION_TIMEOUT);
    }

    #[test]
    fn temporary_budget_holds_largest_live_artifacts() {
        assert!(
            MAX_SOURCE_BYTES
                .checked_add(MAX_CANDIDATE_BYTES)
                .and_then(|bytes| bytes.checked_add(MAX_SOURCE_BYTES))
                .is_some_and(|bytes| bytes <= MAX_TEMPORARY_BYTES)
        );
    }
}
