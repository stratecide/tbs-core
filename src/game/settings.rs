use super::game::FogMode;


#[derive(Debug)]
pub struct GameSettings {
    pub fog_mode: FogMode,
}

#[derive(Debug)]
pub enum NotPlayable {
    TooFewPlayers,
}
