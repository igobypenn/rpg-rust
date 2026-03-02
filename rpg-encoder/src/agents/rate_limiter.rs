//! Token bucket rate limiter for LLM API calls.
//!
//! Implements a token bucket algorithm to limit the rate of API requests,
//! preventing rate limit errors from providers.

use crate::error::{LlmErrorKind, RpgError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u8,
    /// Base delay for exponential backoff (seconds)
    pub base_delay: Duration,
    /// Maximum delay cap (seconds)
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
        }
    }
}

impl RetryConfig {
    /// Create config from environment variables.
    ///
    /// Reads:
    /// - `OPENAI_MAX_RETRIES`: Max retry attempts (default: 3)
    /// - `OPENAI_RETRY_BASE_DELAY_MS`: Base delay in ms (default: 1000)
    /// - `OPENAI_RETRY_MAX_DELAY_MS`: Max delay cap in ms (default: 60000)
    pub fn from_env() -> Self {
        let max_retries = std::env::var("OPENAI_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);
        
        let base_delay_ms = std::env::var("OPENAI_RETRY_BASE_DELAY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);
        
        let max_delay_ms = std::env::var("OPENAI_RETRY_MAX_DELAY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60000);
        
        Self {
            max_retries,
            base_delay: Duration::from_millis(base_delay_ms),
            max_delay: Duration::from_millis(max_delay_ms),
        }
    }

    /// Set the maximum retry attempts.
    pub fn with_max_retries(mut self, max: u8) -> Self {
        self.max_retries = max;
        self
    }

    /// Set the base delay.
    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    /// Set the maximum delay cap.
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Calculate delay for a given retry attempt.
    ///
    /// Uses exponential backoff: base * 2^attempt
    pub fn delay_for_attempt(&self, attempt: u8) -> Duration {
        let multiplier = 1u64 << attempt.min(4);
        let delay = self.base_delay.as_millis() as u64 * multiplier;
        Duration::from_millis(delay.min(self.max_delay.as_millis() as u64))
    }
}

/// Execute an async operation with retry logic.
///
/// Retries on retryable errors (rate limits, timeouts, network errors).
/// Returns the result or the final error after all retries exhausted.
pub async fn with_retry<T, F, Fut>(config: &RetryConfig, mut operation: F) -> Result<T, RpgError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, RpgError>>,
{
    let mut _last_error: Option<RpgError> = None;
    let mut attempt: u8 = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if !err.is_retryable() {
                    return Err(err);
                }

                if attempt >= config.max_retries {
                    return Err(RpgError::llm_with_retry(
                        err.llm_kind().unwrap_or(LlmErrorKind::Unknown),
                        format!("Max retries ({}) exceeded", config.max_retries),
                        None,
                        attempt,
                    ));
                }

                let delay = err.retry_delay().unwrap_or_else(|| config.delay_for_attempt(attempt));
                
                tracing::warn!(
                    attempt = attempt + 1,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis(),
                    error = %err,
                );

                tokio::time::sleep(delay).await;

                _last_error = Some(err);
                attempt += 1;
            }
        }
    }
}

/// Configuration for the rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum concurrent requests (token bucket capacity)
    pub max_concurrent: usize,
    /// Minimum delay between requests
    pub min_delay: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 1,
            min_delay: Duration::from_millis(100),
        }
    }
}

