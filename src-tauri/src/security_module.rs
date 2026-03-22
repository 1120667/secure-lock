// SecureLock — Security Module
// Handles PIN hashing (argon2), verification, and brute-force rate limiting.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rate limiter state.
#[derive(Debug)]
pub struct RateLimiter {
    pub failed_attempts: u32,
    pub max_attempts: u32,
    pub cooldown_secs: u64,
    pub locked_until: Option<Instant>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            failed_attempts: 0,
            max_attempts: 5,
            cooldown_secs: 30,
            locked_until: None,
        }
    }
}

/// Shared rate limiter.
pub type SharedRateLimiter = Arc<Mutex<RateLimiter>>;

/// Create a new shared rate limiter.
pub fn new_rate_limiter() -> SharedRateLimiter {
    Arc::new(Mutex::new(RateLimiter::default()))
}

/// Hash a PIN using argon2id.
pub fn hash_pin(pin: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(pin.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| format!("Failed to hash PIN: {}", e))
}

/// Verify a PIN against a stored hash.
pub fn verify_pin(pin: &str, stored_hash: &str) -> Result<bool, String> {
    let parsed_hash =
        PasswordHash::new(stored_hash).map_err(|e| format!("Invalid hash format: {}", e))?;

    let argon2 = Argon2::default();
    Ok(argon2
        .verify_password(pin.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Attempt a PIN unlock with rate limiting.
/// Returns Ok(true) if verified, Ok(false) if wrong PIN, Err if rate limited.
pub fn attempt_unlock(
    pin: &str,
    stored_hash: &str,
    rate_limiter: &SharedRateLimiter,
) -> Result<bool, String> {
    let mut limiter = rate_limiter.lock();

    // Check if currently in cooldown
    if let Some(locked_until) = limiter.locked_until {
        if Instant::now() < locked_until {
            let remaining = locked_until
                .duration_since(Instant::now())
                .as_secs();
            return Err(format!(
                "Too many failed attempts. Try again in {} seconds.",
                remaining + 1
            ));
        } else {
            // Cooldown expired — reset
            limiter.locked_until = None;
            limiter.failed_attempts = 0;
        }
    }

    // Release lock before expensive hashing
    drop(limiter);

    let is_valid = verify_pin(pin, stored_hash)?;

    let mut limiter = rate_limiter.lock();
    if is_valid {
        // Reset on success
        limiter.failed_attempts = 0;
        limiter.locked_until = None;
        Ok(true)
    } else {
        limiter.failed_attempts += 1;
        if limiter.failed_attempts >= limiter.max_attempts {
            limiter.locked_until =
                Some(Instant::now() + Duration::from_secs(limiter.cooldown_secs));
            log::warn!(
                "Rate limit triggered after {} failed attempts",
                limiter.failed_attempts
            );
        }
        Ok(false)
    }
}

/// Get remaining cooldown seconds (0 if not locked).
pub fn get_cooldown_remaining(rate_limiter: &SharedRateLimiter) -> u64 {
    let limiter = rate_limiter.lock();
    match limiter.locked_until {
        Some(until) if Instant::now() < until => {
            until.duration_since(Instant::now()).as_secs() + 1
        }
        _ => 0,
    }
}

/// Get number of failed attempts.
pub fn get_failed_attempts(rate_limiter: &SharedRateLimiter) -> u32 {
    let limiter = rate_limiter.lock();
    limiter.failed_attempts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let pin = "1234";
        let hash = hash_pin(pin).unwrap();
        assert!(verify_pin(pin, &hash).unwrap());
        assert!(!verify_pin("9999", &hash).unwrap());
    }

    #[test]
    fn test_rate_limiting() {
        let pin = "1234";
        let hash = hash_pin(pin).unwrap();
        let limiter = new_rate_limiter();

        // 5 wrong attempts
        for _ in 0..5 {
            let result = attempt_unlock("wrong", &hash, &limiter);
            assert!(result.is_ok());
            assert!(!result.unwrap());
        }

        // 6th attempt should be rate limited
        let result = attempt_unlock("wrong", &hash, &limiter);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Too many failed attempts"));
    }

    #[test]
    fn test_successful_unlock_resets_counter() {
        let pin = "5678";
        let hash = hash_pin(pin).unwrap();
        let limiter = new_rate_limiter();

        // 3 wrong attempts
        for _ in 0..3 {
            attempt_unlock("wrong", &hash, &limiter).unwrap();
        }
        assert_eq!(get_failed_attempts(&limiter), 3);

        // Correct PIN resets
        let result = attempt_unlock(pin, &hash, &limiter).unwrap();
        assert!(result);
        assert_eq!(get_failed_attempts(&limiter), 0);
    }
}
