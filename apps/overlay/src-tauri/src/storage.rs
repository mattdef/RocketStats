use crate::domain::PlayerCard;
use crate::error::Result;

pub struct Storage {
    pool: sqlx::SqlitePool,
}

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = sqlx::SqlitePool::connect(database_url).await?;
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

#[cfg(test)]
mod tests {
    use super::Storage;
    use crate::domain::PlayerCard;

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
}
