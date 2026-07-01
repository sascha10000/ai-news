-- Global admin-level settings, stored as a simple key/value table.
-- Currently only used for auto_publish (whether generated global
-- articles skip the draft queue and land in "published" directly).

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO app_settings (key, value) VALUES ('auto_publish', 'false');
