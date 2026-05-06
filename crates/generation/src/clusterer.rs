use ai_news_core::PendingSource;
use chrono::{DateTime, FixedOffset};
use std::collections::{HashMap, HashSet};

const MAX_CLUSTER_DATE_GAP_DAYS: i64 = 14;

pub fn cluster_articles(articles: &[PendingSource]) -> Vec<Vec<&PendingSource>> {
    if articles.is_empty() {
        return vec![];
    }

    let tokenized: Vec<HashSet<String>> = articles.iter().map(|a| tokenize(&a.title)).collect();
    let dates: Vec<Option<DateTime<FixedOffset>>> =
        articles.iter().map(|a| parse_date(&a.published_at)).collect();

    let threshold = 0.15;
    let mut used: HashSet<usize> = HashSet::new();
    let mut clusters: Vec<Vec<&PendingSource>> = Vec::new();

    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..articles.len() {
        for j in (i + 1)..articles.len() {
            let sim = jaccard(&tokenized[i], &tokenized[j]);
            if sim < threshold {
                continue;
            }
            if let (Some(di), Some(dj)) = (dates[i], dates[j]) {
                if (di - dj).num_days().abs() > MAX_CLUSTER_DATE_GAP_DAYS {
                    continue;
                }
            }
            adj.entry(i).or_default().push(j);
            adj.entry(j).or_default().push(i);
        }
    }

    let mut remaining: Vec<usize> = (0..articles.len()).collect();
    remaining.sort_by(|a, b| {
        let count_b = adj.get(b).map(|v| v.len()).unwrap_or(0);
        let count_a = adj.get(a).map(|v| v.len()).unwrap_or(0);
        count_b.cmp(&count_a)
    });

    for idx in remaining {
        if used.contains(&idx) {
            continue;
        }

        let mut cluster = vec![idx];
        used.insert(idx);

        if let Some(neighbors) = adj.get(&idx) {
            for &neighbor in neighbors {
                if !used.contains(&neighbor) && cluster.len() < 8 {
                    cluster.push(neighbor);
                    used.insert(neighbor);
                }
            }
        }

        if cluster.len() >= 2 {
            let mut members: Vec<&PendingSource> =
                cluster.iter().map(|&i| &articles[i]).collect();
            members.sort_by(|a, b| {
                let da = parse_date(&a.published_at);
                let db = parse_date(&b.published_at);
                match (da, db) {
                    (Some(x), Some(y)) => y.cmp(&x),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
            clusters.push(members);
        }
    }

    clusters
}

fn parse_date(s: &Option<String>) -> Option<DateTime<FixedOffset>> {
    s.as_deref().and_then(|d| DateTime::parse_from_rfc3339(d).ok())
}

fn tokenize(text: &str) -> HashSet<String> {
    let stopwords: HashSet<&str> = [
        "the", "a", "an", "is", "are", "was", "were", "in", "on", "at", "to", "for", "of", "and",
        "or", "but", "with", "by", "from", "as", "it", "its", "that", "this", "has", "have", "had",
        "be", "been", "will", "would", "could", "should", "not", "no", "do", "does", "did", "can",
        "may", "might", "shall",
    ]
    .into_iter()
    .collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !stopwords.contains(w))
        .map(|w| w.to_string())
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    intersection / union
}
