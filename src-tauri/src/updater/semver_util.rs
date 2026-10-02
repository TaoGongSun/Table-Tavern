//! 版本字串比較。能解析就用 SemVer（`0.9.0` < `0.10.0`）；解析不了就不當成可比較。

use std::cmp::Ordering;

pub(crate) fn versions_equal(left: &str, right: &str) -> bool {
    match (semver::Version::parse(left), semver::Version::parse(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

pub(crate) fn version_ord(left: &str, right: &str) -> Option<Ordering> {
    let left = semver::Version::parse(left).ok()?;
    let right = semver::Version::parse(right).ok()?;
    Some(left.cmp(&right))
}

/// 兩邊都是 SemVer，而且左邊比較舊。同版、較新、或解析不了，都不是。
pub(crate) fn is_older(left: &str, right: &str) -> bool {
    version_ord(left, right) == Some(Ordering::Less)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_orders_0_9_before_0_10() {
        assert_eq!(version_ord("0.9.0", "0.10.0"), Some(Ordering::Less));
        assert!(is_older("0.9.0", "0.10.0"));
        assert!(!is_older("0.10.0", "0.9.0"));
        assert!(!is_older("0.10.0", "0.10.0"));
        assert_ne!("0.9.0" < "0.10.0", true);
    }
}
