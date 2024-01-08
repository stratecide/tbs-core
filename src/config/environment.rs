use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Display;
use std::sync::Arc;
use std::error::Error;

use interfaces::ConfigInterface;
use interfaces::game_interface::ClientPerspective;
use num_rational::Rational32;

use crate::commander::Commander;
use crate::game::fog::VisionMode;
use crate::commander::commander_type::CommanderType;
use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::point_map::MapSize;
use crate::script::attack::AttackScript;
use crate::script::kill::KillScript;
use crate::script::unit::UnitScript;
use crate::terrain::AmphibiousTyping;
use crate::terrain::ExtraMovementOptions;
use crate::terrain::TerrainType;
use crate::terrain::attributes::TerrainAttributeKey;
use crate::terrain::terrain::*;
use crate::units::combat::*;
use crate::units::movement::MovementType;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::units::attributes::*;
use crate::units::hero::*;
use crate::game::settings::GameSettings;

use super::config::Config;
use super::hero_type_config::HeroTypeConfig;
use super::commander_power_config::CommanderPowerConfig;
use super::commander_type_config::CommanderTypeConfig;
use super::commander_unit_config::CommanderPowerUnitConfig;
use super::movement_type_config::MovementPattern;
use super::terrain_type_config::TerrainTypeConfig;
use super::unit_filter::*;
use super::unit_type_config::UnitTypeConfig;

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

    pub(crate) fn unit_visibility(&self, typ: UnitType, hero: &Hero, owner: i8) -> UnitVisibility {
        self.config.unit_config(typ).visibility
    }

    pub fn unit_valid_action_status(&self, _typ: UnitType, _owner: i8) -> &[ActionStatus] {
        // TODO
        &[ActionStatus::Ready, ActionStatus::Exhausted, ActionStatus::Repairing, ActionStatus::Capturing]
    }

    pub fn unit_transport_capacity(&self, typ: UnitType, owner: i8, hero: HeroType) -> usize {
        // TODO
        self.config.unit_config(typ).transport_capacity
        + hero.transport_capacity(self)
        //+ self.settings.and_then(|s| s.get_player(owner)).and_then(|p| p.commander.transport_capacity()).unwrap_or(0)
    }

    pub fn unit_heal_transported(&self, typ: UnitType, owner: i8, hero: HeroType) -> i8 {
        // TODO
        self.config.unit_config(typ).heal_transported
        //+ hero.heal_transported(self)
        //+ self.settings.and_then(|s| s.get_player(owner)).and_then(|p| p.commander.heal_transported()).unwrap_or(0)
    }

    pub fn unit_cost(&self, typ: UnitType, owner_id: i8) -> i32 {
        self.config.base_cost(typ)
    }

    // terrain

    pub fn default_terrain(&self) -> Terrain {
        TerrainBuilder::new(self, self.config.terrain_types()[0])
        .build()
        // TODO: when validating the config, make sure this unwrap doesn't panic
        .unwrap()
    }
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.config, &other.config)
        && match (&self.settings, &other.settings) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false
        }
    }
}
impl Eq for Environment {}
impl Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(settings) = &self.settings {
            write!(f, "Game '{}' with", settings.name)?;
        }
        write!(f, "Ruleset: '{}'", self.config.name())
    }
}
