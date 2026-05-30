use crate::domain::PlayerCard;
use crate::error::Result;
use rocketstats_rlapi::PlayerId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

pub trait SkillClient {
    fn enrich_players<'a>(
        &'a self,
        player_ids: Vec<PlayerId>,
        playlist: i32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerCard>>> + Send + 'a>>;
}

pub struct PlayerEnrichment<C> {
    client: C,
    local_player_id: Option<String>,
    cache: HashMap<String, PlayerCard>,
}

impl<C> PlayerEnrichment<C>
where
    C: SkillClient,
{
    pub fn new(client: C, local_player_id: Option<String>) -> Self {
        Self {
            client,
            local_player_id,
            cache: HashMap::new(),
        }
    }

    pub async fn enrich_detected(
        &mut self,
        detected: Vec<String>,
        playlist: i32,
    ) -> Result<Vec<PlayerCard>> {
        let mut ordered = Vec::new();
        for player_id in detected {
            if self.local_player_id.as_deref() == Some(player_id.as_str()) {
                continue;
            }
            if !ordered.contains(&player_id) {
                ordered.push(player_id);
            }
        }

        let mut missing = Vec::new();
        for player_id in &ordered {
            if !self.cache.contains_key(player_id)
                && let Ok(parsed) = PlayerId::from_str(player_id)
            {
                missing.push(parsed);
            }
        }

        if !missing.is_empty() {
            for card in self.client.enrich_players(missing, playlist).await? {
                self.cache.insert(card.player_id.clone(), card);
            }
        }

        Ok(ordered
            .into_iter()
            .filter_map(|player_id| self.cache.get(&player_id).cloned())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerEnrichment, SkillClient};
    use crate::domain::PlayerCard;
    use crate::error::Result;
    use rocketstats_rlapi::PlayerId;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockSkillClient {
        calls: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl SkillClient for MockSkillClient {
        fn enrich_players<'a>(
            &'a self,
            player_ids: Vec<PlayerId>,
            playlist: i32,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerCard>>> + Send + 'a>> {
            Box::pin(async move {
                let ids = player_ids
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>();
                self.calls.lock().unwrap().push(ids.clone());
                Ok(ids
                    .into_iter()
                    .map(|player_id| PlayerCard {
                        player_id,
                        name: Some("Detected".to_owned()),
                        playlist: Some(playlist),
                        mmr: Some(900.0),
                        tier: Some(14),
                        division: Some(1),
                        data_age_seconds: 0,
                    })
                    .collect())
            })
        }
    }

    #[tokio::test]
    async fn filters_local_player_deduplicates_and_caches() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let client = MockSkillClient {
            calls: Arc::clone(&calls),
        };
        let mut enrichment = PlayerEnrichment::new(
            client,
            Some("Epic|local000000000000000000000000000|0".to_owned()),
        );

        let first = enrichment
            .enrich_detected(
                vec![
                    "Epic|local000000000000000000000000000|0".to_owned(),
                    "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
                    "Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned(),
                ],
                11,
            )
            .await
            .unwrap();
        let second = enrichment
            .enrich_detected(
                vec!["Epic|aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|0".to_owned()],
                11,
            )
            .await
            .unwrap();

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
}

#[derive(Clone)]
pub struct PsyNetSkillClient {
    rpc: rocketstats_rlapi::PsyNetRpc,
}

impl PsyNetSkillClient {
    pub fn new(rpc: rocketstats_rlapi::PsyNetRpc) -> Self {
        Self { rpc }
    }
}

impl SkillClient for PsyNetSkillClient {
    fn enrich_players<'a>(
        &'a self,
        player_ids: Vec<PlayerId>,
        playlist: i32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerCard>>> + Send + 'a>> {
        Box::pin(async move {
            let profiles = self.rpc.get_profiles(player_ids.clone()).await?;
            let skills = self.rpc.get_players_skills(player_ids).await?;

            let mut names = profiles
                .into_iter()
                .map(|profile| (profile.player_id, profile.player_name))
                .collect::<HashMap<_, _>>();

            let cards = skills
                .into_iter()
                .map(|player| {
                    let skill = player
                        .skills
                        .iter()
                        .find(|skill| skill.playlist == playlist);
                    PlayerCard {
                        player_id: player.player_id.to_string(),
                        name: names.remove(player.player_id.as_str()),
                        playlist: skill.map(|skill| skill.playlist),
                        mmr: skill.map(|skill| skill.mmr),
                        tier: skill.map(|skill| skill.tier),
                        division: skill.map(|skill| skill.division),
                        data_age_seconds: 0,
                    }
                })
                .collect();

            Ok(cards)
        })
    }
}
