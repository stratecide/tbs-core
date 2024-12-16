use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use interfaces::ClientPerspective;
use num_rational::Rational32;
use zipper::*;

use crate::commander::commander_type::CommanderType;
use crate::commander::Commander;
use crate::config::environment::Environment;
use crate::config::OwnershipPredicate;
use crate::game::fog::{FogIntensity, FogSetting};
use crate::game::game_view::GameView;
use crate::game::settings::GameSettings;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::map::wrapping_map::Distortion;
use crate::player::{Owner, Player};
use crate::tags::*;
use crate::units::hero::HeroInfluence;
use crate::units::movement::MovementType;
use crate::units::UnitVisibility;

use super::{TerrainType, ExtraMovementOptions};

#[derive(Clone, PartialEq, Eq)]
pub struct Terrain<D: Direction> {
    environment: Environment,
    typ: TerrainType,
    owner: Owner,
    tags: TagBag<D>,
}

impl<D: Direction> Debug for Terrain<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name())?;
        write!(f, "Owner: {}, ", self.owner.0)?;
        self.tags.debug(f, &self.environment)?;
        write!(f, ")")
    }
}

impl<D: Direction> Terrain<D> {
    pub(super) fn new(environment: Environment, typ: TerrainType) -> Self {
        let owner = match environment.config.terrain_ownership(typ) {
            OwnershipPredicate::Always => environment.config.max_player_count() - 1,
            _ => -1
        };
        Self {
            environment,
            typ,
            owner: Owner(owner),
            tags: TagBag::new()
        }
    }

    pub(crate) fn start_game(&mut self, settings: &Arc<GameSettings>) {
        self.environment.start_game(settings);
    }

    // getters that aren't influenced by attributes
    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn typ(&self) -> TerrainType {
        self.typ
    }

    pub fn name(&self) -> &str {
        self.environment.config.terrain_name(self.typ)
    }

    pub fn is_chess(&self) -> bool {
        self.environment.config.terrain_chess(self.typ)
    }

