use interfaces::{ClientPerspective, GameInterface};
use semver::Version;
use zipper::*;
use zipper::zipper_derive::*;

use crate::commander::commander_type::CommanderChargeChange;
use crate::map::board::{Board, BoardView};
use crate::map::map::FieldData;
use crate::map::point::Point;
use crate::map::point_map;
use crate::tags::*;
use crate::units::commands::UnloadIndex;
use crate::units::hero::{Hero, HeroChargeChange};
use crate::units::unit::Unit;
use crate::units::UnitVisibility;
use crate::{player::*, tokens};
use crate::terrain::terrain::*;
use crate::tokens::token::Token;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::game::fog::*;
use crate::config::environment::Environment;

use super::event_fx::*;

impl SupportedZippable<&Environment> for (Point, FogIntensity, FogIntensity) {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.0.export(zipper, support);
        self.1.zip(zipper);
        self.2.zip(zipper);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok((
            Point::import(unzipper, support)?,
            FogIntensity::unzip(unzipper)?,
            FogIntensity::unzip(unzipper)?,
        ))
    }
}

impl<D: Direction> SupportedZippable<&Environment> for (Point, FogIntensity, FieldData<D>, FogIntensity, FieldData<D>) {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.0.export(zipper, support);
        self.1.zip(zipper);
        self.2.export(zipper, support);
        self.3.zip(zipper);
        self.4.export(zipper, support);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Ok((
            Point::import(unzipper, support)?,
            FogIntensity::unzip(unzipper)?,
            FieldData::import(unzipper, support)?,
            FogIntensity::unzip(unzipper)?,
            FieldData::import(unzipper, support)?,
        ))
    }
}

/**
 * first event has to take up more than 7 bits or it may be read from Unzipper padding by accident
 */
#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 6, support_ref = Environment)]
pub enum Event<D:Direction> {
    // global
    NextTurn,
    GameEnds,
    // fog events
    PureFogChange(Perspective, LVec<(Point, FogIntensity, FogIntensity), {point_map::MAX_AREA}>),
    FogChange(Perspective, LVec<(Point, FogIntensity, FieldData<D>, FogIntensity, FieldData<D>), {point_map::MAX_AREA}>),
    // player events
    PlayerDies(Owner),
    PurePlayerFog,
    PlayerFlag(Owner, FlagKey),
    PlayerSetTag(Owner, TagKeyValues<1, D>),
    PlayerRemoveTag(Owner, TagKeyValues<1, D>),
    PlayerReplaceTag(Owner, TagKeyValues<2, D>),
    // commander events
    CommanderCharge(Owner, CommanderChargeChange),
    CommanderPowerIndex(Owner, U<31>, U<31>),
    // hero events
    HeroSet(Point, Hero),
    HeroCharge(Point, HeroChargeChange),
    HeroChargeTransported(Point, UnloadIndex, HeroChargeChange),
    HeroPower(Point, U<31>, U<31>),
    // unit events
    UnitAdd(Point, Unit<D>),
    UnitRemove(Point, Unit<D>),
    UnitAddBoarded(Point, Unit<D>),
    UnitRemoveBoarded(Point, UnloadIndex, Unit<D>),
    UnitFlag(Point, FlagKey),
    UnitFlagBoarded(Point, UnloadIndex, FlagKey),
    UnitSetTag(Point, TagKeyValues<1, D>),
    UnitSetTagBoarded(Point, UnloadIndex, TagKeyValues<1, D>),
    UnitRemoveTag(Point, TagKeyValues<1, D>),
    UnitRemoveTagBoarded(Point, UnloadIndex, TagKeyValues<1, D>),
    UnitReplaceTag(Point, TagKeyValues<2, D>),
    UnitReplaceTagBoarded(Point, UnloadIndex, TagKeyValues<2, D>),
    // terrain events
    TerrainChange(Point, Terrain<D>, Terrain<D>),
    TerrainFlag(Point, FlagKey),
    TerrainSetTag(Point, TagKeyValues<1, D>),
    TerrainRemoveTag(Point, TagKeyValues<1, D>),
    TerrainReplaceTag(Point, TagKeyValues<2, D>),
    // token events
    RemoveToken(Point, U<{tokens::MAX_STACK_SIZE as i32 - 1}>, Token<D>),
    ReplaceToken(Point, LVec<Token<D>, {tokens::MAX_STACK_SIZE}>, LVec<Token<D>, {tokens::MAX_STACK_SIZE}>),
    // visual
    Effect(Effect<D>),
    Effects(LVec<Effect<D>, {point_map::MAX_AREA}>),
}

