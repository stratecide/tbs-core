use std::collections::HashMap;

use interfaces::game_interface::ClientPerspective;

use crate::config::environment::Environment;
use crate::details::Detail;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::map::wrapping_map::WrappingMap;
use crate::player::Player;
use crate::terrain::terrain::Terrain;
use crate::units::hero::{Hero, HeroInfluence};
use crate::units::movement::{Path, PermanentBallast};
use crate::units::unit::Unit;

use super::fog::FogIntensity;
use super::game_view::GameView;
use super::Direction;

pub(crate) struct IgnoreUnits<'a, D: Direction>(Box<&'a dyn GameView<D>>);

impl<'a, D: Direction> IgnoreUnits<'a, D> {
    pub fn new(base: &'a impl GameView<D>) -> Self {
        Self (Box::new(base))
    }
}

impl<'a, D: Direction> MapView<D> for IgnoreUnits<'a, D> {
    fn environment(&self) -> &Environment {
        self.0.environment()
    }

    fn all_points(&self) -> Vec<Point> {
        self.0.all_points()
    }

    fn wrapping_logic(&self) -> &WrappingMap<D> {
        self.0.wrapping_logic()
    }

    fn get_terrain(&self, p: Point) -> Option<&Terrain> {
        self.0.get_terrain(p)
    }

    fn get_details(&self, p: Point) -> &[Detail<D>] {
        self.0.get_details(p)
    }

    fn get_unit(&self, _: Point) -> Option<&Unit<D>> {
        None
    }
}

impl<'a, D: Direction> GameView<D> for IgnoreUnits<'a, D> {
    fn get_owning_player(&self, owner: i8) -> Option<&Player> {
        self.0.get_owning_player(owner)
    }

    fn fog_intensity(&self) -> FogIntensity {
        self.0.fog_intensity()
    }

    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        self.0.get_fog_at(team, position)
    }

    fn get_visible_unit(&self, _: ClientPerspective, _: Point) -> Option<Unit<D>> {
        None
    }
}

/**
 * allows removing, adding, moving of units
 * used in movement planning, finding attack-vectors
 */
#[derive(Clone)]
pub struct UnitMovementView<'a, D: Direction> {
    base: Box<&'a dyn GameView<D>>,
    units: HashMap<Point, Option<Unit<D>>>,
    players: HashMap<i8, Player>,
}

impl<'a, D: Direction> UnitMovementView<'a, D> {
    pub fn new(base: &'a impl GameView<D>) -> Self {
        Self {
            base: Box::new(base),
            units: HashMap::new(),
            players: HashMap::new(),
        }
    }

    pub fn remove_unit(&mut self, pos: Point, unload_index: Option<usize>) -> Option<Unit<D>> {
        if let Some(unit) = self.get_unit(pos) {
            if let Some(index) = unload_index {
                if unit.get_transported().len() > index {
                    let mut unit = unit.clone();
                    let u = unit.get_transported_mut().unwrap().remove(index);
                    self.units.insert(pos, Some(unit));
                    Some(u)
                } else {
                    None
                }
            } else {
                let result = Some(unit.clone());
                self.units.insert(pos, None);
                result
            }
        } else {
            None
        }
    }

    pub fn put_unit(&mut self, pos: Point, unit: Unit<D>) {
        self.units.insert(pos, Some(unit));
    }

    pub fn unit_path_without_placing(&mut self, unload_index: Option<usize>, path: &Path<D>) -> Option<(Point, Unit<D>)> {
        if let Some(mut unit) = self.remove_unit(path.start, unload_index) {
            // TODO: update fog
            if let Some(player) = unit.get_player(self) {
                let funds = player.funds_after_path(self, path);
                if funds != *player.funds {
                    let mut player = player.clone();
                    player.funds = funds.into();
                    self.players.insert(player.get_owner_id(), player);
                }
            }
            let (end, _) = path.end(self).unwrap();
            unit.transformed_by_path(self, path);
            Some((end, unit))
        } else {
            None
        }
    }
}

impl<'a, D: Direction> MapView<D> for UnitMovementView<'a, D> {
    fn environment(&self) -> &Environment {
        self.base.environment()
    }

    fn all_points(&self) -> Vec<Point> {
        self.base.all_points()
    }

    fn wrapping_logic(&self) -> &WrappingMap<D> {
        self.base.wrapping_logic()
    }

    fn get_terrain(&self, p: Point) -> Option<&Terrain> {
        self.base.get_terrain(p)
    }

    fn get_details(&self, p: Point) -> &[Detail<D>] {
        self.base.get_details(p)
    }

    fn get_unit(&self, p: Point) -> Option<&Unit<D>> {
        self.units.get(&p).map(|u| u.as_ref())
        .unwrap_or(self.base.get_unit(p))
    }
}

impl<'a, D: Direction> GameView<D> for UnitMovementView<'a, D> {
    fn get_owning_player(&self, owner: i8) -> Option<&Player> {
        self.players.get(&owner).or(self.base.get_owning_player(owner))
    }

