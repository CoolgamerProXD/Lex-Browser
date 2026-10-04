use std::{collections::HashMap, time::{Duration, SystemTime}};

use reqwest::header::{CACHE_CONTROL, EXPIRES};

use crate::{LexUrl, NavigationResponse};

#[derive(Clone, Debug)]
pub(crate) struct CacheEntry {
    response: NavigationResponse,
    expires_at: SystemTime,
}

impl CacheEntry {
    pub(crate) fn from_response(response: &NavigationResponse, now: SystemTime) -> Option<Self> {
        if response.status != 200 {
            return None;
        }
        let cache_control = response
            .headers
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let mut max_age = None;
        for directive in cache_control.split(',').map(str::trim) {
            if directive.eq_ignore_ascii_case("no-store")
                || directive.eq_ignore_ascii_case("private")
            {
                return None;
            }
            if let Some(value) = directive.strip_prefix("max-age=") {
                max_age = value.trim_matches('"').parse::<u64>().ok();
            }
        }
        // M3 deliberately requires explicit freshness. Expires parsing will be
        // added alongside conditional revalidation; retaining the lookup here
        // prevents accidentally treating an Expires-only response as fresh.
        let _expires = response.headers.get(EXPIRES);
        let lifetime = Duration::from_secs(max_age?);
        if lifetime.is_zero() {
            return None;
        }
        Some(Self {
            response: response.clone(),
            expires_at: now.checked_add(lifetime)?,
        })
    }

    fn fresh_response(&self, now: SystemTime) -> Option<NavigationResponse> {
        if now >= self.expires_at {
            return None;
        }
        let mut response = self.response.clone();
        response.from_cache = true;
        Some(response)
    }
}

#[derive(Debug, Default)]
pub(crate) struct ResponseCache {
    entries: HashMap<LexUrl, CacheEntry>,
}

impl ResponseCache {
    pub(crate) fn insert(&mut self, url: LexUrl, entry: CacheEntry) {
        self.entries.insert(url, entry);
    }

    pub(crate) fn get_fresh(
        &mut self,
        url: &LexUrl,
        now: SystemTime,
    ) -> Option<NavigationResponse> {
        let response = self.entries.get(url)?.fresh_response(now);
        if response.is_none() {
            self.entries.remove(url);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use reqwest::header::{HeaderMap, HeaderValue, CACHE_CONTROL};

    use super::{CacheEntry, ResponseCache};
    use crate::{LexUrl, NavigationResponse};

    fn response(cache_control: &'static str) -> NavigationResponse {
        let mut headers = HeaderMap::new();
        headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache_control));
        NavigationResponse {
            status: 200,
            final_url: LexUrl::parse("https://example.test/").unwrap(),
            headers,
            body: b"cached".to_vec(),
            redirect_chain: Vec::new(),
            from_cache: false,
        }
    }

    #[test]
    fn fresh_max_age_entry_is_reused_then_expires() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let url = LexUrl::parse("https://example.test/").unwrap();
        let entry = CacheEntry::from_response(&response("public, max-age=60"), now).unwrap();
        let mut cache = ResponseCache::default();
        cache.insert(url.clone(), entry);
        assert!(cache.get_fresh(&url, now + Duration::from_secs(59)).unwrap().from_cache);
        assert!(cache.get_fresh(&url, now + Duration::from_secs(61)).is_none());
    }

    #[test]
    fn no_store_and_missing_freshness_are_not_cached() {
        assert!(CacheEntry::from_response(&response("no-store, max-age=60"), SystemTime::now()).is_none());
        assert!(CacheEntry::from_response(&response("public"), SystemTime::now()).is_none());
    }
}
