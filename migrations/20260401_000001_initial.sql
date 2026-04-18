CREATE TABLE feeds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    url TEXT NOT NULL UNIQUE,
    active BOOLEAN NOT NULL DEFAULT 1,
    fetch_interval_minutes INTEGER NOT NULL DEFAULT 60,
    last_fetched_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE source_articles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    guid TEXT,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    author TEXT,
    content TEXT NOT NULL,
    summary TEXT,
    published_at TEXT,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(feed_id, url)
);

CREATE TABLE generated_articles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    summary TEXT,
    topic_label TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    generated_at TEXT NOT NULL DEFAULT (datetime('now')),
    published_at TEXT
);

CREATE TABLE generated_sentences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    generated_article_id INTEGER NOT NULL REFERENCES generated_articles(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    content TEXT NOT NULL
);

CREATE TABLE sentence_citations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sentence_id INTEGER NOT NULL REFERENCES generated_sentences(id) ON DELETE CASCADE,
    source_article_id INTEGER NOT NULL REFERENCES source_articles(id) ON DELETE CASCADE
);

CREATE INDEX idx_source_articles_feed ON source_articles(feed_id);
CREATE INDEX idx_source_articles_published ON source_articles(published_at);
CREATE INDEX idx_generated_sentences_article ON generated_sentences(generated_article_id);
CREATE INDEX idx_sentence_citations_sentence ON sentence_citations(sentence_id);
CREATE INDEX idx_generated_articles_status ON generated_articles(status);
