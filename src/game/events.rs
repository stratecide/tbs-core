use interfaces::game_interface::{EventInterface, ClientPerspective};
use semver::Version;
use zipper::*;
use zipper::zipper_derive::*;

use crate::commander::commander_type::CommanderChargeChange;
use crate::map::map::FieldData;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::point_map;
use crate::terrain::attributes::{CaptureProgress, Anger, BuiltThisTurn};
use crate::units::attributes::{ActionStatus, AttributeKey};
use crate::units::commands::UnloadIndex;
use crate::units::hero::{Hero, HeroChargeChange};
use crate::units::unit::Unit;
use crate::units::movement::MAX_PATH_LENGTH;
use crate::{player::*, details};
use crate::terrain::terrain::*;
use crate::details::Detail;
use crate::map::direction::Direction;
use crate::game::game::*;
use crate::game::fog::*;
use crate::units::movement::PathStep;
use crate::config::environment::Environment;

use super::game_view::GameView;

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
    // unit events
    UnitAdd(Point, Unit<D>),
    UnitRemove(Point, Unit<D>),
    UnitAddBoarded(Point, Unit<D>),
    UnitRemoveBoarded(Point, UnloadIndex, Unit<D>),
    UnitHpChange(Point, I<-100, 99>, I<-999, 100>),
    UnitHpChangeBoarded(Point, UnloadIndex, I<-100, 99>),
    UnitActionStatus(Point, ActionStatus, ActionStatus),
    UnitActionStatusBoarded(Point, UnloadIndex, ActionStatus, ActionStatus),
    UnitMovedThisGame(Point),
    EnPassantOpportunity(Point, Option<Point>, Option<Point>),
    UnitDirection(Point, D, D),
    // global
    NextTurn,
    MoneyChange(Owner, Funds),
    PlayerDies(Owner),
    GameEnds,
    // visual
    Effect(Effect<D>),
    UnitPath(Option<Unit<D>>, LVec<UnitStep<D>, {MAX_PATH_LENGTH}>),
    // fog events
    PureFogChange(Perspective, LVec<(Point, FogIntensity, FogIntensity), {point_map::MAX_AREA}>),
    FogChange(Perspective, LVec<(Point, FogIntensity, FieldData<D>, FogIntensity, FieldData<D>), {point_map::MAX_AREA}>),
    PureHideFunds(Owner),
    HideFunds(Owner, Funds),
    PureRevealFunds(Owner),
    RevealFunds(Owner, Funds),
    // hero events
    HeroSet(Point, Hero, Hero),
    HeroCharge(Point, HeroChargeChange),
    HeroChargeTransported(Point, UnloadIndex, HeroChargeChange),
    HeroPower(Point, U<31>, U<31>),
    // terrain events
    TerrainChange(Point, Terrain, Terrain),
    TerrainAnger(Point, Anger, Anger),
    CaptureProgress(Point, CaptureProgress, CaptureProgress),
    UpdateBuiltThisTurn(Point, BuiltThisTurn, BuiltThisTurn),
    // detail events
    RemoveDetail(Point, U<{details::MAX_STACK_SIZE as i32 - 1}>, Detail<D>),
    ReplaceDetail(Point, LVec<Detail<D>, {details::MAX_STACK_SIZE}>, LVec<Detail<D>, {details::MAX_STACK_SIZE}>),
    // commander events
    CommanderCharge(Owner, CommanderChargeChange),
    CommanderPowerIndex(Owner, U<31>, U<31>),
}

