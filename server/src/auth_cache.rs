//! Authenticated address cache for iperf3 access control.
//!
//! This module maintains a cache of recently authenticated IP addresses
//! with timestamps and user information. The cache is updated from
//! HTTP/HTTPS authentication events and consulted by the iperf3 server.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Normalize an IP address by converting IPv4-mapped IPv6 addresses to IPv4.
/// 
/// When an iperf3 server listens on :: (IPv6 any address), IPv4 connections
/// appear as IPv4-mapped IPv6 addresses (e.g., ::ffff:192.0.2.1). This function
/// converts them back to IPv4 addresses for consistent cache lookups.
fn normalize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                IpAddr::V4(v4)
            } else {
                IpAddr::V6(v6)
            }
        }
        IpAddr::V4(_) => ip,
    }
}

/// Information about an authenticated address
#[derive(Clone, Debug)]
pub struct AuthenticatedAddress {
    /// The IP address
    pub ip: IpAddr,
    /// User identifier (handle/email)
    pub user_id: String,
    /// Display name if available
    pub display_name: Option<String>,
    /// When this address was last authenticated
    pub last_authenticated: Instant,
    /// Source of authentication (e.g., "oauth", "magic_key", "webrtc")
    pub auth_source: String,
}

/// Cache of recently authenticated addresses
/// 
/// This cache is designed to be:
/// - Updated from HTTP/HTTPS authentication requests
/// - Consulted synchronously by the iperf3 auth callback
/// - Thread-safe using std::sync::RwLock for sync access
pub struct AuthAddressCache {
    /// The cache: IP -> AuthenticatedAddress
    cache: RwLock<HashMap<IpAddr, AuthenticatedAddress>>,
    /// How long an address remains valid after last authentication
    timeout: Duration,
}

impl AuthAddressCache {
    /// Create a new cache with the specified timeout
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Record an authenticated address
    pub fn record_auth(&self, ip: IpAddr, user_id: String, display_name: Option<String>, auth_source: String) {
        let normalized_ip = normalize_ip(ip);
        let entry = AuthenticatedAddress {
            ip: normalized_ip,
            user_id: user_id.clone(),
            display_name: display_name.clone(),
            last_authenticated: Instant::now(),
            auth_source: auth_source.clone(),
        };

        if let Ok(mut cache) = self.cache.write() {
            tracing::debug!(
                "Recording authenticated address: {} (normalized from {}) for user '{}' via {}",
                normalized_ip, ip, user_id, auth_source
            );
            cache.insert(normalized_ip, entry);
        }
    }

    /// Refresh an existing authenticated address (update timestamp only)
    pub fn refresh_auth(&self, ip: IpAddr) -> bool {
        let normalized_ip = normalize_ip(ip);
        if let Ok(mut cache) = self.cache.write() {
            if let Some(entry) = cache.get_mut(&normalized_ip) {
                entry.last_authenticated = Instant::now();
                return true;
            }
        }
        false
    }

    /// Check if an IP is authenticated and return user info if so
    /// Also cleans up expired entries
    pub fn check_auth(&self, ip: IpAddr) -> Option<AuthenticatedAddress> {
        let normalized_ip = normalize_ip(ip);
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.get(&normalized_ip) {
                if entry.last_authenticated.elapsed() < self.timeout {
                    return Some(entry.clone());
                }
            }
        }
        None
    }

    /// Check if an IP is authenticated (simple boolean check)
    pub fn is_authenticated(&self, ip: IpAddr) -> bool {
        self.check_auth(ip).is_some()
    }

    /// Remove expired entries from the cache
    pub fn cleanup_expired(&self) {
        if let Ok(mut cache) = self.cache.write() {
            let timeout = self.timeout;
            cache.retain(|_, entry| entry.last_authenticated.elapsed() < timeout);
        }
    }

    /// Get all currently valid authenticated addresses
    pub fn get_all_valid(&self) -> Vec<AuthenticatedAddress> {
        if let Ok(cache) = self.cache.read() {
            cache.values()
                .filter(|entry| entry.last_authenticated.elapsed() < self.timeout)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the configured timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Shared authenticated address cache type
pub type SharedAuthAddressCache = Arc<AuthAddressCache>;

/// Create a new shared authenticated address cache
pub fn create_auth_cache(timeout_secs: u64) -> SharedAuthAddressCache {
    Arc::new(AuthAddressCache::new(timeout_secs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_normalize_ip_v4() {
        let ipv4 = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
        assert_eq!(normalize_ip(ipv4), ipv4);
    }

    #[test]
    fn test_normalize_ip_v6() {
        let ipv6 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        assert_eq!(normalize_ip(ipv6), ipv6);
    }

    #[test]
    fn test_normalize_ip_v4_mapped() {
        // IPv4-mapped IPv6 address: ::ffff:192.0.2.1
        let ipv4_mapped = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc000, 0x0201));
        let expected_ipv4 = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
        assert_eq!(normalize_ip(ipv4_mapped), expected_ipv4);
    }

    #[test]
    fn test_normalize_ip_v4_mapped_from_string() {
        let ipv4_mapped = "::ffff:37.228.235.27".parse::<IpAddr>().unwrap();
        let expected_ipv4 = "37.228.235.27".parse::<IpAddr>().unwrap();
        assert_eq!(normalize_ip(ipv4_mapped), expected_ipv4);
    }

    #[test]
    fn test_cache_with_normalized_ips() {
        let cache = AuthAddressCache::new(60);
        
        // Record with regular IPv4
        let ipv4 = "192.0.2.1".parse::<IpAddr>().unwrap();
        cache.record_auth(ipv4, "user1".to_string(), None, "test".to_string());
        
        // Check with IPv4-mapped IPv6 should find it
        let ipv4_mapped = "::ffff:192.0.2.1".parse::<IpAddr>().unwrap();
        assert!(cache.is_authenticated(ipv4_mapped));
        assert!(cache.check_auth(ipv4_mapped).is_some());
    }

    #[test]
    fn test_cache_record_with_ipv4_mapped() {
        let cache = AuthAddressCache::new(60);
        
        // Record with IPv4-mapped IPv6
        let ipv4_mapped = "::ffff:192.0.2.1".parse::<IpAddr>().unwrap();
        cache.record_auth(ipv4_mapped, "user1".to_string(), None, "test".to_string());
        
        // Check with regular IPv4 should find it
        let ipv4 = "192.0.2.1".parse::<IpAddr>().unwrap();
        assert!(cache.is_authenticated(ipv4));
        assert!(cache.check_auth(ipv4).is_some());
    }

    #[test]
    fn test_cache_both_directions() {
        let cache = AuthAddressCache::new(60);
        
        let ipv4 = "37.228.235.27".parse::<IpAddr>().unwrap();
        let ipv4_mapped = "::ffff:37.228.235.27".parse::<IpAddr>().unwrap();
        
        // Record with IPv4
        cache.record_auth(ipv4, "user1".to_string(), Some("User One".to_string()), "oauth".to_string());
        
        // Check with both should work
        let auth_v4 = cache.check_auth(ipv4);
        let auth_v6 = cache.check_auth(ipv4_mapped);
        
        assert!(auth_v4.is_some());
        assert!(auth_v6.is_some());
        assert_eq!(auth_v4.unwrap().user_id, auth_v6.unwrap().user_id);
    }
}