impl<D: Direction> Event<D> {
    pub fn export_list(list: &Vec<Self>, environment: &Environment) -> Vec<u8> {
        let mut zipper = Zipper::new();
        for e in list {
            e.export(&mut zipper, environment);
        }
        zipper.finish()
    }
    pub fn import_list(list: Vec<u8>, environment: &Environment, version: Version) -> Result<Vec<Self>, ZipperError> {
        let mut unzipper = Unzipper::new(list, version);
        let mut result = vec![];
        loop {
            match Self::import(&mut unzipper, environment) {
                Ok(e) => result.push(e),
                Err(ZipperError::NotEnoughBits) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(result)
    }

    pub fn apply(&self, game: &mut Game<D>) {
        match self {
            // global
            Self::NextTurn => game.current_turn += 1,
            Self::GameEnds => {
                game.set_ended(true);
            }
            // fog
            Self::PureFogChange(team, vision_changes) => {
                let team = to_client_perspective(&team);
                for (pos, _, intensity) in vision_changes {
                    game.set_fog(team, *pos, *intensity);
                }
            }
            Self::FogChange(team, changes) => {
                let team = to_client_perspective(&team);
                for (pos, _, _, intensity, fd) in changes.iter() {
                    apply_vision_changes(game, team, *pos, *intensity, fd);
                }
            }
            // player
            Self::PlayerDies(owner) => {
                game.get_owning_player_mut(owner.0).unwrap().dead = true;
            }
            Self::PurePlayerFog => (), // server doesn't hide/reveal player flags/tags
            Self::PlayerFlag(owner, flag) => {
                let environment = game.environment().clone();
                game.get_owning_player_mut(owner.0).unwrap().flip_flag(&environment, flag.0);
            }
            Self::PlayerSetTag(owner, TagKeyValues(key, [value])) |
            Self::PlayerReplaceTag(owner, TagKeyValues(key, [_, value])) => {
                let environment = game.environment().clone();
                game.get_owning_player_mut(owner.0).unwrap().set_tag(&environment, key.0, value.clone());
            }
            Self::PlayerRemoveTag(owner, TagKeyValues(key, [_])) => {
                game.get_owning_player_mut(owner.0).unwrap().remove_tag(key.0);
            }
            // commander
            Self::CommanderCharge(owner, delta) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.add_charge(delta.0);
            }
            Self::CommanderPowerIndex(owner, _, index) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.set_active_power(**index as usize);
            }
            // hero
            Self::HeroSet(p, hero) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.set_hero(hero.clone());
                }
            }
            Self::HeroCharge(p, change) => {
                let environment = game.environment().clone();
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_charge(&environment, (hero.get_charge() as i32 + change.0) as u32);
                }
            }
            Self::HeroChargeTransported(p, unload_index, change) => {
                let environment = game.environment().clone();
                if let Some(mut transported) = game.get_map_mut().get_unit_mut(*p)
                .map(|u| u.get_transported_mut()) {
                    if let Some(hero) = transported.get_mut(unload_index.0)
                    .and_then(|u| u.get_hero_mut()) {
                        hero.set_charge(&environment, (hero.get_charge() as i32 + change.0) as u32);
                    }
                }
            }
            Self::HeroPower(p, _, index) => {
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_active_power(**index as usize);
                }
            }
            // unit events
            Self::UnitAdd(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitRemove(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to remove!", pos));
            }
            Self::UnitAddBoarded(pos, unit) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut();
                transported.push(unit.clone());
            }
            Self::UnitRemoveBoarded(pos, index, _) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut();
                transported.remove(index.0);
            }
            Self::UnitFlag(pos, key) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.flip_flag(key.0);
                }
            }
            Self::UnitFlagBoarded(pos, index, key) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to flip flag!", pos));
                if let Some(unit) = transporter.get_transported_mut().get_mut(index.0) {
                    unit.flip_flag(key.0);
                }
            }
            Self::UnitSetTag(pos, TagKeyValues(key, [value])) |
            Self::UnitReplaceTag(pos, TagKeyValues(key, [_, value])) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.set_tag(key.0, value.clone());
                }
            }
            Self::UnitSetTagBoarded(pos, index, TagKeyValues(key, [value])) |
            Self::UnitReplaceTagBoarded(pos, index, TagKeyValues(key, [_, value])) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to set tag!", pos));
                if let Some(unit) = transporter.get_transported_mut().get_mut(index.0) {
                    unit.set_tag(key.0, value.clone());
                }
            }
            Self::UnitRemoveTag(pos, TagKeyValues(key, _)) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.remove_tag(key.0);
                }
            }
            Self::UnitRemoveTagBoarded(pos, index, TagKeyValues(key, _)) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to remove tag!", pos));
                if let Some(unit) = transporter.get_transported_mut().get_mut(index.0) {
                    unit.remove_tag(key.0);
                }
            }
            // terrain
            Self::TerrainChange(pos, _, terrain) => {
                game.get_map_mut().set_terrain(*pos, terrain.clone());
            }
            Self::TerrainFlag(pos, key) => {
                if let Some(terrain) = game.get_map_mut().get_terrain_mut(*pos) {
                    terrain.flip_flag(key.0);
                }
            }
            Self::TerrainSetTag(pos, TagKeyValues(key, [value])) |
            Self::TerrainReplaceTag(pos, TagKeyValues(key, [_, value])) => {
                if let Some(terrain) = game.get_map_mut().get_terrain_mut(*pos) {
                    terrain.set_tag(key.0, value.clone());
                }
            }
            Self::TerrainRemoveTag(pos, TagKeyValues(key, _)) => {
                if let Some(terrain) = game.get_map_mut().get_terrain_mut(*pos) {
                    terrain.remove_tag(key.0);
                }
            }
            // token
            Self::RemoveToken(p, index, _) => {
                game.get_map_mut().remove_token(*p, **index as usize);
            }
            Self::ReplaceToken(p, _, list) => {
                game.get_map_mut().set_tokens(*p, list.iter().cloned().collect());
            }
            // visual
            Self::Effect(_) |
            Self::Effects(_) => {}
        }
    }
    pub fn undo(&self, game: &mut Game<D>) {
        match self {
            // global
            Self::NextTurn => game.current_turn -= 1,
            Self::GameEnds => {
                game.set_ended(false);
            }
            // fog
            Self::PureFogChange(team, vision_changes) => {
                let team = to_client_perspective(&team);
                for (pos, intensity, _) in vision_changes {
                    game.set_fog(team, *pos, *intensity);
                }
            }
            Self::FogChange(team, changes) => {
                let team = to_client_perspective(&team);
                for (pos, intensity, fd, _, _) in changes.iter() {
                    apply_vision_changes(game, team, *pos, *intensity, fd);
                }
            }
            // player
            Self::PlayerDies(owner) => {
                game.get_owning_player_mut(owner.0).unwrap().dead = false;
            }
            Self::PurePlayerFog => (), // server doesn't hide/reveal player flags/tags
            Self::PlayerFlag(owner, flag) => {
                let environment = game.environment().clone();
                game.get_owning_player_mut(owner.0).unwrap().flip_flag(&environment, flag.0);
            }
            Self::PlayerSetTag(owner, TagKeyValues(key, [_])) => {
                game.get_owning_player_mut(owner.0).unwrap().remove_tag(key.0);
            }
            Self::PlayerRemoveTag(owner, TagKeyValues(key, [value])) |
            Self::PlayerReplaceTag(owner, TagKeyValues(key, [value, _])) => {
                let environment = game.environment().clone();
                game.get_owning_player_mut(owner.0).unwrap().set_tag(&environment, key.0, value.clone());
            }
            // commander
            Self::CommanderCharge(owner, delta) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.add_charge(-delta.0);
            }
            Self::CommanderPowerIndex(owner, index, _) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.set_active_power(**index as usize);
            }
            // hero
            Self::HeroSet(p, _) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.remove_hero();
                }
            }
            Self::HeroCharge(p, change) => {
                let environment = game.environment().clone();
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_charge(&environment, (hero.get_charge() as i32 - change.0) as u32);
                }
            }
            Self::HeroChargeTransported(p, unload_index, change) => {
                let environment = game.environment().clone();
                if let Some(mut transported) = game.get_map_mut().get_unit_mut(*p)
                .map(|u| u.get_transported_mut()) {
                    if let Some(hero) = transported.get_mut(unload_index.0)
                    .and_then(|u| u.get_hero_mut()) {
                        hero.set_charge(&environment, (hero.get_charge() as i32 - change.0) as u32);
                    }
                }
            }
            Self::HeroPower(p, index, _) => {
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_active_power(**index as usize);
                }
            }
            // unit
            Self::UnitAdd(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitRemove(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitAddBoarded(pos, _) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut();
                transported.pop();
            }
            Self::UnitRemoveBoarded(pos, index, unit) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut();
                transported.insert(index.0, unit.clone());
            }
            Self::UnitFlag(pos, key) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.flip_flag(key.0);
                }
            }
            Self::UnitFlagBoarded(pos, index, key) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to flip flag!", pos));
                if let Some(unit) = transporter.get_transported_mut().get_mut(index.0) {
                    unit.flip_flag(key.0);
                }
            }
            Self::UnitSetTag(pos, TagKeyValues(key, _)) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.remove_tag(key.0);
                }
            }
            Self::UnitSetTagBoarded(pos, index, TagKeyValues(key, _)) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to remove tag!", pos));
                if let Some(unit) = transporter.get_transported_mut().get_mut(index.0) {
                    unit.remove_tag(key.0);
                }
            }
            Self::UnitRemoveTag(pos, TagKeyValues(key, [value])) |
            Self::UnitReplaceTag(pos, TagKeyValues(key, [value, _])) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.set_tag(key.0, value.clone());
                }
            }
            Self::UnitRemoveTagBoarded(pos, index, TagKeyValues(key, [value])) |
            Self::UnitReplaceTagBoarded(pos, index, TagKeyValues(key, [value, _])) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to set tag!", pos));
                if let Some(unit) = transporter.get_transported_mut().get_mut(index.0) {
                    unit.set_tag(key.0, value.clone());
                }
            }
            // terrain
            Self::TerrainChange(pos, terrain, _) => {
                game.get_map_mut().set_terrain(*pos, terrain.clone());
            }
            Self::TerrainFlag(pos, key) => {
                if let Some(terrain) = game.get_map_mut().get_terrain_mut(*pos) {
                    terrain.flip_flag(key.0);
                }
            }
            Self::TerrainSetTag(pos, TagKeyValues(key, _)) => {
                if let Some(terrain) = game.get_map_mut().get_terrain_mut(*pos) {
                    terrain.remove_tag(key.0);
                }
            }
            Self::TerrainRemoveTag(pos, TagKeyValues(key, [value])) |
            Self::TerrainReplaceTag(pos, TagKeyValues(key, [value, _])) => {
                if let Some(terrain) = game.get_map_mut().get_terrain_mut(*pos) {
                    terrain.set_tag(key.0, value.clone());
                }
            }
            // token
            Self::RemoveToken(p, index, token) => {
                game.get_map_mut().insert_token(*p, **index as usize, token.clone());
            }
            Self::ReplaceToken(p, list, _) => {
                game.get_map_mut().set_tokens(*p, list.iter().cloned().collect());
            }
            // visual
            Self::Effect(_) |
            Self::Effects(_) => {}
        }
    }
    pub fn fog_replacement(&self, game: &Game<D>, team: ClientPerspective) -> Vec<Event<D>> {
        let board = Board::from(game);
        let mut result = Vec::new();
        match self {
            // global
            Self::NextTurn => result.push(Self::NextTurn),
            Self::GameEnds => result.push(self.clone()),
            // fog
            Self::PureFogChange(t, points) => {
                if to_client_perspective(t) == team {
                    let mut changes = LVec::new();
                    for (p, intensity_before, intensity) in points.iter() {
                        let fd = FieldData::game_field(game, *p);
                        changes.push((*p, *intensity_before, fd.clone().fog_replacement(&board, *p, *intensity_before), *intensity, fd.fog_replacement(&board, *p, *intensity)));
                    }
                    result.push(Self::FogChange(t.clone(), changes))
                }
            }
            Self::FogChange(_, _) => {
                panic!("FogChange should only ever be created as replacement for PureFogChange. It shouldn't be replaced itself!");
            }
            // player
            Self::PlayerDies(_) => result.push(self.clone()),
            Self::PurePlayerFog => {
                for player in game.players.iter()
                .filter(|p| p.get_team() != team) {
                    let foggy = player.fog_replacement();
                    for flag in player.get_tag_bag().flags()
                    .filter(|f| !foggy.has_flag(**f)) {
                        result.push(Self::PlayerFlag(Owner(player.get_owner_id()), FlagKey(*flag)));
                    }
                    for (key, value) in player.get_tag_bag().tags()
                    .filter(|(key, _)| foggy.get_tag(**key).is_none()) {
                        if game.get_fog_setting().intensity() <= FogIntensity::NormalVision {
                            result.push(Self::PlayerSetTag(Owner(player.get_owner_id()), TagKeyValues(TagKey(*key), [value.clone()])));
                        } else {
                            result.push(Self::PlayerRemoveTag(Owner(player.get_owner_id()), TagKeyValues(TagKey(*key), [value.clone()])));
                        }
                    }
                }
            }
            Self::PlayerFlag(owner, FlagKey(key)) => {
                if team == game.get_team(owner.0)
                || game.get_fog_setting().intensity() <= FogIntensity::NormalVision
                || game.environment().config.flag_visibility(*key) == UnitVisibility::AlwaysVisible {
                    result.push(self.clone())
                }
            }
            Self::PlayerSetTag(owner, TagKeyValues(TagKey(key), _)) |
            Self::PlayerRemoveTag(owner, TagKeyValues(TagKey(key), _)) |
            Self::PlayerReplaceTag(owner, TagKeyValues(TagKey(key), _)) => {
                if team == game.get_team(owner.0)
                || game.get_fog_setting().intensity() <= FogIntensity::NormalVision
                || game.environment().config.tag_visibility(*key) == UnitVisibility::AlwaysVisible {
                    result.push(self.clone())
                }
            }
            // commander
            Self::CommanderCharge(_, _) |
            Self::CommanderPowerIndex(_, _, _) => result.push(self.clone()),
            // hero
            Self::HeroSet(pos, _) => {
                if can_see_unit_at(&board, team, *pos, &game.get_unit(*pos).unwrap(), false) {
                    result.push(self.clone())
                }
            }
            Self::HeroCharge(p, _) |
            Self::HeroPower(p, _, _) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    return vec![self.clone()];
                }
                let unit = game.get_unit(*p).unwrap();
                if unit.fog_replacement(&board, *p, fog_intensity).filter(|u| u.is_hero()).is_some() {
                    result.push(self.clone())
                }
            }
            Self::HeroChargeTransported(p, unload_index, change) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    return vec![self.clone()];
                }
                let transporter = game.get_unit(*p).unwrap();
                let transporter_visibility = transporter.visibility(&board, *p);
                let transport_visibility = transporter.environment().unit_transport_visibility(&board, &transporter, *p, &[]);
                if !is_unit_attribute_visible(fog_intensity, transporter_visibility, transport_visibility) {
                    return result;
                }
                let unit = &transporter.get_transported()[unload_index.0];
                if unit.fog_replacement(&board, *p, fog_intensity).filter(|u| u.is_hero()).is_some() {
                    let i = transporter.get_transported().iter()
                    .take(unload_index.0)
                    .filter(|u| u.fog_replacement(&board, *p, fog_intensity).is_some())
                    .count();
                    result.push(Self::HeroChargeTransported(*p, i.into(), *change))
                }
            }
            // unit
            Self::UnitAdd(pos, unit) => {
                if let Some(unit) = unit.fog_replacement(&board, *pos, game.get_fog_at(team, *pos)) {
                    result.push(Self::UnitAdd(*pos, unit))
                }
            }
            Self::UnitRemove(pos, unit) => {
                if let Some(unit) = unit.fog_replacement(&board, *pos, game.get_fog_at(team, *pos)) {
                    result.push(Self::UnitRemove(*pos, unit))
                }
            }
            Self::UnitAddBoarded(p, unit) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    return vec![self.clone()];
                }
                let transporter = game.get_unit(*p).unwrap();
                let transporter_visibility = transporter.visibility(&board, *p);
                let transport_visibility = transporter.environment().unit_transport_visibility(&board, &transporter, *p, &[]);
                if !is_unit_attribute_visible(fog_intensity, transporter_visibility, transport_visibility) {
                    return result;
                }
                if let Some(unit) = unit.fog_replacement(&board, *p, fog_intensity) {
                    result.push(Self::UnitAddBoarded(*p, unit))
                }
            }
            Self::UnitRemoveBoarded(p, unload_index, unit) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    return vec![self.clone()];
                }
                let transporter = game.get_unit(*p).unwrap();
                let transporter_visibility = transporter.visibility(&board, *p);
                let transport_visibility = transporter.environment().unit_transport_visibility(&board, &transporter, *p, &[]);
                if !is_unit_attribute_visible(fog_intensity, transporter_visibility, transport_visibility) {
                    return result;
                }
                if let Some(unit) = unit.fog_replacement(&board, *p, fog_intensity) {
                    let i = transporter.get_transported().iter()
                    .take(unload_index.0)
                    .filter(|u| u.fog_replacement(&board, *p, fog_intensity).is_some())
                    .count();
                    result.push(Self::UnitRemoveBoarded(*p, i.into(), unit))
                }
            }
            Self::UnitFlag(p, FlagKey(key)) => {
                let unit = game.get_unit(*p).unwrap();
                if visible_unit_with_attribute(&board, team, *p, unit.environment().config.flag_visibility(*key)) {
                    result.push(self.clone())
                }
            }
            Self::UnitSetTag(p, TagKeyValues(TagKey(key), _)) |
            Self::UnitRemoveTag(p, TagKeyValues(TagKey(key), _)) |
            Self::UnitReplaceTag(p, TagKeyValues(TagKey(key), _)) => {
                let unit = game.get_unit(*p).unwrap();
                if visible_unit_with_attribute(&board, team, *p, unit.environment().config.tag_visibility(*key)) {
                    result.push(self.clone())
                }
            }
            Self::UnitFlagBoarded(p, unload_index, key) => {
                let unit = game.get_unit(*p).unwrap();
                if let Some(unload_index) = visible_unit_with_attribute_transported(&board, team, *p, unload_index.0, unit.environment().config.flag_visibility(key.0)) {
                    result.push(Self::UnitFlagBoarded(*p, unload_index.into(), *key))
                }
            }
            Self::UnitSetTagBoarded(p, unload_index, TagKeyValues(key, value)) => {
                let unit = game.get_unit(*p).unwrap();
                if let Some(unload_index) = visible_unit_with_attribute_transported(&board, team, *p, unload_index.0, unit.environment().config.tag_visibility(key.0)) {
                    result.push(Self::UnitSetTagBoarded(*p, unload_index.into(), TagKeyValues(*key, value.clone())))
                }
            }
            Self::UnitRemoveTagBoarded(p, unload_index, TagKeyValues(key, value)) => {
                let unit = game.get_unit(*p).unwrap();
                if let Some(unload_index) = visible_unit_with_attribute_transported(&board, team, *p, unload_index.0, unit.environment().config.tag_visibility(key.0)) {
                    result.push(Self::UnitRemoveTagBoarded(*p, unload_index.into(), TagKeyValues(*key, value.clone())))
                }
            }
            Self::UnitReplaceTagBoarded(p, unload_index, TagKeyValues(key, value)) => {
                let unit = game.get_unit(*p).unwrap();
                if let Some(unload_index) = visible_unit_with_attribute_transported(&board, team, *p, unload_index.0, unit.environment().config.tag_visibility(key.0)) {
                    result.push(Self::UnitReplaceTagBoarded(*p, unload_index.into(), TagKeyValues(*key, value.clone())))
                }
            }
            // terrain
            Self::TerrainChange(pos, before, after) => {
                let fog_intensity = game.get_fog_at(team, *pos);
                let before = before.fog_replacement(fog_intensity);
                let after = after.fog_replacement(fog_intensity);
                if before != after {
                    result.push(Self::TerrainChange(*pos, before, after))
                }
            }
            Self::TerrainFlag(p, FlagKey(key)) => {
                // terrain is AlwaysVisible
                // flag_visibility should be same as in Terrain::fog_replacement
                if match game.environment().config.flag_visibility(*key) {
                    UnitVisibility::AlwaysVisible => true,
                    UnitVisibility::Normal |
                    UnitVisibility::Stealth => game.get_fog_at(team, *p) < FogIntensity::Light,
                } {
                    result.push(self.clone())
                }
            }
            Self::TerrainSetTag(p, TagKeyValues(TagKey(key), _)) |
            Self::TerrainRemoveTag(p, TagKeyValues(TagKey(key), _)) |
            Self::TerrainReplaceTag(p, TagKeyValues(TagKey(key), _)) => {
                // terrain is AlwaysVisible
                // tag_visibility should be same as in Terrain::fog_replacement
                if match game.environment().config.tag_visibility(*key) {
                    UnitVisibility::AlwaysVisible => true,
                    UnitVisibility::Normal |
                    UnitVisibility::Stealth => game.get_fog_at(team, *p) < FogIntensity::Light,
                } {
                    result.push(self.clone())
                }
            }
            // token
            Self::RemoveToken(p, index, token) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    result.push(self.clone())
                } else if let Some(token) = token.fog_replacement(fog_intensity) {
                    let mut new_index = 0;
                    for (i, token) in game.get_tokens(*p).into_iter().enumerate() {
                        if i == **index as usize {
                            break;
                        }
                        if token.fog_replacement(fog_intensity).is_some() {
                            new_index += 1;
                        }
                    }
                    result.push(Self::RemoveToken(*p, new_index.into(), token))
                }
            }
            Self::ReplaceToken(p, old, new) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    result.push(self.clone())
                } else {
                    let old: Vec<Token<D>> = old.iter().filter_map(|token| {
                        token.fog_replacement(fog_intensity)
                    }).collect();
                    let new: Vec<Token<D>> = new.iter().filter_map(|token| {
                        token.fog_replacement(fog_intensity)
                    }).collect();
                    if old != new {
                        result.push(Self::ReplaceToken(*p, old.try_into().unwrap(), new.try_into().unwrap()))
                    }
                }
            }
            // visual
            Self::Effect(effect) => {
                if !game.has_secrets() {
                    result.push(self.clone())
                } else if let Some(effect) = effect.fog_replacement(&board, team) {
                    result.push(Self::Effect(effect))
                }
            }
            Self::Effects(effects) => {
                if !game.has_secrets() {
                    result.push(self.clone())
                } else {
                    let mut effects: Vec<_> = effects.iter()
                    .filter_map(|e| e.fog_replacement(&board, team))
                    .collect();
                    match effects.len() {
                        0 => (),
                        1 => result.push(Self::Effect(effects.pop().unwrap())),
                        _ => result.push(Self::Effects(effects.try_into().unwrap()))
                    }
                }
            }
        }
        result
    }
}


fn apply_vision_changes<D: Direction>(game: &mut Game<D>, team: ClientPerspective, pos: Point, intensity: FogIntensity, change: &FieldData<D>) {
    game.set_fog(team, pos, intensity);
    game.get_map_mut().set_terrain(pos, change.terrain.clone());
    game.get_map_mut().set_tokens(pos, change.tokens.to_vec());
    game.get_map_mut().set_unit(pos, change.unit.clone());
}