    pub fn income_factor(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> Rational32 {
        self.environment.config.terrain_income_factor(game, pos, self, heroes)
    }

    pub fn vision_range(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
    ) -> Option<usize> {
        let mut range = self.environment.config.terrain_vision_range(game, pos, self, heroes)?;
        // TODO: add config column for whether fog_setting should increase vision range instead of this check
        if range == 0 {
            return Some(range);
        }
        match game.get_fog_setting() {
            FogSetting::None => (),
            FogSetting::Light(bonus) |
            FogSetting::Sharp(bonus) |
            FogSetting::Fade1(bonus) |
            FogSetting::Fade2(bonus) |
            FogSetting::ExtraDark(bonus) => range += bonus as usize,
        }
        Some(range)
    }

    pub fn extra_step_options(&self) -> ExtraMovementOptions {
        self.environment.config.terrain_path_extra(self.typ)
    }

    pub fn movement_cost(&self, movement_type: MovementType) -> Option<Rational32> {
        self.environment.config.terrain_movement_cost(self.typ, movement_type)
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner.0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        match self.environment.config.terrain_ownership(self.typ) {
            OwnershipPredicate::Always if id < 0 => (),
            OwnershipPredicate::Never if id >= 0 => (),
            _ => {
                self.owner.0 = id;
            }
        }
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.environment.get_team(self.get_owner_id())
    }

    pub fn get_player(&self, game: &impl GameView<D>) -> Option<Player> {
        game.get_owning_player(self.get_owner_id())
    }

    pub fn get_commander(&self, game: &impl GameView<D>) -> Commander {
        self.get_player(game)
        .and_then(|player| Some(player.commander.clone()))
        .unwrap_or(Commander::new(&self.environment, CommanderType(0)))
    }

    pub(super) fn copy_from(&mut self, other: &Terrain<D>) {
        if self.environment != other.environment {
            panic!("Can't copy from terrain from different environment");
        }
        for key in other.tags.flags() {
            self.set_flag(*key);
        }
        for (key, value) in other.tags.tags() {
            self.set_tag(*key, value.clone());
        }
    }
    pub fn get_tag_bag(&self) -> &TagBag<D> {
        &self.tags
    }

    pub fn has_flag(&self, key: usize) -> bool {
        self.tags.has_flag(key)
    }
    pub fn set_flag(&mut self, key: usize) {
        self.tags.set_flag(&self.environment, key);
    }
    pub fn remove_flag(&mut self, key: usize) {
        self.tags.remove_flag(key);
    }
    pub fn flip_flag(&mut self, key: usize) {
        if self.has_flag(key) {
            self.remove_flag(key);
        } else {
            self.set_flag(key);
        }
    }

    pub fn get_tag(&self, key: usize) -> Option<TagValue<D>> {
        self.tags.get_tag(key)
    }
    pub fn set_tag(&mut self, key: usize, value: TagValue<D>) {
        self.tags.set_tag(&self.environment, key, value);
    }
    pub fn remove_tag(&mut self, key: usize) {
        self.tags.remove_tag(key);
    }

    // methods that go beyond getter / setter functionality

    pub fn get_vision(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        // the heroes affecting this terrain. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
        team: ClientPerspective
    ) -> HashMap<Point, FogIntensity> {
        let allow_vision = if self.environment.config.terrain_ownership(self.typ) == OwnershipPredicate::Never {
            // terrain can never be owned, so its vision is provided to all
            true
        } else {
            // terrain can be owned. it's vision is only provided to the team that owns it
            self.get_team() == team && team != ClientPerspective::Neutral
        };
        if !allow_vision {
            return HashMap::new();
        }
        let Some(vision_range) = self.vision_range(game, pos, heroes) else {
            return HashMap::new();
        };
        let mut result = HashMap::new();
        result.insert(pos, FogIntensity::TrueSight);
        let normal_range = match game.get_fog_setting() {
            FogSetting::ExtraDark(_) => 0,
            FogSetting::Fade1(_) => 1.max(vision_range) - 1,
            FogSetting::Fade2(_) => 2.max(vision_range) - 2,
            _ => vision_range
        };
        let layers = game.range_in_layers(pos, vision_range);
        for (i, layer) in layers.into_iter().enumerate() {
            for p in layer {
                let vision = if i < normal_range {
                    FogIntensity::NormalVision
                } else {
                    FogIntensity::Light
                };
                result.insert(p, vision.min(result.get(&p).cloned().unwrap_or(FogIntensity::Dark)));
            }
        }
        result
    }

    pub fn fog_replacement(&self, intensity: FogIntensity) -> Self {
        if intensity == FogIntensity::TrueSight {
            return self.clone();
        }
        let visibility = match intensity {
            FogIntensity::TrueSight => return self.clone(),
            FogIntensity::NormalVision => UnitVisibility::Normal,
            FogIntensity::Light => UnitVisibility::Normal,
            FogIntensity::Dark => UnitVisibility::AlwaysVisible,
        };
        let mut builder = self.typ.instance(&self.environment)
            .set_tag_bag(self.tags.fog_replacement(&self.environment, visibility));
        if self.environment.config.terrain_owner_visibility(self.typ) >= visibility {
            builder = builder.set_owner_id(self.owner.0);
        }
        builder.build_with_defaults()
    }

    pub fn distort(&mut self, distortion: Distortion<D>) {
        self.tags.distort(distortion);
    }
}

impl<D: Direction> SupportedZippable<&Environment> for Terrain<D> {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.typ.export(zipper, support);
        if support.config.terrain_ownership(self.typ) != OwnershipPredicate::Never {
            self.owner.export(zipper, &*self.environment.config);
        }
        self.tags.export(zipper, &self.environment);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let typ = TerrainType::import(unzipper, support)?;
        let owner = if support.config.terrain_ownership(typ) != OwnershipPredicate::Never {
            Owner::import(unzipper, &*support.config)?
        } else {
            Owner(-1)
        };
        let tags = TagBag::import(unzipper, support)?;
        Ok(Self {
            environment: support.clone(),
            typ,
            owner,
            tags,
        })
    }
}

#[derive(Clone)]
pub struct TerrainBuilder<D: Direction> {
    terrain: Terrain<D>,
}

impl<D: Direction> TerrainBuilder<D> {
    pub fn new(environment: &Environment, typ: TerrainType) -> Self {
        let terrain = Terrain::new(environment.clone(), typ);
        Self {
            terrain,
        }
    }

    pub fn copy_from(mut self, other: &Terrain<D>) -> Self {
        self.terrain.copy_from(other);
        self
    }

    pub fn set_tag_bag(mut self, bag: TagBag<D>) -> Self {
        self.terrain.tags = bag;
        self
    }

    pub fn set_flag(mut self, key: usize) -> Self {
        self.terrain.set_flag(key);
        self
    }
    pub fn remove_flag(mut self, key: usize) -> Self {
        self.terrain.remove_flag(key);
        self
    }

    pub fn set_tag(mut self, key: usize, value: TagValue<D>) -> Self {
        self.terrain.set_tag(key, value);
        self
    }
    pub fn remove_tag(mut self, key: usize) -> Self {
        self.terrain.remove_tag(key);
        self
    }

    pub fn set_owner_id(mut self, id: i8) -> Self {
        self.terrain.set_owner_id(id);
        self
    }

    pub fn build(&self) -> Terrain<D> {
        self.terrain.clone()
    }

    /**
     * TODO: call rhai script to get default flags/tag values?
     */
    pub fn build_with_defaults(&self) -> Terrain<D> {
        self.terrain.clone()
    }
}
