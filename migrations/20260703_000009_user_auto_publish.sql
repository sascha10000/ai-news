-- Per-user auto-publish preference. When enabled, newly ingested articles
-- owned by this user skip the draft queue and land in "published" directly,
-- mirroring the global admin `auto_publish` setting but scoped to the user's
-- own articles. 0 = off (default): new articles land in the user's drafts.

ALTER TABLE users ADD COLUMN auto_publish INTEGER NOT NULL DEFAULT 0;