impl RateLimitConfig {
    /// Create config from environment variables.
    ///
    /// Reads:
    /// - `OPENAI_MAX_CONCURRENT`: Max concurrent requests (default: 1)
    /// - `OPENAI_MIN_DELAY_MS`: Min delay between requests in ms (default: 100)
    pub fn from_env() -> Self {
        let max_concurrent = std::env::var("OPENAI_MAX_CONCURRENT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1)
            .max(1);
        
        let min_delay_ms = std::env::var("OPENAI_MIN_DELAY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        
        Self {
            max_concurrent,
            min_delay: Duration::from_millis(min_delay_ms),
        }
    }

    /// Set the maximum concurrent requests.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max.max(1);
        self
    }

    /// Set the minimum delay between requests.
    pub fn with_min_delay(mut self, delay: Duration) -> Self {
        self.min_delay = delay;
        self
    }
}

/// Token bucket rate limiter for controlling API request rates.
///
/// Uses a semaphore for concurrency control and tracks last request time
/// for minimum delay enforcement.
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    semaphore: Arc<Semaphore>,
    last_request: Arc<Mutex<Option<Instant>>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent)),
            config,
            last_request: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a rate limiter from environment configuration.
    pub fn from_env() -> Self {
        Self::new(RateLimitConfig::from_env())
    }

    /// Create a rate limiter with default configuration.
    pub fn default_limiter() -> Self {
        Self::new(RateLimitConfig::default())
    }

    /// Acquire a permit to make an API request.
    ///
    /// This will wait until:
    /// 1. A semaphore permit is available (concurrency limit)
    /// 2. The minimum delay since the last request has elapsed
    ///
    /// Returns a guard that releases the permit when dropped.
    pub async fn acquire(&self) -> RateLimitGuard<'_> {
        let permit = self.semaphore.acquire().await.expect("semaphore closed");
        self.enforce_min_delay().await;
        *self.last_request.lock().await = Some(Instant::now());
        RateLimitGuard { permit }
    }

    /// Try to acquire a permit without waiting.
    ///
    /// Returns `None` if the rate limit would be exceeded.
    pub async fn try_acquire(&self) -> Option<RateLimitGuard<'_>> {
        let permit = self.semaphore.try_acquire().ok()?;
        
        let last = self.last_request.lock().await;
        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.config.min_delay {
                drop(permit);
                return None;
            }
        }
        drop(last);
        
        *self.last_request.lock().await = Some(Instant::now());
        Some(RateLimitGuard { permit })
    }

    /// Wait for the minimum delay if necessary.
    async fn enforce_min_delay(&self) {
        let last = self.last_request.lock().await;
        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.config.min_delay {
                let wait_time = self.config.min_delay - elapsed;
                drop(last);
                tokio::time::sleep(wait_time).await;
            }
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Get the number of available permits.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

/// Guard that releases the rate limit permit when dropped.
#[must_use = "RateLimitGuard must be held during the API call"]
pub struct RateLimitGuard<'a> {
    #[allow(dead_code)]
    permit: SemaphorePermit<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_concurrent, 1);
        assert_eq!(config.min_delay, Duration::from_millis(100));
    }

    #[test]
    fn test_config_builders() {
        let config = RateLimitConfig::default()
            .with_max_concurrent(5)
            .with_min_delay(Duration::from_millis(200));
        
        assert_eq!(config.max_concurrent, 5);
        assert_eq!(config.min_delay, Duration::from_millis(200));
    }

    #[test]
    fn test_config_max_concurrent_floor() {
        let config = RateLimitConfig::default().with_max_concurrent(0);
        assert_eq!(config.max_concurrent, 1);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_concurrent: 2,
            min_delay: Duration::from_millis(10),
        });
        
        let _guard1 = limiter.acquire().await;
        let _guard2 = limiter.acquire().await;
        
        assert_eq!(limiter.available_permits(), 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_release() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_concurrent: 1,
            min_delay: Duration::from_millis(0),
        });
        
        {
            let _guard = limiter.acquire().await;
            assert_eq!(limiter.available_permits(), 0);
        }
        
        assert_eq!(limiter.available_permits(), 1);
    }

    #[tokio::test]
    async fn test_rate_limiter_min_delay() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_concurrent: 2,
            min_delay: Duration::from_millis(50),
        });
        
        let start = Instant::now();
        let _guard1 = limiter.acquire().await;
        let _guard2 = limiter.acquire().await;
        let elapsed = start.elapsed();
        
        assert!(elapsed >= Duration::from_millis(40));
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_concurrent: 1,
            min_delay: Duration::from_millis(100),
        });
        
        let guard1 = limiter.try_acquire().await;
        assert!(guard1.is_some());
        
        let guard2 = limiter.try_acquire().await;
        assert!(guard2.is_none());
        
        drop(guard1);
        
        tokio::time::sleep(Duration::from_millis(110)).await;
        let guard3 = limiter.try_acquire().await;
        assert!(guard3.is_some());
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let limiter = RateLimiter::new(RateLimitConfig {
            max_concurrent: 3,
            min_delay: Duration::from_millis(0),
        });
        
        let mut handles = vec![];
        
        for _ in 0..6 {
            let limiter = limiter.clone();
            handles.push(tokio::spawn(async move {
                let _guard = limiter.acquire().await;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }));
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
    }
}
