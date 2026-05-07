-- Multi-user accounts.
-- Adds users table; attaches optional user ownership to feeds, lists,
-- generated_articles, and sessions. NULL = global / admin-owned.
--
-- NOTE: this migration rebuilds the `feeds` and `lists` tables to switch
-- their UNIQUE constraints from global to per-user. Per the SQLite manual
-- ("Making Other Kinds Of Table Schema Changes"), this requires
-- foreign_keys = OFF for the duration. The migration runner in db.rs
-- opens its connection with foreign_keys disabled and re-enables them on
-- the pool afterward, with PRAGMA foreign_key_check to verify integrity.

CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    public INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_users_username ON users(username);

ALTER TABLE generated_articles
    ADD COLUMN user_id INTEGER REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE sessions
    ADD COLUMN user_id INTEGER REFERENCES users(id) ON DELETE CASCADE;

CREATE INDEX idx_generated_articles_user ON generated_articles(user_id);
CREATE INDEX idx_generated_articles_public_feed
    ON generated_articles(user_id, status, published_at DESC);

-- Save feed_lists; we have to drop it to rebuild the parent tables.
CREATE TABLE feed_lists_save (
    feed_id INTEGER NOT NULL,
    list_id INTEGER NOT NULL
);
INSERT INTO feed_lists_save (feed_id, list_id) SELECT feed_id, list_id FROM feed_lists;
DROP TABLE feed_lists;

-- Rebuild lists: was UNIQUE(name) and UNIQUE(slug) globally; needs to be
-- per-user so two users can have a list named "AI" without collision.
CREATE TABLE lists_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(user_id, name),
    UNIQUE(user_id, slug)
);
INSERT INTO lists_new (id, name, slug, created_at)
    SELECT id, name, slug, created_at FROM lists;
DROP TABLE lists;
ALTER TABLE lists_new RENAME TO lists;
CREATE INDEX idx_lists_user ON lists(user_id);

-- Rebuild feeds: was UNIQUE(url) globally; same problem.
CREATE TABLE feeds_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    active BOOLEAN NOT NULL DEFAULT 1,
    fetch_interval_minutes INTEGER NOT NULL DEFAULT 60,
    last_fetched_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(user_id, url)
);
INSERT INTO feeds_new (id, name, url, active, fetch_interval_minutes, last_fetched_at, created_at)
    SELECT id, name, url, active, fetch_interval_minutes, last_fetched_at, created_at FROM feeds;
DROP TABLE feeds;
ALTER TABLE feeds_new RENAME TO feeds;
CREATE INDEX idx_feeds_user ON feeds(user_id);

-- Restore feed_lists with FKs to the rebuilt parent tables.
CREATE TABLE feed_lists (
    feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    list_id INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    PRIMARY KEY (feed_id, list_id)
);
INSERT INTO feed_lists (feed_id, list_id) SELECT feed_id, list_id FROM feed_lists_save;
DROP TABLE feed_lists_save;
CREATE INDEX idx_feed_lists_list ON feed_lists(list_id);
