//! Keeping the server standing: response headers, rate limits and the few password rules.
//!
//! None of this is clever. It is the small set of limits that stop one browser tab, or one script, from filling the
//! database with accounts, the memory with rooms, or the logs with noise.

use axum::extract::Request;
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A sliding window counter per key. Keys are addresses or client IPs, so the map stays small; it is swept whenever
/// it grows past a few thousand entries.
#[derive(Default)]
pub struct Limiter {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl Limiter {
    /// Records one attempt. Returns false when `key` has already used up `max` attempts inside `window`.
    pub fn allow(&self, key: &str, max: usize, window: Duration) -> bool {
        let Ok(mut map) = self.hits.lock() else { return true }; // a poisoned lock must not lock players out
        if map.len() > 4000 {
            map.retain(|_, v| v.iter().any(|t| t.elapsed() < window));
        }
        let list = map.entry(key.to_string()).or_default();
        list.retain(|t| t.elapsed() < window);
        if list.len() >= max {
            return false;
        }
        list.push(Instant::now());
        true
    }

    /// Forgets a key, for example after a successful login.
    pub fn clear(&self, key: &str) {
        if let Ok(mut map) = self.hits.lock() {
            map.remove(key);
        }
    }
}

/// The 20 passwords people actually pick first. Everything else of the right length is accepted: this is a game,
/// not a bank, and a long rule list only pushes players towards writing it down.
const WEAK: &[&str] = &[
    "123456", "password", "123456789", "12345678", "12345", "qwerty", "abc123", "password1", "111111", "123123",
    "1234567", "letmein", "welcome", "monkey", "iloveyou", "admin123", "000000", "qwerty123", "1q2w3e4r", "kolom",
];

/// Checks a new password. Length is the real rule; the list only blocks the handful that get guessed first.
pub fn check_password(p: &str) -> Result<(), String> {
    if p.chars().count() < 6 {
        return Err("পাসওয়ার্ড কমপক্ষে ৬ অক্ষরের হতে হবে".into());
    }
    if p.len() > 100 {
        return Err("পাসওয়ার্ড অনেক বড়".into());
    }
    if WEAK.contains(&p.to_lowercase().trim()) {
        return Err("এই পাসওয়ার্ডটি খুব সহজ, অন্য একটি দিন".into());
    }
    Ok(())
}

/// How long a signed-in session stays good. After this the player signs in again.
pub const SESSION_MAX_AGE: i64 = 60 * 60 * 24 * 30;

/// Headers on every response.
///
/// The page is served from this same origin and talks only to it, so the policy below is as tight as a Next.js
/// export allows: inline scripts have to stay allowed (the export writes one inline bootstrap script and cannot be
/// given a nonce), but nothing may be loaded from another origin, the page cannot be framed, and no plugin,
/// camera, microphone or location is reachable.
pub async fn headers(req: Request, next: Next) -> Response {
    let api = req.uri().path().starts_with("/api/");
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    let set = |h: &mut axum::http::HeaderMap, k: header::HeaderName, v: &'static str| {
        h.insert(k, HeaderValue::from_static(v));
    };
    set(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(h, header::X_FRAME_OPTIONS, "DENY");
    set(h, header::REFERRER_POLICY, "no-referrer");
    h.insert("permissions-policy", HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=()"));
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             img-src 'self' data: blob:; \
             style-src 'self' 'unsafe-inline'; \
             script-src 'self' 'unsafe-inline'; \
             connect-src 'self' ws: wss:; \
             font-src 'self'; \
             media-src 'self'; \
             object-src 'none'; \
             base-uri 'none'; \
             form-action 'self'; \
             frame-ancestors 'none'",
        ),
    );
    // Static files may sit in a cache for a few minutes; anything from the API is per-player and never may.
    if !h.contains_key(header::CACHE_CONTROL) {
        set(h, header::CACHE_CONTROL, if api { "no-store" } else { "public, max-age=300" });
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_lets_through_max_and_then_stops() {
        let l = Limiter::default();
        let w = Duration::from_secs(60);
        for i in 0..3 {
            assert!(l.allow("a", 3, w), "attempt {i} should pass");
        }
        assert!(!l.allow("a", 3, w), "the fourth is over the limit");
        assert!(l.allow("b", 3, w), "a different key is untouched");
        l.clear("a");
        assert!(l.allow("a", 3, w), "clearing lets it start again");
    }

    #[test]
    fn the_window_moves() {
        let l = Limiter::default();
        let w = Duration::from_millis(40);
        assert!(l.allow("x", 1, w));
        assert!(!l.allow("x", 1, w));
        std::thread::sleep(Duration::from_millis(60));
        assert!(l.allow("x", 1, w), "once the window has passed it is allowed again");
    }

    #[test]
    fn passwords() {
        assert!(check_password("secret1").is_ok());
        assert!(check_password("12345").is_err(), "too short");
        assert!(check_password("password").is_err(), "on the common list");
        assert!(check_password("PassWord").is_err(), "the list ignores case");
        assert!(check_password(&"a".repeat(101)).is_err());
    }
}
