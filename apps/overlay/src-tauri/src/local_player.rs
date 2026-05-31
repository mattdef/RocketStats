use crate::domain::{LocalPlayerSummary, RANKED_DOUBLES_PLAYLIST_ID};
use crate::error::Result;
use rocketstats_rlapi::{GetPlayerSkillResponse, Platform, PlayerData, PlayerId, PsyNetRpc};
use std::future::Future;
use std::pin::Pin;

pub trait LocalPlayerClient {
    fn get_profiles<'a>(
        &'a self,
        player_ids: Vec<PlayerId>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerData>>> + Send + 'a>>;

    fn get_players_skills<'a>(
        &'a self,
        player_id: PlayerId,
    ) -> Pin<Box<dyn Future<Output = Result<GetPlayerSkillResponse>> + Send + 'a>>;
}

pub struct LocalPlayerSummaryLoader<C> {
    client: C,
}

impl<C> LocalPlayerSummaryLoader<C>
where
    C: LocalPlayerClient,
{
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn load(
        &self,
        account_id: &str,
        fallback_name: Option<&str>,
    ) -> Result<LocalPlayerSummary> {
        let player_id = PlayerId::new(Platform::Epic, account_id);
        let profiles = self.client.get_profiles(vec![player_id.clone()]).await?;
        let skills = self.client.get_players_skills(player_id.clone()).await?;

        let display_name = profiles
            .into_iter()
            .find(|profile| profile.player_id == player_id.as_str())
            .map(|profile| profile.player_name)
            .filter(|name| !name.is_empty())
            .or_else(|| fallback_name.map(ToOwned::to_owned))
            .unwrap_or_else(|| account_id.to_owned());

        let ranked_doubles = skills
            .skills
            .into_iter()
            .find(|skill| skill.playlist == RANKED_DOUBLES_PLAYLIST_ID);
        let displayed_mmr = ranked_doubles
            .as_ref()
            .map(|skill| visible_skill_rating(skill.mmr) as f64);

        if let Some(skill) = ranked_doubles.as_ref() {
            tracing::info!(
                playlist = RANKED_DOUBLES_PLAYLIST_ID,
                raw_skill_mmr = skill.mmr,
                displayed_mmr,
                tier = skill.tier,
                division = skill.division,
                "loaded local ranked doubles summary"
            );
        }

        Ok(LocalPlayerSummary {
            display_name,
            ranked_2v2_mmr: displayed_mmr,
            ranked_2v2_tier: ranked_doubles.as_ref().map(|skill| skill.tier),
            ranked_2v2_division: ranked_doubles.map(|skill| skill.division),
        })
    }
}

#[derive(Clone)]
pub struct PsyNetLocalPlayerClient {
    rpc: PsyNetRpc,
}

impl PsyNetLocalPlayerClient {
    pub fn new(rpc: PsyNetRpc) -> Self {
        Self { rpc }
    }
}

impl LocalPlayerClient for PsyNetLocalPlayerClient {
    fn get_profiles<'a>(
        &'a self,
        player_ids: Vec<PlayerId>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerData>>> + Send + 'a>> {
        Box::pin(async move { Ok(self.rpc.get_profiles(player_ids).await?) })
    }

    fn get_players_skills<'a>(
        &'a self,
        player_id: PlayerId,
    ) -> Pin<Box<dyn Future<Output = Result<GetPlayerSkillResponse>> + Send + 'a>> {
        Box::pin(async move { Ok(self.rpc.get_player_skill(player_id).await?) })
    }
}

fn visible_skill_rating(raw_mmr: f64) -> i32 {
    // PsyNet skill endpoints expose the raw skill value; the visible rating
    // used by trackers/UI is derived from it.
    (raw_mmr * 20.0 + 100.0).round() as i32
}

#[cfg(test)]
mod tests {
    use super::{LocalPlayerClient, LocalPlayerSummaryLoader, RANKED_DOUBLES_PLAYLIST_ID};
    use crate::domain::LocalPlayerSummary;
    use crate::error::Result;
    use rocketstats_rlapi::{GetPlayerSkillResponse, PlayerData, PlayerId, RewardLevels, Skill};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MockLocalPlayerClient {
        requested_players: Arc<Mutex<Vec<Vec<String>>>>,
        profiles: Vec<PlayerData>,
        skills: GetPlayerSkillResponse,
    }

