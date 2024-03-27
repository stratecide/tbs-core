use std::fmt::Debug;
use std::sync::Arc;
use interfaces::game_interface::ClientPerspective;

use crate::commander::commander_type::CommanderType;
use crate::map::point_map::MapSize;
use crate::terrain::terrain::*;
use crate::units::unit_types::UnitType;
use crate::units::attributes::*;
use crate::units::hero::*;
use crate::game::settings::GameSettings;

use super::config::Config;

#[derive(Clone)]
pub struct Environment {
    pub map_size: MapSize,
    pub config: Arc<Config>,
    pub settings: Option<Arc<GameSettings>>,
}

impl Environment {
    pub fn start_game(&mut self, settings: &Arc<GameSettings>) {
        if self.settings.is_some() {
            panic!("Attempted to start an already started game!")
        }
        self.settings = Some(settings.clone());
    }

    pub fn built_this_turn_cost_factor(&self) -> i32 {
        // TODO
        200
    }

    pub fn get_team(&self, owner_id: i8) -> ClientPerspective {
        if let Some(settings) = &self.settings {
            for player in &settings.players {
                if player.get_owner_id() == owner_id {
                    return ClientPerspective::Team(player.get_team())
                }
            }
        }
        ClientPerspective::Neutral
    }

    pub fn get_income(&self, owner_id: i8) -> i32 {
        if let Some(settings) = &self.settings {
            for player in &settings.players {
                if player.get_owner_id() == owner_id {
                    return player.get_income()
                }
            }
        }
        0
    }

    pub fn get_commander(&self, owner_id: i8) -> CommanderType {
        if let Some(settings) = &self.settings {
            for player in &settings.players {
                if player.get_owner_id() == owner_id {
                    return player.get_commander();
                }
            }
        }
        CommanderType::None
    }

    pub(crate) fn unit_attributes(&self, typ: UnitType, owner: i8) -> std::iter::Chain<std::slice::Iter<'_, AttributeKey>, std::slice::Iter<'_, AttributeKey>> {
        self.config.unit_specific_attributes(typ).iter()
        .chain(self.config.commander_attributes(self.get_commander(owner), typ).iter())
    }

    pub(crate) fn unit_attributes_hidden_by_fog(&self, typ: UnitType, hero: &Hero, owner: i8) -> Vec<AttributeKey> {
        self.config.unit_specific_hidden_attributes(typ).iter()
        .chain(self.config.commander_attributes_hidden_by_fog(self.get_commander(owner), typ).iter())
        .cloned()
        .collect()
    }

    pub fn unit_valid_action_status(&self, typ: UnitType, _owner: i8) -> &[ActionStatus] {
        self.config.unit_specific_statuses(typ)
    }

    pub fn unit_transport_capacity(&self, typ: UnitType, owner: i8, hero: HeroType) -> usize {
        self.config.unit_config(typ).transport_capacity
        + self.config.commander_config(self.get_commander(owner)).transport_capacity as usize
        + hero.transport_capacity(self)
    }

    // terrain

    pub fn default_terrain(&self) -> Terrain {
        TerrainBuilder::new(self, crate::terrain::TerrainType::Grass)
        .build()
        // TODO: when validating the config, make sure this unwrap won't panic
        .unwrap()
    }
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.config, &other.config)
        && match (&self.settings, &other.settings) {
            (Some(a), Some(b)) => **a == **b,
            (None, None) => true,
            _ => false
        }
    }
}
impl Eq for Environment {}
impl Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(settings) = &self.settings {
            write!(f, "Game '{:?}' with ", **settings)?;
        }
        write!(f, "Ruleset: '{}'", self.config.name())
    }
}
