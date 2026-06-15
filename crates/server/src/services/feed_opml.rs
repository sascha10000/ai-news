/// Pre-validates an uploaded OPML 2.0 file (an XML export of RSS subscriptions
/// from readers like Feedly/NewsBlur/Reeder). Returns parsed feeds with their
/// top-level category attached, or per-outline errors so the user can fix the
/// file and re-upload in one pass instead of trial-and-error.
use serde::Deserialize;

pub use super::feed_import::ImportError;

#[derive(Debug)]
pub struct ParsedOpmlFeed {
    pub name: String,
    pub url: String,
    /// Top-level OPML category folder, if the feed lived inside one.
    /// Nested categories collapse to their root.
    pub category: Option<String>,
}

#[derive(Debug)]
pub struct ParsedOpml {
    pub feeds: Vec<ParsedOpmlFeed>,
}

// ---- OPML wire format (minimal subset of 2.0) ----

#[derive(Deserialize)]
struct Opml {
    body: Body,
}

#[derive(Deserialize)]
struct Body {
    #[serde(default, rename = "outline")]
    outlines: Vec<Outline>,
}

#[derive(Deserialize)]
struct Outline {
    #[serde(rename = "@xmlUrl")]
    xml_url: Option<String>,
    #[serde(rename = "@title")]
    title: Option<String>,
    #[serde(rename = "@text")]
    text: Option<String>,
    #[serde(default, rename = "outline")]
    children: Vec<Outline>,
}

pub fn parse_opml(xml: &str) -> Result<ParsedOpml, Vec<ImportError>> {
    let doc: Opml = match quick_xml::de::from_str(xml) {
        Ok(d) => d,
        Err(e) => {
            return Err(vec![ImportError {
                line: 0,
                raw: "<file>".to_string(),
                message: format!("OPML parse failed: {e}"),
            }]);
        }
    };

    let mut feeds = Vec::new();
    let mut errors = Vec::new();
    let mut seen_urls: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut counter: usize = 0;

    for outline in &doc.body.outlines {
        walk(
            outline,
            None,
            &mut feeds,
            &mut errors,
            &mut seen_urls,
            &mut counter,
        );
    }

    if errors.is_empty() {
        Ok(ParsedOpml { feeds })
    } else {
        Err(errors)
    }
}

fn walk(
    outline: &Outline,
    inherited_category: Option<&str>,
    feeds: &mut Vec<ParsedOpmlFeed>,
    errors: &mut Vec<ImportError>,
    seen_urls: &mut std::collections::HashSet<String>,
    counter: &mut usize,
) {
    let label = outline
        .title
        .as_deref()
        .or(outline.text.as_deref())
        .unwrap_or("")
        .trim();

    match outline.xml_url.as_deref().map(str::trim) {
        Some(url) if !url.is_empty() => {
            *counter += 1;
            let outline_no = *counter;
            let raw = if label.is_empty() {
                url.to_string()
            } else {
                format!("{label} ({url})")
            };

            if !is_http_url(url) {
                errors.push(ImportError {
                    line: outline_no,
                    raw,
                    message: "xmlUrl must start with http:// or https://".to_string(),
                });
                return;
            }

            let name = if !label.is_empty() {
                label.to_string()
            } else {
                match host_from_url(url) {
                    Some(h) => h,
                    None => {
                        errors.push(ImportError {
                            line: outline_no,
                            raw,
                            message: "outline has no title/text and no derivable host".to_string(),
                        });
                        return;
                    }
                }
            };

            if !seen_urls.insert(url.to_string()) {
                errors.push(ImportError {
                    line: outline_no,
                    raw,
                    message: format!("duplicate xmlUrl '{url}' earlier in file"),
                });
                return;
            }

            feeds.push(ParsedOpmlFeed {
                name,
                url: url.to_string(),
                category: inherited_category.map(str::to_string),
            });
        }
        _ => {
            // No xmlUrl — treat as category. Carry our own label down only if
            // we don't already have one inherited (nested categories collapse).
            let category = inherited_category.or(if label.is_empty() { None } else { Some(label) });
            for child in &outline.children {
                walk(child, category, feeds, errors, seen_urls, counter);
            }
        }
    }
}