    impl Default for MockLocalPlayerClient {
        fn default() -> Self {
            Self {
                requested_players: Arc::new(Mutex::new(Vec::new())),
                profiles: Vec::new(),
                skills: GetPlayerSkillResponse {
                    skills: Vec::new(),
                    reward_levels: RewardLevels {
                        season_level: 0,
                        season_level_wins: 0,
                    },
                },
            }
        }
    }

    impl LocalPlayerClient for MockLocalPlayerClient {
        fn get_profiles<'a>(
            &'a self,
            player_ids: Vec<PlayerId>,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<PlayerData>>> + Send + 'a>> {
            Box::pin(async move {
                self.requested_players.lock().unwrap().push(
                    player_ids
                        .into_iter()
                        .map(|player_id| player_id.to_string())
                        .collect(),
                );
                Ok(self.profiles.clone())
            })
        }

        fn get_players_skills<'a>(
            &'a self,
            _player_id: PlayerId,
        ) -> Pin<Box<dyn Future<Output = Result<GetPlayerSkillResponse>> + Send + 'a>> {
            Box::pin(async move { Ok(self.skills.clone()) })
        }
    }

    #[tokio::test]
    async fn derives_visible_rating_from_raw_skill_when_leaderboard_value_is_rank_code() {
        let requested_players = Arc::new(Mutex::new(Vec::new()));
        let client = MockLocalPlayerClient {
            requested_players: Arc::clone(&requested_players),
            profiles: vec![PlayerData {
                player_id: "Epic|7efc351e447043c4be4447da51b790e4|0".to_owned(),
                player_name: "LeSingeDePaille".to_owned(),
                presence_state: "Online".to_owned(),
                presence_info: String::new(),
            }],
            skills: GetPlayerSkillResponse {
                skills: vec![
                    Skill {
                        playlist: 10,
                        mu: 0.0,
                        sigma: 0.0,
                        tier: 7,
                        division: 2,
                        mmr: 500.0,
                        win_streak: 0,
                        matches_played: 0,
                        placement_matches_played: 0,
                    },
                    Skill {
                        playlist: RANKED_DOUBLES_PLAYLIST_ID,
                        mu: 0.0,
                        sigma: 0.0,
                        tier: 18,
                        division: 3,
                        mmr: 42.6797,
                        win_streak: 4,
                        matches_played: 200,
                        placement_matches_played: 0,
                    },
                ],
                reward_levels: RewardLevels {
                    season_level: 0,
                    season_level_wins: 0,
                },
            },
        };

        let loader = LocalPlayerSummaryLoader::new(client);

        let summary = loader
            .load("7efc351e447043c4be4447da51b790e4", Some("FallbackName"))
            .await
            .unwrap();

        assert_eq!(
            summary,
            LocalPlayerSummary {
                display_name: "LeSingeDePaille".to_owned(),
                ranked_2v2_mmr: Some(954.0),
                ranked_2v2_tier: Some(18),
                ranked_2v2_division: Some(3),
            }
        );
        assert_eq!(
            requested_players.lock().unwrap().as_slice(),
            &[vec!["Epic|7efc351e447043c4be4447da51b790e4|0".to_owned()]]
        );
    }

    #[tokio::test]
    async fn falls_back_to_auth_name_when_profile_lookup_returns_no_name() {
        let client = MockLocalPlayerClient {
            profiles: Vec::new(),
            skills: GetPlayerSkillResponse {
                skills: Vec::new(),
                reward_levels: RewardLevels {
                    season_level: 0,
                    season_level_wins: 0,
                },
            },
            ..Default::default()
        };

        let loader = LocalPlayerSummaryLoader::new(client);

        let summary = loader
            .load("7efc351e447043c4be4447da51b790e4", Some("FallbackName"))
            .await
            .unwrap();

        assert_eq!(summary.display_name, "FallbackName");
        assert_eq!(summary.ranked_2v2_mmr, None);
        assert_eq!(summary.ranked_2v2_tier, None);
        assert_eq!(summary.ranked_2v2_division, None);
    }

    #[test]
    fn visible_skill_rating_matches_tracker_scale() {
        assert_eq!(super::visible_skill_rating(25.0), 600);
        assert_eq!(super::visible_skill_rating(43.0), 960);
    }
}
