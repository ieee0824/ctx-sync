//! Clone URL of the context Gist.

use crate::state::Protocol;

/// Environment variable that replaces the Gist URL (used by tests to point
/// at a local bare repository). It is read by `Runtime`, not here.
pub const REMOTE_URL_OVERRIDE_ENV: &str = "CTX_SYNC_REMOTE_URL_OVERRIDE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSpec {
    pub gist_id: String,
    pub protocol: Protocol,
    /// When set, used as the URL as-is.
    pub url_override: Option<String>,
}

impl RemoteSpec {
    pub fn new(gist_id: impl Into<String>, protocol: Protocol) -> Self {
        Self {
            gist_id: gist_id.into(),
            protocol,
            url_override: None,
        }
    }

    pub fn with_url_override(mut self, url: Option<String>) -> Self {
        self.url_override = url;
        self
    }

    pub fn url(&self) -> String {
        if let Some(url) = &self.url_override {
            return url.clone();
        }
        match self.protocol {
            Protocol::Https => format!("https://gist.github.com/{}.git", self.gist_id),
            Protocol::Ssh => format!("git@gist.github.com:{}.git", self.gist_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_url() {
        assert_eq!(
            RemoteSpec::new("abc", Protocol::Https).url(),
            "https://gist.github.com/abc.git"
        );
    }

    #[test]
    fn ssh_url() {
        assert_eq!(
            RemoteSpec::new("abc", Protocol::Ssh).url(),
            "git@gist.github.com:abc.git"
        );
    }

    #[test]
    fn override_wins() {
        let spec =
            RemoteSpec::new("abc", Protocol::Ssh).with_url_override(Some("/tmp/remote.git".into()));
        assert_eq!(spec.url(), "/tmp/remote.git");
        assert_eq!(
            spec.with_url_override(None).url(),
            "git@gist.github.com:abc.git"
        );
    }
}
