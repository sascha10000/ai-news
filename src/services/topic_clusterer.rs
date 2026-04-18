use crate::models::source_article::SourceArticle;
use std::collections::{HashMap, HashSet};

pub fn cluster_articles(articles: &[SourceArticle]) -> Vec<Vec<&SourceArticle>> {
    if articles.is_empty() {
        return vec![];
    }

    let tokenized: Vec<HashSet<String>> = articles
        .iter()
        .map(|a| tokenize(&a.title))
        .collect();

    let threshold = 0.15;
    let mut used: HashSet<usize> = HashSet::new();
    let mut clusters: Vec<Vec<&SourceArticle>> = Vec::new();

    // Build adjacency based on Jaccard similarity
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..articles.len() {
        for j in (i + 1)..articles.len() {
            let sim = jaccard(&tokenized[i], &tokenized[j]);
            if sim >= threshold {
                adj.entry(i).or_default().push(j);
                adj.entry(j).or_default().push(i);
            }
        }
    }

    // Greedy clustering: pick node with most neighbors, form cluster
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
            clusters.push(cluster.iter().map(|&i| &articles[i]).collect());
        }
    }

    clusters
}

fn tokenize(text: &str) -> HashSet<String> {
    let stopwords: HashSet<&str> = [
        "the", "a", "an", "is", "are", "was", "were", "in", "on", "at", "to", "for",
        "of", "and", "or", "but", "with", "by", "from", "as", "it", "its", "that",
        "this", "has", "have", "had", "be", "been", "will", "would", "could", "should",
        "not", "no", "do", "does", "did", "can", "may", "might", "shall",
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