    fn fog_intensity(&self) -> FogIntensity {
        self.base.fog_intensity()
    }

    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        self.base.get_fog_at(team, position)
    }

    fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
        self.get_unit(p)
        .and_then(|u| {
            // use base's fog instead of self.get_fog_at
            // maybe it should be possible to predict the fog (i.e. modify fog in this view)
            // but that shouldn't influence the output of this method because
            // while it can be predicted where fog will be lifted, it can't be predicted which units are revealed
            u.fog_replacement(self, p, self.base.get_fog_at(team, p))
        })
    }

    fn additional_hero_influence_at(&self, point: Point, only_owner_id: i8) -> Option<Vec<HeroInfluence<D>>> {
        self.base.additional_hero_influence_at(point, only_owner_id)
    }

    fn additional_hero_influence_map(&self, only_owner_id: i8) -> Option<HashMap<(Point, i8), Vec<HeroInfluence<D>>>> {
        self.base.additional_hero_influence_map(only_owner_id)
    }
}


#[derive(Clone)]
pub(crate) struct MovingHeroView<'a, D: Direction> {
    map: UnitMovementView<'a, D>,
    hero_pos: Option<Point>,
    hero_unit: Unit<D>,
    transporter: Option<(Unit<D>, usize)>,
    round: usize,
}

impl<'a, D: Direction> MovingHeroView<'a, D> {
    pub fn new(map: &'a impl GameView<D>, unit: &Unit<D>, unit_origin: Option<(Point, Option<usize>)>) -> Self {
        let mut map = UnitMovementView::new(map);
        let mut transporter = None;
        if let Some((pos, unload_index)) = unit_origin {
            map.remove_unit(pos, unload_index);
            if let Some(unload_index) = unload_index {
                transporter = Some((map.get_unit(pos).unwrap().clone(), unload_index));
            }
        }
        Self {
            map,
            hero_pos: None,
            hero_unit: unit.clone(),
            transporter,
            round: 0,
        }
    }

    pub fn update_hero(&mut self, pos: Point, permanent: &PermanentBallast<D>, round: usize) {
        self.hero_pos = Some(pos);
        permanent.update_unit(&mut self.hero_unit);
        self.round = round;
    }

    pub fn get_hero(&self) -> &Unit<D> {
        &self.hero_unit
    }

    pub fn get_transporter(&self) -> Option<(&Unit<D>, usize)> {
        self.transporter.as_ref()
        .filter(|_| self.round == 0)
        .map(|(u, i)| (u, *i))
    }
}

impl<'a, D: Direction> MapView<D> for MovingHeroView<'a, D> {
    fn environment(&self) -> &Environment {
        self.map.environment()
    }

    fn all_points(&self) -> Vec<Point> {
        self.map.all_points()
    }

    fn wrapping_logic(&self) -> &WrappingMap<D> {
        self.map.wrapping_logic()
    }

    fn get_terrain(&self, p: Point) -> Option<&Terrain> {
        self.map.get_terrain(p)
    }

    fn get_details(&self, p: Point) -> &[Detail<D>] {
        self.map.get_details(p)
    }

    fn get_unit(&self, p: Point) -> Option<&Unit<D>> {
        self.map.get_unit(p)
    }
}

impl<'a, D: Direction> GameView<D> for MovingHeroView<'a, D> {
    fn get_owning_player(&self, owner: i8) -> Option<&Player> {
        self.map.get_owning_player(owner)
    }

    fn fog_intensity(&self) -> FogIntensity {
        self.map.fog_intensity()
    }

    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        self.map.get_fog_at(team, position)
    }

    fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
        self.map.get_visible_unit(team, p)
    }

    fn additional_hero_influence_at(&self, point: Point, only_owner_id: i8) -> Option<Vec<HeroInfluence<D>>> {
        if !self.hero_unit.is_hero() || self.hero_unit.get_owner_id() != only_owner_id {
            return None;
        }
        let pos = self.hero_pos?;
        let unit = self.hero_unit.clone();
        if let Some(strength) = Hero::aura(self, &unit, pos, self.get_transporter()).get(&point) {
            let hero = unit.get_hero();
            Some(vec![(unit, hero, pos, self.get_transporter().map(|(_, unload_index)| unload_index), *strength as u8)])
        } else {
            None
        }
    }

    fn additional_hero_influence_map(&self, only_owner_id: i8) -> Option<HashMap<(Point, i8), Vec<HeroInfluence<D>>>> {
        if !self.hero_unit.is_hero() || self.hero_unit.get_owner_id() != only_owner_id {
            return None;
        }
        let pos = self.hero_pos?;
        let unit = self.hero_unit.clone();
        let mut result: HashMap<(Point, i8), Vec<HeroInfluence<D>>> = HashMap::new();
        let hero = unit.get_hero();
        for (p, strength) in Hero::aura(self, &unit, pos, self.get_transporter()) {
            let key = (p, unit.get_owner_id());
            result.insert(key, vec![(unit.clone(), hero.clone(), p, self.get_transporter().map(|(_, unload_index)| unload_index), strength as u8)]);
        }
        Some(result)
    }
}
