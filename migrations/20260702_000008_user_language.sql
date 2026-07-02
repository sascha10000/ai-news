-- Per-user target language for LLM-generated summaries.
-- NULL means "no preference": generation falls back to whatever the LLM
-- picks (typically the source language). Users set this from the
-- Operations area of their dashboard; admin uses the unscoped default.

ALTER TABLE users ADD COLUMN language TEXT;
