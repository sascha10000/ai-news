-- Per-user article interactions: "like" and "read later".
-- Both are simple toggle relations keyed by (user_id, article_id). A row
-- exists iff the user has the article in that state; toggling off deletes
-- the row. Composite primary keys make duplicate inserts a no-op via
-- INSERT OR IGNORE and let DELETE be a single-row hit.
--
-- Cascades clean up automatically when either the user or the article
-- is removed.

CREATE TABLE article_likes (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    article_id INTEGER NOT NULL REFERENCES generated_articles(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, article_id)
);

CREATE INDEX idx_article_likes_article ON article_likes(article_id);

CREATE TABLE article_read_later (
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    article_id INTEGER NOT NULL REFERENCES generated_articles(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, article_id)
);

CREATE INDEX idx_article_read_later_article ON article_read_later(article_id);