impl<D: Direction> EventInterface for Event<D> {
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
            Self::NextTurn => game.current_turn += 1,
            Self::UnitActionStatus(pos, _, action_status) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.set_status(*action_status);
                }
            }
            Self::UnitActionStatusBoarded(pos, index, _, action_status) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                if let Some(boarded) = transported.get_mut(index.0) {
                    boarded.set_status(*action_status);
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, hp_change));
                if unit.has_attribute(AttributeKey::Hp) {
                    let hp = unit.get_hp() as i8;
                    unit.set_hp((hp + **hp_change as i8) as u8);
                }
            }
            Self::UnitHpChangeBoarded(pos, index, hp_change) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                if let Some(boarded) = transported.get_mut(index.0).filter(|u| u.has_attribute(AttributeKey::Hp)) {
                    let hp = boarded.get_hp() as i8;
                    boarded.set_hp((hp + **hp_change as i8) as u8);
                }
            }
            Self::UnitAdd(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitRemove(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to remove!", pos));
            }
            Self::UnitAddBoarded(pos, unit) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                transported.push(unit.clone());
            }
            Self::UnitRemoveBoarded(pos, index, _) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                transported.remove(index.0);
            }
            Self::MoneyChange(owner, change) => {
                if let Some(player) = game.get_owning_player_mut(owner.0) {
                    player.funds += **change;
                }
            }
            Self::PlayerDies(owner) => {
                game.get_owning_player_mut(owner.0).unwrap().dead = true;
            }
            Self::GameEnds => {
                game.set_ended(true);
            }
            Self::PureHideFunds(_) => {}
            Self::HideFunds(owner, _) => {
                if let Some(player) = game.get_owning_player_mut(owner.0) {
                    player.funds = 0.into();
                }
            }
            Self::PureRevealFunds(_) => {}
            Self::RevealFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(owner.0) {
                    player.funds = *value;
                }
            }
            Self::RemoveDetail(p, index, _) => {
                game.get_map_mut().remove_detail(*p, **index as usize);
            }
            Self::ReplaceDetail(p, _, list) => {
                game.get_map_mut().set_details(*p, list.iter().cloned().collect());
            }
            Self::Effect(_) => {}
            Self::UnitPath(_, _) => {}
            Self::CommanderCharge(owner, delta) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.add_charge(delta.0);
            }
            Self::CommanderPowerIndex(owner, _, index) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.set_active_power(**index as usize);
            }
            Self::UnitMovedThisGame(p) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.set_unmoved(!unit.get_unmoved());
                }
            }
            Self::EnPassantOpportunity(pos, _, en_passant) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.set_en_passant(*en_passant);
                }
            }
            Self::UnitDirection(p, _, new_dir) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.set_direction(*new_dir);
                }
            }
            Self::HeroSet(p, _, hero) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.set_hero(hero.clone());
                }
            }
            Self::HeroCharge(p, change) => {
                let environment = game.environment().clone();
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_charge(&environment, (hero.get_charge() as i8 + change.0) as u8);
                }
            }
            Self::HeroChargeTransported(p, unload_index, change) => {
                let environment = game.environment().clone();
                if let Some(mut transported) = game.get_map_mut().get_unit_mut(*p)
                .and_then(|u| u.get_transported_mut()) {
                    if let Some(hero) = transported.get_mut(unload_index.0)
                    .and_then(|u| u.get_hero_mut()) {
                        hero.set_charge(&environment, (hero.get_charge() as i8 + change.0) as u8);
                    }
                }
            }
            Self::HeroPower(p, _, index) => {
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_active_power(**index as usize);
                }
            }
            Self::TerrainChange(pos, _, terrain) => {
                game.get_map_mut().set_terrain(*pos, terrain.clone());
            }
            Self::TerrainAnger(pos, _, anger) => {
                game.get_map_mut().get_terrain_mut(*pos).unwrap().set_anger(anger.0);
            }
            Self::CaptureProgress(pos, _, progress) => {
                game.get_map_mut().get_terrain_mut(*pos).unwrap().set_capture_progress(*progress);
            }
            Self::UpdateBuiltThisTurn(pos, _, built_this_turn) => {
                game.get_map_mut().get_terrain_mut(*pos).unwrap().set_built_this_turn(built_this_turn.0);
            }
        }
    }
    pub fn undo(&self, game: &mut Game<D>) {
        match self {
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
            Self::NextTurn => game.current_turn -= 1,
            Self::UnitActionStatus(pos, action_status, _) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.set_status(*action_status);
                }
            }
            Self::UnitActionStatusBoarded(pos, index, action_status, _) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                if let Some(boarded) = transported.get_mut(index.0) {
                    boarded.set_status(*action_status);
                }
            }
            Self::UnitHpChange(pos, hp_change, _) => {
                let unit = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a unit at {:?} to change hp by {}!", pos, hp_change));
                if unit.has_attribute(AttributeKey::Hp) {
                    let hp = unit.get_hp() as i8;
                    unit.set_hp((hp - **hp_change as i8) as u8);
                }
            }
            Self::UnitHpChangeBoarded(pos, index, hp_change) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                if let Some(boarded) = transported.get_mut(index.0).filter(|u| u.has_attribute(AttributeKey::Hp)) {
                    let hp = boarded.get_hp() as i8;
                    boarded.set_hp((hp - **hp_change as i8) as u8);
                }
            }
            Self::UnitAdd(pos, _) => {
                game.get_map_mut().set_unit(*pos, None).expect(&format!("expected a unit at {:?} to die!", pos));
            }
            Self::UnitRemove(pos, unit) => {
                game.get_map_mut().set_unit(*pos, Some(unit.clone()));
            }
            Self::UnitAddBoarded(pos, _) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                transported.pop();
            }
            Self::UnitRemoveBoarded(pos, index, unit) => {
                let transporter = game.get_map_mut().get_unit_mut(*pos).expect(&format!("expected a transport at {:?} to change hp!", pos));
                let mut transported = transporter.get_transported_mut().expect(&format!("unit at {:?} doesn't transport units", pos));
                transported.insert(index.0, unit.clone());
            }
            Self::MoneyChange(owner, change) => {
                if let Some(player) = game.get_owning_player_mut(owner.0) {
                    player.funds -= **change;
                }
            }
            Self::PlayerDies(owner) => {
                game.get_owning_player_mut(owner.0).unwrap().dead = false;
            }
            Self::GameEnds => {
                game.set_ended(false);
            }
            Self::PureHideFunds(_) => {}
            Self::HideFunds(owner, value) => {
                if let Some(player) = game.get_owning_player_mut(owner.0) {
                    player.funds = *value;
                }
            }
            Self::PureRevealFunds(_) => {}
            Self::RevealFunds(owner, _) => {
                if let Some(player) = game.get_owning_player_mut(owner.0) {
                    player.funds = 0.into();
                }
            }
            Self::RemoveDetail(p, index, detail) => {
                game.get_map_mut().insert_detail(*p, **index as usize, detail.clone());
            }
            Self::ReplaceDetail(p, list, _) => {
                game.get_map_mut().set_details(*p, list.iter().cloned().collect());
            }
            Self::Effect(_) => {}
            Self::UnitPath(_, _) => {}
            Self::CommanderCharge(owner, delta) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.add_charge(-delta.0);
            }
            Self::CommanderPowerIndex(owner, index, _) => {
                game.get_owning_player_mut(owner.0).unwrap().commander.set_active_power(**index as usize);
            }
            Self::UnitMovedThisGame(p) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.set_unmoved(!unit.get_unmoved());
                }
            }
            Self::EnPassantOpportunity(pos, en_passant, _) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*pos) {
                    unit.set_en_passant(*en_passant);
                }
            }
            Self::UnitDirection(p, old_dir, _) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.set_direction(*old_dir);
                }
            }
            Self::HeroSet(p, hero, _) => {
                if let Some(unit) = game.get_map_mut().get_unit_mut(*p) {
                    unit.set_hero(hero.clone());
                }
            }
            Self::HeroCharge(p, change) => {
                let environment = game.environment().clone();
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_charge(&environment, (hero.get_charge() as i8 - change.0) as u8);
                }
            }
            Self::HeroChargeTransported(p, unload_index, change) => {
                let environment = game.environment().clone();
                if let Some(mut transported) = game.get_map_mut().get_unit_mut(*p)
                .and_then(|u| u.get_transported_mut()) {
                    if let Some(hero) = transported.get_mut(unload_index.0)
                    .and_then(|u| u.get_hero_mut()) {
                        hero.set_charge(&environment, (hero.get_charge() as i8 - change.0) as u8);
                    }
                }
            }
            Self::HeroPower(p, index, _) => {
                if let Some(hero) = game.get_map_mut().get_unit_mut(*p).and_then(|u| u.get_hero_mut()) {
                    hero.set_active_power(**index as usize);
                }
            }
            Self::TerrainChange(pos, terrain, _) => {
                game.get_map_mut().set_terrain(*pos, terrain.clone());
            }
            Self::TerrainAnger(pos, anger, _) => {
                game.get_map_mut().get_terrain_mut(*pos).unwrap().set_anger(anger.0);
            }
            Self::CaptureProgress(pos, progress, _) => {
                game.get_map_mut().get_terrain_mut(*pos).unwrap().set_capture_progress(*progress);
            }
            Self::UpdateBuiltThisTurn(pos, built_this_turn, _) => {
                game.get_map_mut().get_terrain_mut(*pos).unwrap().set_built_this_turn(built_this_turn.0);
            }
        }
    }
    pub fn fog_replacement(&self, game: &Game<D>, team: ClientPerspective) -> Option<Event<D>> {
        match self {
            Self::PureFogChange(t, points) => {
                if to_client_perspective(t) == team {
                    let mut changes = LVec::new();
                    for (p, intensity_before, intensity) in points.iter() {
                        let fd = game.get_map().get_field_data(*p);
                        changes.push((*p, *intensity_before, fd.clone().fog_replacement(game, *p, *intensity_before), *intensity, fd.fog_replacement(game, *p, *intensity)));
                    }
                    Some(Self::FogChange(t.clone(), changes))
                } else {
                    None
                }
            }
            Self::FogChange(_, _) => {
                panic!("FogChange should only ever be created as replacement for PureFogChange. It shouldn't be replaced itself!");
            }
            Self::NextTurn => Some(Self::NextTurn),
            Self::UnitActionStatus(p, _, _) => {
                if game.visible_unit_with_attribute(team, *p, AttributeKey::ActionStatus) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitHpChange(p, _, _) => {
                if game.visible_unit_with_attribute(team, *p, AttributeKey::Hp) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitAdd(pos, unit) => {
                if let Some(unit) = unit.fog_replacement(game, *pos, game.get_fog_at(team, *pos)) {
                    Some(Self::UnitAdd(*pos, unit))
                } else {
                    None
                }
            }
            Self::UnitRemove(pos, unit) => {
                if let Some(unit) = unit.fog_replacement(game, *pos, game.get_fog_at(team, *pos)) {
                    Some(Self::UnitRemove(*pos, unit))
                } else {
                    None
                }
            }
            Self::UnitActionStatusBoarded(p, _, _, _) |
            Self::UnitHpChangeBoarded(p, _, _) |
            Self::UnitAddBoarded(p, _) |
            Self::UnitRemoveBoarded(p, _, _) |
            Self::HeroChargeTransported(p, _, _) => {
                if game.visible_unit_with_attribute(team, *p, AttributeKey::Transported) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::MoneyChange(owner, _) => {
                if !game.is_foggy() || team == game.get_team(Some(owner.0)) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::PlayerDies(_) |
            Self::GameEnds => Some(self.clone()),
            Self::PureHideFunds(owner) => {
                if team != game.get_team(Some(owner.0)) {
                    Some(Self::HideFunds(owner.clone(), game.get_owning_player(owner.0).unwrap().funds))
                } else {
                    None
                }
            }
            Self::HideFunds(_, _) => {
                panic!("HideFunds should only ever be created as replacement for PureHideFunds. It shouldn't be replaced itself!");
            }
            Self::PureRevealFunds(owner) => {
                if team != game.get_team(Some(owner.0)) {
                    Some(Self::RevealFunds(owner.clone(), game.get_owning_player(owner.0).unwrap().funds))
                } else {
                    None
                }
            }
            Self::RevealFunds(_, _) => {
                panic!("RevealFunds should only ever be created as replacement for PureRevealFunds. It shouldn't be replaced itself!");
            }
            Self::RemoveDetail(p, index, detail) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    Some(self.clone())
                } else if let Some(detail) = detail.fog_replacement(fog_intensity) {
                    let mut new_index = 0;
                    for (i, detail) in game.get_map().get_details(*p).into_iter().enumerate() {
                        if i == **index as usize {
                            break;
                        }
                        if detail.fog_replacement(fog_intensity).is_some() {
                            new_index += 1;
                        }
                    }
                    Some(Self::RemoveDetail(*p, new_index.into(), detail))
                } else {
                    None
                }
            }
            Self::ReplaceDetail(p, old, new) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity == FogIntensity::TrueSight {
                    Some(self.clone())
                } else {
                    let old: Vec<Detail<D>> = old.iter().filter_map(|detail| {
                        detail.fog_replacement(fog_intensity)
                    }).collect();
                    let new: Vec<Detail<D>> = new.iter().filter_map(|detail| {
                        detail.fog_replacement(fog_intensity)
                    }).collect();
                    if old != new {
                        Some(Self::ReplaceDetail(*p, old.try_into().unwrap(), new.try_into().unwrap()))
                    } else {
                        None
                    }
                }
            }
            Self::Effect(effect) => {
                if !game.is_foggy() {
                    Some(self.clone())
                } else if let Some(effect) = effect.fog_replacement(game, team) {
                    Some(Self::Effect(effect))
                } else {
                    None
                }
            }
            Self::UnitPath(unit, steps) => {
                let mut unit = unit.clone().expect("UnitPath needs to have a unit before fog_replacement");
                if unit.get_team() == team {
                    return Some(self.clone());
                }
                let mut result = Vec::new();
                for step in steps {
                    if let Some(step) = step.replace_unit_path(game, team, &mut unit) {
                        result.push(step);
                    }
                }
                if result.len() == 0 {
                    None
                } else {
                    let start = result[0].get_start();
                    let unit = unit.fog_replacement(game, start, game.get_fog_at(team, start));
                    Some(Self::UnitPath(unit, result.try_into().unwrap()))
                }
            }
            Self::CommanderCharge(_, _) => {
                Some(self.clone())
            }
            Self::CommanderPowerIndex(_, _, _) => {
                Some(self.clone())
            }
            Self::UnitMovedThisGame(p) => {
                if game.visible_unit_with_attribute(team, *p, AttributeKey::Unmoved) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::EnPassantOpportunity(p, _, _) => {
                if game.visible_unit_with_attribute(team, *p, AttributeKey::EnPassant) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::UnitDirection(p, _, _) => {
                if game.visible_unit_with_attribute(team, *p, AttributeKey::Direction) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::HeroSet(pos, _, _) => {
                if game.can_see_unit_at(team, *pos, game.get_map().get_unit(*pos).unwrap(), false) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::HeroCharge(p, _) |
            Self::HeroPower(p, _, _) => {
                if game.visible_unit_with_attribute(team, *p, AttributeKey::Hero) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::TerrainChange(pos, before, after) => {
                let fog_intensity = game.get_fog_at(team, *pos);
                let before = before.fog_replacement(fog_intensity);
                let after = after.fog_replacement(fog_intensity);
                if before != after {
                    Some(Self::TerrainChange(*pos, before, after))
                } else {
                    None
                }
            }
            Self::TerrainAnger(_, _, _) => {
                Some(self.clone())
            }
            Self::CaptureProgress(pos, _, _) => {
                match game.get_fog_at(team, *pos) {
                    FogIntensity::TrueSight |
                    FogIntensity::NormalVision => {
                        Some(self.clone())
                    }
                    _ => {
                        None
                    }
                }
            }
            Self::UpdateBuiltThisTurn(p, _, _) => {
                match game.get_fog_at(team, *p) {
                    FogIntensity::TrueSight |
                    FogIntensity::NormalVision => Some(self.clone()),
                    _ => None,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 1, support_ref = Environment)]
pub enum UnitStep<D: Direction> {
    Simple(Point, PathStep<D>),
    Transform(Point, PathStep<D>, Option<Unit<D>>),
}

impl<D: Direction> UnitStep<D> {
    pub fn get_start(&self) -> Point {
        match self {
            Self::Simple(p, _) => *p,
            Self::Transform(p, _, _) => *p,
        }
    }

    pub fn get_step(&self) -> PathStep<D> {
        match self {
            Self::Simple(_, step) => *step,
            Self::Transform(_, step, _) => *step,
        }
    }

    fn replace_unit_path(&self, game: &Game<D>, team: ClientPerspective, unit: &mut Unit<D>) -> Option<Self> {
        let (p, step, unit2) = match self {
            Self::Simple(p, step) => (*p, *step, Some(unit.clone())),
            Self::Transform(p, step, unit2) => (*p, *step, unit2.clone()),
        };
        let p2 = step.progress(game.get_map(), p).unwrap().0;
        let unit1 = unit.fog_replacement(game, p, game.get_fog_at(team, p));
        let unit2 = unit2.and_then(|unit| unit.fog_replacement(game, p2, game.get_fog_at(team, p2)));
        if let Some(unit2) = unit2.clone() {
            *unit = unit2;
        }
        if unit1 == None && unit2 == None {
            None
        } else if unit1 == unit2 {
            Some(Self::Simple(p, step))
        } else {
            Some(Self::Transform(p, step, unit2))
        }
    }

}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 8, support_ref = Environment)]
pub enum Effect<D: Direction> {
    Laser(Point, D),
    Lightning(Point),
    // unit effects - only visible if the affected unit is visible
    Flame(Point),
    GunFire(Point),
    ShellFire(Point),
    Repair(Point),
    Explode(Point),
    KrakenRage(Point),
    Surprise(Point, Team),
}
impl<D: Direction> Effect<D> {
    pub fn fog_replacement(&self, game: &Game<D>, team: ClientPerspective) -> Option<Self> {
        match self {
            Self::Flame(p) |
            Self::GunFire(p) |
            Self::Repair(p) |
            Self::Explode(p) |
            Self::ShellFire(p) => {
                let fog_intensity = game.get_fog_at(team, *p);
                if fog_intensity <= FogIntensity::NormalVision {
                    Some(self.clone())
                } else {
                    None
                }
            }
            Self::KrakenRage(_) => Some(self.clone()),
            Self::Lightning(_) |
            Self::Laser(_, _) => Some(self.clone()),
            Self::Surprise(_, t) => {
                if team == ClientPerspective::Team(t.0) {
                    Some(self.clone())
                } else {
                    None
                }
            }
        }
    }
}

fn apply_vision_changes<D: Direction>(game: &mut Game<D>, team: ClientPerspective, pos: Point, intensity: FogIntensity, change: &FieldData<D>) {
    game.set_fog(team, pos, intensity);
    game.get_map_mut().set_terrain(pos, change.terrain.clone());
    game.get_map_mut().set_details(pos, change.details.to_vec());
    game.get_map_mut().set_unit(pos, change.unit.clone());
}

