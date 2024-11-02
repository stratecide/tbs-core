use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use interfaces::ClientPerspective;
use rustc_hash::FxHashSet;
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
use crate::units::UnitVisibility;

use super::token_types::TokenType;
use super::MAX_STACK_SIZE;

#[derive(Clone, PartialEq, Eq)]
pub struct Token<D: Direction> {
    environment: Environment,
    typ: TokenType,
    owner: Owner,
    tags: TagBag<D>,
}

impl<D: Direction> Debug for Token<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name())?;
        write!(f, "Owner: {}, ", self.owner.0)?;
        self.tags.debug(f, &self.environment)?;
        write!(f, ")")
    }
}

impl<D: Direction> Token<D> {
    pub fn new(environment: Environment, typ: TokenType) -> Self {
        let owner = match environment.config.token_ownership(typ) {
            OwnershipPredicate::Always => environment.config.max_player_count() - 1,
            _ => -1
        };
        Self {
            environment,
            typ,
            owner: Owner(owner),
            tags: TagBag::new()
            //attributes: FxHashMap::default(),
        }
    }

    // remove Token from value that conflict with other Token
    // starting from the back, so add_token can be used by the editor to overwrite previous data
    pub fn correct_stack(stack: Vec<Self>) -> Vec<Self> {
        let mut existing = FxHashSet::default();
        let stack: Vec<Self> = stack.into_iter()
        .rev()
        .filter(|token| {
            existing.insert((token.typ, token.owner.0))
        }).take(MAX_STACK_SIZE as usize)
        .collect();
        stack.into_iter().rev().collect()
    }

    pub(crate) fn start_game(&mut self, settings: &Arc<GameSettings>) {
        self.environment.start_game(settings);
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn typ(&self) -> TokenType {
        self.typ
    }

    pub fn name(&self) -> &str {
        self.environment.config.token_name(self.typ)
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner.0
    }
    pub fn set_owner_id(&mut self, id: i8) {
        match self.environment.config.token_ownership(self.typ) {
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
        .unwrap_or(Commander::new(&self.environment, CommanderType::None))
    }

    pub(super) fn copy_from(&mut self, other: &TagBag<D>) {
        for key in other.flags() {
            self.set_flag(*key);
        }
        for (key, value) in other.tags() {
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

    pub fn distort(&mut self, distortion: Distortion<D>) {
        self.tags.distort(distortion);
    }
    pub fn translate(&mut self, translations: [D::T; 2], odd_if_hex: bool) {
        self.tags.translate(translations, odd_if_hex);
    }

    pub fn vision_range(&self, game: &impl GameView<D>) -> Option<usize> {
        let mut range = self.environment.config.token_vision_range(self.typ)?;
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

    pub fn get_vision(
        &self,
        game: &impl GameView<D>,
        pos: Point,
        team: ClientPerspective
    ) -> HashMap<Point, FogIntensity> {
        if self.get_team() != team && self.get_team() != ClientPerspective::Neutral {
            return HashMap::new();
        }
        let vision_range = if let Some(v) = self.vision_range(game) {
            v
        } else {
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

    pub fn fog_replacement(&self, intensity: FogIntensity) -> Option<Self> {
        // for now, heroes don't affect token visibility.
        // when they do in the future, the heroes should be given to this method instead of calculating here
        let visibility = self.environment.config.token_visibility(self.typ);
        let minimum_visibility = match intensity {
            FogIntensity::TrueSight => return Some(self.clone()),
            FogIntensity::NormalVision => {
                if visibility == UnitVisibility::Stealth {
                    return None
                } else {
                    return Some(self.clone());
                }
            }
            FogIntensity::Light => {
                match visibility {
                    UnitVisibility::Stealth => return None,
                    UnitVisibility::Normal => UnitVisibility::AlwaysVisible,
                    UnitVisibility::AlwaysVisible => UnitVisibility::Normal,
                }
            }
            FogIntensity::Dark => {
                // normal units don't have AlwaysVisible so far, but doesn't hurt
                if visibility != UnitVisibility::AlwaysVisible {
                    return None
                } else {
                    UnitVisibility::Normal
                }
            }
        };
        // unit is visible, hide some attributes maybe
        let mut result = self.typ.instance(&self.environment);
        result.tags = self.tags.fog_replacement(&self.environment, minimum_visibility);
        if self.environment.config.token_owner_visibility(self.typ) >= minimum_visibility {
            result.owner = self.owner;
        }
        Some(result)
    }

}

impl<D: Direction> SupportedZippable<&Environment> for Token<D> {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.typ.export(zipper, support);
        if support.config.token_ownership(self.typ) != OwnershipPredicate::Never {
            self.owner.export(zipper, &*self.environment.config);
        }
        self.tags.export(zipper, &self.environment);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let typ = TokenType::import(unzipper, support)?;
        let owner = if support.config.token_ownership(typ) != OwnershipPredicate::Never {
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
