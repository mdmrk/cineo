use semver::Version;
use serde_json::Value;

pub const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/mdmrk/cineo/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReleaseError {
    #[error("response is not JSON: {0}")]
    Json(String),
    #[error("response has no `tag_name`")]
    MissingTag,
    #[error("`tag_name` is not a version")]
    InvalidTag,
}

pub fn parse_release(body: &[u8]) -> Result<Version, ReleaseError> {
    let json: Value =
        serde_json::from_slice(body).map_err(|err| ReleaseError::Json(err.to_string()))?;
    let tag = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or(ReleaseError::MissingTag)?;
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).map_err(|_| ReleaseError::InvalidTag)
}

pub fn release_page(version: &Version) -> String {
    format!("https://github.com/mdmrk/cineo/releases/tag/v{version}")
}

pub(crate) fn is_newer(latest: &Version) -> bool {
    Version::parse(env!("CARGO_PKG_VERSION")).is_ok_and(|current| *latest > current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_name_is_the_version_with_or_without_a_v() {
        assert_eq!(
            parse_release(br#"{"tag_name":"v0.4.0","name":"v0.4.0"}"#),
            Ok(Version::new(0, 4, 0))
        );
        assert_eq!(
            parse_release(br#"{"tag_name":"1.2.3-beta.1"}"#),
            Version::parse("1.2.3-beta.1").map_err(|_| ReleaseError::InvalidTag)
        );
    }

    #[test]
    fn a_response_without_a_version_is_an_error() {
        assert_eq!(
            parse_release(br#"{"name":"x"}"#),
            Err(ReleaseError::MissingTag)
        );
        assert_eq!(
            parse_release(br#"{"tag_name":"latest"}"#),
            Err(ReleaseError::InvalidTag)
        );
        assert!(matches!(
            parse_release(b"<html>"),
            Err(ReleaseError::Json(_))
        ));
    }

    #[test]
    fn only_a_higher_version_is_newer() {
        assert!(is_newer(&Version::new(999, 0, 0)));
        assert!(!is_newer(&Version::new(0, 0, 1)));
        assert!(!is_newer(
            &Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version::new(0, 0, 0))
        ));
    }

    #[test]
    fn the_release_page_is_on_github() {
        assert_eq!(
            release_page(&Version::new(0, 4, 0)),
            "https://github.com/mdmrk/cineo/releases/tag/v0.4.0"
        );
    }
}
