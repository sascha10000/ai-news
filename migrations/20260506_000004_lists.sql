CREATE TABLE lists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    slug TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE feed_lists (
    feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    list_id INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    PRIMARY KEY (feed_id, list_id)
);

CREATE INDEX idx_feed_lists_list ON feed_lists(list_id);

ALTER TABLE generated_articles ADD COLUMN list_id INTEGER REFERENCES lists(id) ON DELETE SET NULL;

CREATE INDEX idx_generated_articles_list ON generated_articles(list_id);
