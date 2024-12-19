use libs::percent_encoding::percent_decode;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use errors::{anyhow, Result};

/// Result of a successful resolution of an internal link.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ResolvedInternalLink {
    /// Resolved link target, as absolute URL address.
    pub permalink: String,
    /// Internal path to the .md file, without the leading `@/`.
    pub md_path: String,
    /// Optional anchor target.
    /// We can check whether it exists only after all the markdown markdown is done.
    pub anchor: Option<String>,
}

/// Resolves an internal link (of the `@/posts/something.md#hey` sort) to its absolute link and
/// returns the path + anchor as well
pub fn resolve_internal_link(
    link: &str,
    permalinks: &HashMap<String, String>,
) -> Result<ResolvedInternalLink> {
    let (decoded, anchor) = get_permalink_key_from_link(&link);
    let target =
        permalinks.get(&decoded).ok_or_else(|| anyhow!("Relative link {} not found.", link))?;

    Ok(ResolvedInternalLink {
        permalink: combine_anchor(target, anchor),
        md_path: decoded,
        anchor: anchor.map(|a| a.to_owned()),
    })
}

/// Converts a link into a canonical key for the permalinks array
pub fn get_permalink_key_from_link(link: &str) -> (String, Option<&str>) {
    // First we remove the @/ since that's zola specific
    let clean_link = link.strip_prefix("@/").unwrap_or(link);

    // Then we remove any potential anchor
    let (clean_link_no_anchor, anchor) = extract_anchor(&clean_link);

    // If we have slugification turned off, we might end up with some escaped characters so we need
    // to decode them first
    let decoded = percent_decode(clean_link_no_anchor.as_bytes()).decode_utf8_lossy().to_string();
    (decoded, anchor)
}

/// Takes a link and finds out if it is captured in the permalinks map
pub fn is_link_internal_page(link: &str, permalinks: &HashMap<String, String>) -> bool {
    let (key, _) = get_permalink_key_from_link(&link);
    permalinks.contains_key(&key)
}

/// Takes a link and splits out the anchor piece, if it exists
pub fn extract_anchor(link: &str) -> (&str, Option<&str>) {
    if let Some(pos) = link.find('#') {
        let (base, anchor) = link.split_at(pos);
        (base, Some(&anchor[1..]))
    } else {
        (link, None)
    }
}
pub fn combine_anchor(link: &str, anchor: Option<&str>) -> String {
    match anchor {
        Some(a) => format!("{}#{}", link, a),
        None => link.to_owned(),
    }
}

pub fn link_has_protocol_or_zola(link: &str) -> bool {
    ["http://", "https://", "mailto:", "ftp://", "file://", "@/"]
        .iter()
        .any(|&proto| link.starts_with(proto))
}

