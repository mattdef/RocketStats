use crate::domain::{
    LocalPlayerSummary, RANKED_DOUBLES_PLAYLIST_ID, RANKED_SOLO_PLAYLIST_ID,
    RANKED_STANDARD_PLAYLIST_ID,
};
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

        let ranked_1v1 = skills
            .skills
            .iter()
            .find(|skill| skill.playlist == RANKED_SOLO_PLAYLIST_ID);
        let ranked_2v2 = skills
            .skills
            .iter()
            .find(|skill| skill.playlist == RANKED_DOUBLES_PLAYLIST_ID);
        let ranked_3v3 = skills
            .skills
            .iter()
            .find(|skill| skill.playlist == RANKED_STANDARD_PLAYLIST_ID);

        let ranked_1v1_mmr = ranked_1v1.map(|skill| visible_skill_rating(skill.mmr) as f64);
        let ranked_2v2_mmr = ranked_2v2.map(|skill| visible_skill_rating(skill.mmr) as f64);
        let ranked_3v3_mmr = ranked_3v3.map(|skill| visible_skill_rating(skill.mmr) as f64);

        tracing::info!(
            ranked_1v1_playlist = RANKED_SOLO_PLAYLIST_ID,
            ranked_1v1_raw_skill_mmr = ?ranked_1v1.map(|skill| skill.mmr),
            ranked_1v1_displayed_mmr = ?ranked_1v1_mmr,
            ranked_1v1_tier = ?ranked_1v1.map(|skill| skill.tier),
            ranked_1v1_division = ?ranked_1v1.map(|skill| skill.division),
            ranked_2v2_playlist = RANKED_DOUBLES_PLAYLIST_ID,
            ranked_2v2_raw_skill_mmr = ?ranked_2v2.map(|skill| skill.mmr),
            ranked_2v2_displayed_mmr = ?ranked_2v2_mmr,
            ranked_2v2_tier = ?ranked_2v2.map(|skill| skill.tier),
            ranked_2v2_division = ?ranked_2v2.map(|skill| skill.division),
            ranked_3v3_playlist = RANKED_STANDARD_PLAYLIST_ID,
            ranked_3v3_raw_skill_mmr = ?ranked_3v3.map(|skill| skill.mmr),
            ranked_3v3_displayed_mmr = ?ranked_3v3_mmr,
            ranked_3v3_tier = ?ranked_3v3.map(|skill| skill.tier),
            ranked_3v3_division = ?ranked_3v3.map(|skill| skill.division),
            "loaded local ranked playlist summary"
        );

        Ok(LocalPlayerSummary {
            display_name,
            ranked_1v1_mmr,
            ranked_1v1_tier: ranked_1v1.map(|skill| skill.tier),
            ranked_1v1_division: ranked_1v1.map(|skill| skill.division),
            ranked_2v2_mmr,
            ranked_2v2_tier: ranked_2v2.map(|skill| skill.tier),
            ranked_2v2_division: ranked_2v2.map(|skill| skill.division),
            ranked_3v3_mmr,
            ranked_3v3_tier: ranked_3v3.map(|skill| skill.tier),
            ranked_3v3_division: ranked_3v3.map(|skill| skill.division),
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
    use super::{LocalPlayerClient, LocalPlayerSummaryLoader};
    use crate::domain::{
        LocalPlayerSummary, RANKED_DOUBLES_PLAYLIST_ID, RANKED_SOLO_PLAYLIST_ID,
        RANKED_STANDARD_PLAYLIST_ID,
    };
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
    async fn loads_ranked_summaries_for_all_competitive_playlists() {
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
                        playlist: RANKED_SOLO_PLAYLIST_ID,
                        mu: 0.0,
                        sigma: 0.0,
                        tier: 7,
                        division: 2,
                        mmr: 20.0,
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
                    Skill {
                        playlist: RANKED_STANDARD_PLAYLIST_ID,
                        mu: 0.0,
                        sigma: 0.0,
                        tier: 10,
                        division: 1,
                        mmr: 25.0,
                        win_streak: 1,
                        matches_played: 40,
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
                ranked_1v1_mmr: Some(500.0),
                ranked_1v1_tier: Some(7),
                ranked_1v1_division: Some(2),
                ranked_2v2_mmr: Some(954.0),
                ranked_2v2_tier: Some(18),
                ranked_2v2_division: Some(3),
                ranked_3v3_mmr: Some(600.0),
                ranked_3v3_tier: Some(10),
                ranked_3v3_division: Some(1),
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

        assert_eq!(
            summary,
            LocalPlayerSummary {
                display_name: "FallbackName".to_owned(),
                ranked_1v1_mmr: None,
                ranked_1v1_tier: None,
                ranked_1v1_division: None,
                ranked_2v2_mmr: None,
                ranked_2v2_tier: None,
                ranked_2v2_division: None,
                ranked_3v3_mmr: None,
                ranked_3v3_tier: None,
                ranked_3v3_division: None,
            }
        );
    }

    #[tokio::test]
    async fn leaves_missing_ranked_playlists_empty() {
        let client = MockLocalPlayerClient {
            profiles: vec![PlayerData {
                player_id: "Epic|7efc351e447043c4be4447da51b790e4|0".to_owned(),
                player_name: "LeSingeDePaille".to_owned(),
                presence_state: "Online".to_owned(),
                presence_info: String::new(),
            }],
            skills: GetPlayerSkillResponse {
                skills: vec![
                    Skill {
                        playlist: RANKED_SOLO_PLAYLIST_ID,
                        mu: 0.0,
                        sigma: 0.0,
                        tier: 9,
                        division: 4,
                        mmr: 30.0,
                        win_streak: 2,
                        matches_played: 10,
                        placement_matches_played: 0,
                    },
                    Skill {
                        playlist: RANKED_STANDARD_PLAYLIST_ID,
                        mu: 0.0,
                        sigma: 0.0,
                        tier: 12,
                        division: 2,
                        mmr: 35.0,
                        win_streak: 1,
                        matches_played: 20,
                        placement_matches_played: 0,
                    },
                ],
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

        assert_eq!(
            summary,
            LocalPlayerSummary {
                display_name: "LeSingeDePaille".to_owned(),
                ranked_1v1_mmr: Some(700.0),
                ranked_1v1_tier: Some(9),
                ranked_1v1_division: Some(4),
                ranked_2v2_mmr: None,
                ranked_2v2_tier: None,
                ranked_2v2_division: None,
                ranked_3v3_mmr: Some(800.0),
                ranked_3v3_tier: Some(12),
                ranked_3v3_division: Some(2),
            }
        );
    }

    #[test]
    fn visible_skill_rating_matches_tracker_scale() {
        assert_eq!(super::visible_skill_rating(25.0), 600);
        assert_eq!(super::visible_skill_rating(43.0), 960);
    }
}
