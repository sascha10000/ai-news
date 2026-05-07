/// Pre-validates a feeds CSV (one feed per line, format: `name;url`).
/// Returns parsed (name, url) pairs on success, or per-line errors so the
/// user can fix the whole file in one pass instead of trial-and-error.

#[derive(Debug)]
pub struct ImportError {
    pub line: usize,
    pub raw: String,
    pub message: String,
}

#[derive(Debug)]
pub struct ParsedFeed {
    pub name: String,
    pub url: String,
}

pub fn parse_csv(input: &str) -> Result<Vec<ParsedFeed>, Vec<ImportError>> {
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (name, url) = match trimmed.split_once(';') {
            Some(pair) => pair,
            None => {
                errors.push(ImportError {
                    line: line_no,
                    raw: trimmed.to_string(),
                    message: "missing ';' separator (expected 'name;url')".to_string(),
                });
                continue;
            }
        };

        let name = name.trim();
        let url = url.trim();

        // A row that's only punctuation/whitespace (e.g. ";" or "  ;  ")
        // is treated like a blank line — silently skipped, not flagged.
        if name.is_empty() && url.is_empty() {
            continue;
        }

        let mut line_errs = Vec::new();
        if name.is_empty() {
            line_errs.push("name is empty");
        }
        if url.is_empty() {
            line_errs.push("URL is empty");
        } else if !is_http_url(url) {
            line_errs.push("URL must start with http:// or https://");
        }

        if !line_errs.is_empty() {
            errors.push(ImportError {
                line: line_no,
                raw: trimmed.to_string(),
                message: line_errs.join("; "),
            });
            continue;
        }

        if !seen_urls.insert(url.to_string()) {
            errors.push(ImportError {
                line: line_no,
                raw: trimmed.to_string(),
                message: format!("duplicate URL '{url}' earlier in CSV"),
            });
            continue;
        }

        parsed.push(ParsedFeed {
            name: name.to_string(),
            url: url.to_string(),
        });
    }

    if errors.is_empty() {
        Ok(parsed)
    } else {
        Err(errors)
    }
}

fn is_http_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_csv() {
        let csv = "HN;https://hnrss.org/\nVerge;https://www.theverge.com/rss/index.xml";
        let r = parse_csv(csv).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].name, "HN");
    }

    #[test]
    fn collects_all_errors() {
        let csv = "Good;https://ok.example/\nbroken-line\n;https://noname.example/\nBadUrl;not-a-url";
        let errs = parse_csv(csv).unwrap_err();
        assert_eq!(errs.len(), 3);
        assert_eq!(errs[0].line, 2);
        assert!(errs[0].message.contains("missing ';'"));
        assert_eq!(errs[1].line, 3);
        assert!(errs[1].message.contains("name is empty"));
        assert_eq!(errs[2].line, 4);
        assert!(errs[2].message.contains("http://"));
    }

    #[test]
    fn detects_duplicate_url() {
        let csv = "A;https://x.example/\nB;https://x.example/";
        let errs = parse_csv(csv).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line, 2);
        assert!(errs[0].message.contains("duplicate"));
    }

    #[test]
    fn skips_blank_lines() {
        let csv = "\n\nA;https://x.example/\n\n";
        let r = parse_csv(csv).unwrap();
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn skips_punctuation_only_rows() {
        // A row that's just a stray semicolon (or whitespace + semicolon)
        // is treated like a blank line — silently skipped, not an error.
        let csv = "A;https://a.example/\n;\n   ;   \nB;https://b.example/";
        let r = parse_csv(csv).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].name, "A");
        assert_eq!(r[1].name, "B");
    }
}
