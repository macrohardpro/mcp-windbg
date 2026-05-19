use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

use crate::http::error::{HttpError, HttpResult};

/// Rate limiter using sliding window algorithm
pub struct RateLimiter {
    /// Map of IP address to request timestamps
    requests: Arc<RwLock<HashMap<IpAddr, Vec<Instant>>>>,
    /// Maximum requests per window
    max_requests: usize,
    /// Time window duration
    window: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(max_requests_per_minute: usize) -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            max_requests: max_requests_per_minute,
            window: Duration::from_secs(60),
        }
    }
    
    /// Check if a request from the given IP is allowed
    pub async fn check_rate_limit(&self, ip: IpAddr) -> HttpResult<()> {
        let now = Instant::now();
        let mut requests = self.requests.write().await;
        
        // Get or create entry for this IP
        let timestamps = requests.entry(ip).or_insert_with(Vec::new);
        
        // Remove expired timestamps (outside the window)
        timestamps.retain(|&timestamp| now.duration_since(timestamp) < self.window);
        
        // Check if limit exceeded
        if timestamps.len() >= self.max_requests {
            debug!(
                "Rate limit exceeded for IP {}: {} requests in last {} seconds",
                ip,
                timestamps.len(),
                self.window.as_secs()
            );
            return Err(HttpError::RateLimitExceeded);
        }
        
        // Add current timestamp
        timestamps.push(now);
        
        debug!(
            "Rate limit check passed for IP {}: {} / {} requests",
            ip,
            timestamps.len(),
            self.max_requests
        );
        
        Ok(())
    }
    
    /// Clean up expired entries (optional maintenance)
    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        let mut requests = self.requests.write().await;
        
        // Remove IPs with no recent requests
        requests.retain(|_ip, timestamps| {
            timestamps.retain(|&timestamp| now.duration_since(timestamp) < self.window);
            !timestamps.is_empty()
        });
        
        debug!("Rate limiter cleanup: {} active IPs", requests.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    
    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(3);
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        
        // First 3 requests should succeed
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        assert!(limiter.check_rate_limit(ip).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(3);
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        
        // First 3 requests should succeed
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        
        // 4th request should fail
        assert!(matches!(
            limiter.check_rate_limit(ip).await,
            Err(HttpError::RateLimitExceeded)
        ));
    }
    
    #[tokio::test]
    async fn test_rate_limiter_different_ips() {
        let limiter = RateLimiter::new(2);
        let ip1 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));
        
        // Each IP should have its own limit
        assert!(limiter.check_rate_limit(ip1).await.is_ok());
        assert!(limiter.check_rate_limit(ip1).await.is_ok());
        
        assert!(limiter.check_rate_limit(ip2).await.is_ok());
        assert!(limiter.check_rate_limit(ip2).await.is_ok());
        
        // Both should be at limit
        assert!(matches!(
            limiter.check_rate_limit(ip1).await,
            Err(HttpError::RateLimitExceeded)
        ));
        assert!(matches!(
            limiter.check_rate_limit(ip2).await,
            Err(HttpError::RateLimitExceeded)
        ));
    }
    
    #[tokio::test]
    async fn test_cleanup_expired() {
        let limiter = RateLimiter::new(3);
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        
        // Make some requests
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        
        // Cleanup should not remove recent requests
        limiter.cleanup_expired().await;
        
        // Should still be able to make one more request (2/3 used)
        assert!(limiter.check_rate_limit(ip).await.is_ok());
    }
}
