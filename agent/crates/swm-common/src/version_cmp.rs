use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
enum VersionPart {
    Numeric(u64),
    Text(String),
}

fn tokenize_version(v: &str) -> Vec<VersionPart> {
    let trimmed = v.trim().trim_start_matches(['v', 'V']);
    // Strip Debian epoch prefix when comparing standard parts only if needed, or preserve epoch as leading number
    let mut parts = Vec::new();
    let mut cur_digits = String::new();
    let mut cur_text = String::new();

    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            if !cur_text.is_empty() {
                parts.push(VersionPart::Text(std::mem::take(&mut cur_text).to_ascii_lowercase()));
            }
            cur_digits.push(ch);
        } else if ch == '.' || ch == '-' || ch == '_' || ch == '+' || ch == ':' || ch == '~' {
            if !cur_digits.is_empty() {
                let n = cur_digits.parse::<u64>().unwrap_or(0);
                cur_digits.clear();
                parts.push(VersionPart::Numeric(n));
            }
            if !cur_text.is_empty() {
                parts.push(VersionPart::Text(std::mem::take(&mut cur_text).to_ascii_lowercase()));
            }
        } else {
            if !cur_digits.is_empty() {
                let n = cur_digits.parse::<u64>().unwrap_or(0);
                cur_digits.clear();
                parts.push(VersionPart::Numeric(n));
            }
            cur_text.push(ch);
        }
    }

    if !cur_digits.is_empty() {
        parts.push(VersionPart::Numeric(cur_digits.parse::<u64>().unwrap_or(0)));
    }
    if !cur_text.is_empty() {
        parts.push(VersionPart::Text(cur_text.to_ascii_lowercase()));
    }

    parts
}

/// Compares two software/OS version strings numerically & lexically.
/// Trailing `.0` segments are treated as equal (`"1.2"` == `"1.2.0"`).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let ta = tokenize_version(a);
    let tb = tokenize_version(b);
    let max_len = ta.len().max(tb.len());

    for idx in 0..max_len {
        let pa = ta.get(idx).cloned().unwrap_or(VersionPart::Numeric(0));
        let pb = tb.get(idx).cloned().unwrap_or(VersionPart::Numeric(0));

        let ord = match (pa, pb) {
            (VersionPart::Numeric(na), VersionPart::Numeric(nb)) => na.cmp(&nb),
            (VersionPart::Text(sa), VersionPart::Text(sb)) => sa.cmp(&sb),
            // A numeric release segment is greater than a pre-release text tag (e.g. 1.0.0 > 1.0.alpha)
            (VersionPart::Numeric(_), VersionPart::Text(_)) => Ordering::Greater,
            (VersionPart::Text(_), VersionPart::Numeric(_)) => Ordering::Less,
        };

        if ord != Ordering::Equal {
            return ord;
        }
    }

    Ordering::Equal
}

pub fn version_satisfies(installed: &str, expected: &str, operator: &str) -> bool {
    let ord = compare_versions(installed, expected);
    match operator.trim().to_ascii_lowercase().as_str() {
        "eq" | "==" | "=" => ord == Ordering::Equal,
        "gt" | ">" => ord == Ordering::Greater,
        "lt" | "<" => ord == Ordering::Less,
        "lte" | "<=" => ord != Ordering::Greater,
        "exists" => !installed.trim().is_empty(),
        // Default: "gte" / ">="
        _ => ord != Ordering::Less,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_numeric_and_prerelease() {
        assert_eq!(compare_versions("24.08", "24.07"), Ordering::Greater);
        assert_eq!(compare_versions("1.2.0", "1.2"), Ordering::Equal);
        assert_eq!(compare_versions("1.10.0", "1.2.9"), Ordering::Greater);
        assert_eq!(compare_versions("v2.0.0", "2.0.0-rc1"), Ordering::Greater);
        assert!(version_satisfies("128.0.2", "128.0", "gte"));
        assert!(!version_satisfies("127.9.0", "128.0", "gte"));
        assert!(version_satisfies("1.4.0", "1.4", "eq"));
    }
}