fn is_http_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn host_from_url(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.");
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_opml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <body>
    <outline type="rss" text="HN" xmlUrl="https://hnrss.org/frontpage"/>
    <outline type="rss" title="Verge" xmlUrl="https://www.theverge.com/rss/index.xml"/>
  </body>
</opml>"#;
        let parsed = parse_opml(xml).unwrap();
        assert_eq!(parsed.feeds.len(), 2);
        assert_eq!(parsed.feeds[0].name, "HN");
        assert_eq!(parsed.feeds[0].category, None);
        assert_eq!(parsed.feeds[1].name, "Verge");
    }

    #[test]
    fn parses_categorized_opml() {
        let xml = r#"<opml version="2.0">
  <body>
    <outline text="Tech">
      <outline type="rss" text="HN" xmlUrl="https://hnrss.org/"/>
      <outline type="rss" text="Verge" xmlUrl="https://verge.example/rss"/>
    </outline>
    <outline type="rss" text="Loose" xmlUrl="https://loose.example/rss"/>
  </body>
</opml>"#;
        let parsed = parse_opml(xml).unwrap();
        assert_eq!(parsed.feeds.len(), 3);
        assert_eq!(parsed.feeds[0].category.as_deref(), Some("Tech"));
        assert_eq!(parsed.feeds[1].category.as_deref(), Some("Tech"));
        assert_eq!(parsed.feeds[2].category, None);
    }

    #[test]
    fn nested_categories_collapse_to_root() {
        let xml = r#"<opml><body>
            <outline text="Tech">
                <outline text="AI">
                    <outline type="rss" text="OpenAI" xmlUrl="https://openai.example/feed"/>
                </outline>
            </outline>
        </body></opml>"#;
        let parsed = parse_opml(xml).unwrap();
        assert_eq!(parsed.feeds.len(), 1);
        assert_eq!(parsed.feeds[0].category.as_deref(), Some("Tech"));
    }

    #[test]
    fn collects_outline_errors() {
        let xml = r#"<opml><body>
            <outline type="rss" text="Good" xmlUrl="https://good.example/feed"/>
            <outline type="rss" text="BadScheme" xmlUrl="ftp://nope.example/feed"/>
            <outline type="rss" xmlUrl=""/>
            <outline type="rss" text="Dup" xmlUrl="https://good.example/feed"/>
        </body></opml>"#;
        let errs = parse_opml(xml).unwrap_err();
        // BadScheme (1 error), empty xmlUrl is treated as no xmlUrl => no error, just skipped,
        // Dup (1 error) — so 2 errors total.
        assert_eq!(errs.len(), 2);
        assert!(errs[0].message.contains("http://"));
        assert!(errs[1].message.contains("duplicate"));
    }

    #[test]
    fn rejects_malformed_xml() {
        let xml = "<opml><body><outline";
        let errs = parse_opml(xml).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.starts_with("OPML parse failed"));
    }

    #[test]
    fn name_fallback_title_text_host() {
        let xml = r#"<opml><body>
            <outline type="rss" title="From Title" xmlUrl="https://a.example/rss"/>
            <outline type="rss" text="From Text" xmlUrl="https://b.example/rss"/>
            <outline type="rss" xmlUrl="https://www.c.example/rss"/>
        </body></opml>"#;
        let parsed = parse_opml(xml).unwrap();
        assert_eq!(parsed.feeds[0].name, "From Title");
        assert_eq!(parsed.feeds[1].name, "From Text");
        assert_eq!(parsed.feeds[2].name, "c.example");
    }

    #[test]
    fn empty_body_is_ok() {
        let xml = r#"<opml version="2.0"><body/></opml>"#;
        let parsed = parse_opml(xml).unwrap();
        assert!(parsed.feeds.is_empty());
    }
}
