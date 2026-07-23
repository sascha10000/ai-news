-- Per-user API keys for the MCP endpoint. key_hash is an UNSALTED sha256 hex
-- of the full key: keys are 128-bit random (unlike passwords), so salting adds
-- nothing and a deterministic hash allows direct lookup by hash. The plaintext
-- key is shown to the user exactly once at generation time.
CREATE TABLE api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at TEXT
);
