use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

const RESERVED_USERNAMES: &[&str] = &[
    "admin", "api", "static", "user", "users", "register", "login", "logout",
    "me", "system", "root", "support", "help", "about",
];

#[derive(sqlx::FromRow, Clone, Debug)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub public: bool,
    pub created_at: String,
    pub language: Option<String>,
    pub auto_publish: bool,
}

// Language table lives in `ai_news_core` so client & server stay in sync;
// re-export here for callers that only import from the model module.
pub use ai_news_core::{language_label, SUPPORTED_LANGUAGES};

#[derive(thiserror::Error, Debug)]
pub enum UserError {
    #[error("Username must be 3-32 chars, lowercase a-z, 0-9, dash, underscore")]
    InvalidUsername,

    #[error("Username '{0}' is reserved")]
    ReservedUsername(String),

    #[error("Username already taken")]
    UsernameTaken,

    #[error("Password must be at least 8 characters")]
    WeakPassword,

    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl User {
    pub async fn create(
        pool: &SqlitePool,
        username: &str,
        password: &str,
    ) -> Result<i64, UserError> {
        let username = normalize_username(username)?;
        if password.len() < 8 {
            return Err(UserError::WeakPassword);
        }

        let hash = hash_password(password);
        match sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
            .bind(&username)
            .bind(&hash)
            .execute(pool)
            .await
        {
            Ok(result) => Ok(result.last_insert_rowid()),
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
                Err(UserError::UsernameTaken)
            }
            Err(e) => Err(UserError::Db(e)),
        }
    }

    pub async fn all_brief(
        pool: &SqlitePool,
    ) -> Result<Vec<(i64, String, Option<String>)>, sqlx::Error> {
        sqlx::query_as::<_, (i64, String, Option<String>)>(
            "SELECT id, username, language FROM users ORDER BY username",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "SELECT id, username, public, created_at, language, auto_publish FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
    }

    pub async fn by_username(
        pool: &SqlitePool,
        username: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "SELECT id, username, public, created_at, language, auto_publish FROM users WHERE username = ?",
        )
        .bind(username.to_lowercase())
        .fetch_optional(pool)
        .await
    }

    pub async fn authenticate(
        pool: &SqlitePool,
        username: &str,
        password: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        let row: Option<(i64, String, bool, String, Option<String>, bool, String)> = sqlx::query_as(
            "SELECT id, username, public, created_at, language, auto_publish, password_hash FROM users WHERE username = ?",
        )
        .bind(username.to_lowercase())
        .fetch_optional(pool)
        .await?;

        Ok(row.and_then(|(id, username, public, created_at, language, auto_publish, hash)| {
            if verify_password(password, &hash) {
                Some(User { id, username, public, created_at, language, auto_publish })
            } else {
                None
            }
        }))
    }

    pub async fn set_public(
        pool: &SqlitePool,
        id: i64,
        public: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET public = ? WHERE id = ?")
            .bind(public)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// `language` is stored as the raw code (e.g. "en", "de") or NULL for
    /// "no preference". Callers must have already validated the code against
    /// `SUPPORTED_LANGUAGES`; passing None clears the preference.
    pub async fn set_language(
        pool: &SqlitePool,
        id: i64,
        language: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET language = ? WHERE id = ?")
            .bind(language)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn language_of(
        pool: &SqlitePool,
        id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT language FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        Ok(row.and_then(|r| r.0))
    }

    pub async fn set_auto_publish(
        pool: &SqlitePool,
        id: i64,
        enabled: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET auto_publish = ? WHERE id = ?")
            .bind(enabled)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Lightweight lookup used by ingest to decide whether this user's newly
    /// ingested articles skip the draft queue. A missing user (deleted mid-run)
    /// defaults to `false` so we never publish without an explicit opt-in.
    pub async fn auto_publish_of(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
        let row: Option<(bool,)> =
            sqlx::query_as("SELECT auto_publish FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        Ok(row.map(|r| r.0).unwrap_or(false))
    }
}

fn normalize_username(input: &str) -> Result<String, UserError> {
    let trimmed = input.trim().to_lowercase();
    if !is_valid_username(&trimmed) {
        return Err(UserError::InvalidUsername);
    }
    if RESERVED_USERNAMES.contains(&trimmed.as_str()) {
        return Err(UserError::ReservedUsername(trimmed));
    }
    Ok(trimmed)
}

pub fn is_valid_username(s: &str) -> bool {
    let len = s.len();
    if !(3..=32).contains(&len) {
        return false;
    }
    s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn is_reserved_username(s: &str) -> bool {
    RESERVED_USERNAMES.contains(&s.to_lowercase().as_str())
}

/// Stored format: `sha256$<32-hex-salt>$<64-hex-hash>`. The algo prefix lets us
/// migrate to argon2/bcrypt later without a backfill — `verify_password` just
/// dispatches on it.
pub fn hash_password(plain: &str) -> String {
    let mut salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut salt);
    let salt_hex = hex::encode(salt);
    let hash = sha256_hex(plain, &salt_hex);
    format!("sha256${salt_hex}${hash}")
}

pub fn verify_password(plain: &str, stored: &str) -> bool {
    let mut parts = stored.splitn(3, '$');
    let algo = parts.next().unwrap_or("");
    let salt = parts.next().unwrap_or("");
    let hash = parts.next().unwrap_or("");
    if algo != "sha256" || salt.is_empty() || hash.is_empty() {
        return false;
    }
    let computed = sha256_hex(plain, salt);
    constant_time_eq(computed.as_bytes(), hash.as_bytes())
}

fn sha256_hex(plain: &str, salt_hex: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt_hex.as_bytes());
    hasher.update(b"$");
    hasher.update(plain.as_bytes());
    hex::encode(hasher.finalize())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let h = hash_password("hunter2");
        assert!(verify_password("hunter2", &h));
        assert!(!verify_password("hunter3", &h));
    }

    #[test]
    fn username_validation() {
        assert!(is_valid_username("alice"));
        assert!(is_valid_username("a-b_c"));
        assert!(!is_valid_username("ab"));
        assert!(!is_valid_username("Alice"));
        assert!(!is_valid_username("alice@"));
        assert!(is_reserved_username("admin"));
    }
}