/// Takes a relative path with no leading slash or with leading ./ or ../and normalizes
pub fn canonicalize_relative_path(link: &str, current_page_path: Option<&str>) -> String {
    // Make sure external links with protocols are left untouched and explicit zola links are ignored, too
    if link_has_protocol_or_zola(link) {
        return link.to_owned();
    }

    // Process everything whether it starts with an "@" or not
    let linkpath = Path::new(link);
    let basepath = Path::new(
        current_page_path
            .map(|p| {
                if p.ends_with(".md") {
                    // The base path as far as a markdown file is concerned is the parent of the md file
                    // so we delete out everything after the last slash
                    if let Some(pos) = p.rfind('/') {
                        &p[..pos]
                    } else {
                        p
                    }
                } else {
                    p
                }
            })
            .unwrap_or(""),
    );
    let combined_path =
        if linkpath.is_absolute() { linkpath.to_path_buf() } else { basepath.join(linkpath) };
    let canon_path = combined_path.canonicalize().unwrap_or_else(|_| {
        let mut resolved = PathBuf::new();
        for component in combined_path.components() {
            match component {
                Component::RootDir => resolved.push(component),
                Component::ParentDir => {
                    resolved.pop();
                    ()
                }
                Component::CurDir => {} // skip
                Component::Normal(_) => resolved.push(component),
                Component::Prefix(_) => resolved.push(component),
            };
        }
        resolved
    });
    canon_path
        .to_str()
        .map(|s| s.to_owned())
        .ok_or(std::io::Error::new(std::io::ErrorKind::Other, "Invalid UTF-8"))
        .unwrap_or_else(|_| link.to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn can_resolve_valid_internal_link() {
        let mut permalinks = HashMap::new();
        permalinks.insert("pages/about.md".to_string(), "https://vincent.is/about".to_string());
        let res = resolve_internal_link("@/pages/about.md", &permalinks).unwrap();
        assert_eq!(res.permalink, "https://vincent.is/about");
    }

    #[test]
    fn can_resolve_valid_root_internal_link() {
        let mut permalinks = HashMap::new();
        permalinks.insert("about.md".to_string(), "https://vincent.is/about".to_string());
        let res = resolve_internal_link("@/about.md", &permalinks).unwrap();
        assert_eq!(res.permalink, "https://vincent.is/about");
    }

    #[test]
    fn can_resolve_internal_links_with_anchors() {
        let mut permalinks = HashMap::new();
        permalinks.insert("pages/about.md".to_string(), "https://vincent.is/about".to_string());
        let res = resolve_internal_link("@/pages/about.md#hello", &permalinks).unwrap();
        assert_eq!(res.permalink, "https://vincent.is/about#hello");
        assert_eq!(res.md_path, "pages/about.md".to_string());
        assert_eq!(res.anchor, Some("hello".to_string()));
    }

    #[test]
    fn can_resolve_escaped_internal_links() {
        let mut permalinks = HashMap::new();
        permalinks.insert(
            "pages/about space.md".to_string(),
            "https://vincent.is/about%20space/".to_string(),
        );
        let res = resolve_internal_link("@/pages/about%20space.md#hello", &permalinks).unwrap();
        assert_eq!(res.permalink, "https://vincent.is/about%20space/#hello");
        assert_eq!(res.md_path, "pages/about space.md".to_string());
        assert_eq!(res.anchor, Some("hello".to_string()));
    }

    #[test]
    fn errors_resolve_inexistant_internal_link() {
        let res = resolve_internal_link("@/pages/about.md#hello", &HashMap::new());
        assert!(res.is_err());
    }

    #[test]
    fn test_get_permalink_key_from_link() {
        assert_eq!(get_permalink_key_from_link("@/some/path"), ("some/path".to_string(), None));
        assert_eq!(
            get_permalink_key_from_link("@/some/path#anchor"),
            ("some/path".to_string(), Some("anchor"))
        );
        assert_eq!(get_permalink_key_from_link("some/path"), ("some/path".to_string(), None));
        assert_eq!(
            get_permalink_key_from_link("/some/path#anchor"),
            ("/some/path".to_string(), Some("anchor"))
        );
    }

    #[test]
    fn test_is_link_internal_page() {
        let mut permalinks = HashMap::new();
        permalinks.insert("some/path".to_string(), "some/path".to_string());

        assert!(is_link_internal_page("@/some/path", &permalinks));
        assert!(!is_link_internal_page("@/other/path", &permalinks));
    }

    #[test]
    fn test_extract_anchor() {
        assert_eq!(extract_anchor("some/path#anchor"), ("some/path", Some("anchor")));
        assert_eq!(extract_anchor("some/path"), ("some/path", None));
    }

    #[test]
    fn test_canonicalize_relative_path() {
        assert_eq!(
            canonicalize_relative_path("some/path", Some("/base")),
            "/base/some/path".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("./some/path", Some("/base")),
            "/base/some/path".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("../some/path", Some("/base")),
            "/some/path".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("/some/./path", Some("/base")),
            "/some/path".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("some/../../path", Some("/base")),
            "/path".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("/some/.././path", Some("/base")),
            "/path".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("/some/.././path/", Some("/base")),
            "/path".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("/some/.././path#xyz", Some("/base")),
            "/path#xyz".to_string()
        );
        assert_eq!(
            canonicalize_relative_path("../some/path", Some("/base/second/file.md")),
            "/base/some/path".to_string()
        );
    }
}
