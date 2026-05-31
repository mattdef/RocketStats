use crate::domain::{PlayerCard, StoredTokens};
use crate::error::Result;
use sqlx::sqlite::SqliteConnectOptions;
use std::path::Path;

pub struct Storage {
    pool: sqlx::SqlitePool,
}

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = sqlx::SqlitePool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn connect_file(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(options).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS player_cards (
                player_id TEXT PRIMARY KEY NOT NULL,
                name TEXT,
                playlist INTEGER,
                mmr REAL,
                tier INTEGER,
                division INTEGER,
                data_age_seconds INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS auth_tokens (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                refresh_token TEXT NOT NULL,
                account_id TEXT NOT NULL,
                player_name TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_player_card(&self, card: &PlayerCard) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO player_cards (
                player_id,
                name,
                playlist,
                mmr,
                tier,
                division,
                data_age_seconds
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(player_id) DO UPDATE SET
                name = excluded.name,
                playlist = excluded.playlist,
                mmr = excluded.mmr,
                tier = excluded.tier,
                division = excluded.division,
                data_age_seconds = excluded.data_age_seconds
            "#,
        )
        .bind(&card.player_id)
        .bind(&card.name)
        .bind(card.playlist)
        .bind(card.mmr)
        .bind(card.tier)
        .bind(card.division)
        .bind(card.data_age_seconds as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_player_card(&self, player_id: &str) -> Result<Option<PlayerCard>> {
        let row = sqlx::query_as::<_, PlayerCardRow>(
            r#"
            SELECT player_id, name, playlist, mmr, tier, division, data_age_seconds
            FROM player_cards
            WHERE player_id = ?1
            "#,
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(PlayerCardRow::into_card))
    }

    pub async fn store_auth_tokens(&self, tokens: &StoredTokens) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO auth_tokens (id, refresh_token, account_id, player_name)
            VALUES (1, ?1, ?2, ?3)
            ON CONFLICT(id) DO UPDATE SET
                refresh_token = excluded.refresh_token,
                account_id = excluded.account_id,
                player_name = excluded.player_name
            "#,
        )
        .bind(&tokens.refresh_token)
        .bind(&tokens.account_id)
        .bind(&tokens.player_name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_auth_tokens(&self) -> Result<Option<StoredTokens>> {
        let row = sqlx::query_as::<_, AuthTokensRow>(
            r#"
            SELECT refresh_token, account_id, player_name
            FROM auth_tokens
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(AuthTokensRow::into_stored))
    }

    pub async fn clear_auth_tokens(&self) -> Result<()> {
        sqlx::query("DELETE FROM auth_tokens WHERE id = 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct PlayerCardRow {
    player_id: String,
    name: Option<String>,
    playlist: Option<i32>,
    mmr: Option<f64>,
    tier: Option<i32>,
    division: Option<i32>,
    data_age_seconds: i64,
}

impl PlayerCardRow {
    fn into_card(self) -> PlayerCard {
        PlayerCard {
            player_id: self.player_id,
            name: self.name,
            playlist: self.playlist,
            mmr: self.mmr,
            tier: self.tier,
            division: self.division,
            data_age_seconds: self.data_age_seconds.max(0) as u64,
        }
    }
}

#[derive(sqlx::FromRow)]
struct AuthTokensRow {
    refresh_token: String,
    account_id: String,
    player_name: Option<String>,
}

impl AuthTokensRow {
    fn into_stored(self) -> StoredTokens {
        StoredTokens {
            refresh_token: self.refresh_token,
            account_id: self.account_id,
            player_name: self.player_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Storage;
    use crate::domain::{PlayerCard, StoredTokens};
    use tempfile::tempdir;

    #[tokio::test]
    async fn stores_and_reads_player_card() {
        let storage = Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let card = PlayerCard {
            player_id: "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
            name: Some("Opponent".to_owned()),
            playlist: Some(11),
            mmr: Some(912.4),
            tier: Some(15),
            division: Some(2),
            data_age_seconds: 0,
        };

        storage.upsert_player_card(&card).await.unwrap();

        let stored = storage
            .get_player_card("Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0")
            .await
            .unwrap();
        assert_eq!(stored, Some(card));
    }

    #[tokio::test]
    async fn stores_reads_and_clears_auth_tokens() {
        let storage = Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        // No tokens initially
        assert!(storage.load_auth_tokens().await.unwrap().is_none());

        // Store tokens
        let tokens = StoredTokens {
            refresh_token: "test-refresh-token".to_owned(),
            account_id: "7efc351e447043c4be4447da51b790e4".to_owned(),
            player_name: Some("TestPlayer".to_owned()),
        };
        storage.store_auth_tokens(&tokens).await.unwrap();

        // Read tokens
        let loaded = storage.load_auth_tokens().await.unwrap();
        assert_eq!(loaded, Some(tokens.clone()));

        // Upsert updates existing
        let updated = StoredTokens {
            refresh_token: "new-refresh-token".to_owned(),
            account_id: "7efc351e447043c4be4447da51b790e4".to_owned(),
            player_name: None,
        };
        storage.store_auth_tokens(&updated).await.unwrap();
        let loaded = storage.load_auth_tokens().await.unwrap();
        assert_eq!(loaded, Some(updated));

        // Clear tokens
        storage.clear_auth_tokens().await.unwrap();
        assert!(storage.load_auth_tokens().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn persists_auth_tokens_across_file_backed_connections() {
        let tempdir = tempdir().unwrap();
        let db_path = tempdir.path().join("rocketstats-overlay.db");

        let storage = Storage::connect_file(&db_path).await.unwrap();
        storage.migrate().await.unwrap();

        let tokens = StoredTokens {
            refresh_token: "persisted-refresh-token".to_owned(),
            account_id: "7efc351e447043c4be4447da51b790e4".to_owned(),
            player_name: Some("PersistentPlayer".to_owned()),
        };
        storage.store_auth_tokens(&tokens).await.unwrap();

        drop(storage);

        let reopened = Storage::connect_file(&db_path).await.unwrap();
        reopened.migrate().await.unwrap();

        let loaded = reopened.load_auth_tokens().await.unwrap();
        assert_eq!(loaded, Some(tokens));
    }
}
