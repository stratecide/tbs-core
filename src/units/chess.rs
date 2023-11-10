use std::collections::HashSet;
use std::fmt;

use crate::game::event_handler::EventHandler;
use crate::game::game::Game;
use crate::game::commands::*;
use crate::map::direction::Direction;
use crate::map::map::NeighborMode;
use crate::map::point::Point;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::*;
use crate::terrain::Terrain;

use super::*;

use zipper::zipper_derive::*;


#[derive(Debug, Zippable)]
#[zippable(bits = 6)]
pub struct ChessCommand<D: Direction> {
    pub path: Path<D>,
    pub pawn_upgrade: PawnUpgrade,
    pub castle: bool,
}
impl<D: Direction> ChessCommand<D> {
    fn check_path(game: &Game<D>, unit: &ChessUnit<D>, path_taken: &Path<D>, vision: Option<&HashMap<Point, FogIntensity>>) -> bool {
        search_path(game, &unit.as_unit(), &path_taken, vision, |path, _p, can_stop_here| {
            if can_stop_here && path == path_taken {
                return PathSearchFeedback::Found;
            } else {
                PathSearchFeedback::Rejected
            }
        }).is_some()
    }

    pub fn convert(self, handler: &mut EventHandler<D>) -> Result<Point, CommandError> {
        if !handler.get_map().is_point_valid(self.path.start) {
            return Err(CommandError::InvalidPoint(self.path.start));
        }
        let unit = check_chess_unit_can_act(handler.get_game(), self.path.start)?.clone();
        let team = handler.get_game().get_team(Some(unit.owner));
        let fog = handler.get_game().get_fog().get(&team);
        if !Self::check_path(handler.get_game(), &unit, &self.path, fog) {
            return Err(CommandError::InvalidPath);
        }
        let mut path_taken = self.path.clone();
        let mut path_taken_works = false;
        while !path_taken_works {
            path_taken_works = Self::check_path(handler.get_game(), &unit, &path_taken, None);
            if path_taken.steps.len() == 0 {
                // doesn't matter if path_taken_works is true or not at this point
                break
            } else if !path_taken_works {
                path_taken.steps.pop();
            }
        }
        let end = path_taken.end(handler.get_map())?;
        let mut recalculate_fog = false;
        if let Some(_) = handler.get_map().get_unit(end) {
            recalculate_fog = true;
            handler.unit_death(end, true);
        }
        handler.unit_path(None, &path_taken, false, true);
        match unit.typ {
            ChessUnits::Pawn(d, en_passant) => {
                match path_taken.steps[0] {
                    PathStep::Diagonal(_) => {
                        // en passant
                        for n in handler.get_map().get_neighbors(end, NeighborMode::FollowPipes) {
                            if let Some(UnitType::Chess(unit)) = handler.get_map().get_unit(n.point) {
                                match unit.typ {
                                    ChessUnits::Pawn(d, true) => {
                                        if n.direction == d && handler.get_game().get_team(Some(unit.owner)) != team {
                                            handler.unit_death(n.point, true);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
                // TODO: could transform the unit during the last PathStep's animation
                if ChessUnit::pawn_upgrades_after_path(handler.get_map(), &path_taken, d) {
                    // upgrade if "end of map" reached, i.e. no terrain ahead that the pawn can enter
                    handler.unit_replace(end, UnitType::Chess(ChessUnit {
                        typ: self.pawn_upgrade.to_chess_typ(),
                        hp: unit.hp,
                        owner: unit.owner,
                        exhausted: false, // will be exhausted after
                    }))
                } else {
                    if (path_taken.steps.len() > 1) != en_passant {
                        handler.unit_en_passant_opportunity(end);
                    }
                    let new_dir = ChessUnit::pawn_dir_after_path(handler.get_map(), &path_taken, d.clone());
                    handler.unit_direction(end, new_dir);
                }
            }
            ChessUnits::Rook(moved_this_game) => {
                if !moved_this_game {
                    handler.unit_moved_this_game(end);
                    if self.castle && path_taken.steps.len() > 0 && self.path.steps.len() == path_taken.steps.len() {
                        if let Some(dp) = ChessUnit::find_king_for_castling(handler.get_game(), path_taken.start, path_taken.steps[0].dir().unwrap(), path_taken.steps.len(), unit.owner) {
                            let mut king_path = Path::new(dp.point);
                            king_path.steps.push(PathStep::Jump(dp.direction.opposite_direction()));
                            handler.unit_path(None, &king_path, false, false);
                        }
                    }
                }
            }
            ChessUnits::King(moved_this_game) => {
                if !moved_this_game {
                    handler.unit_moved_this_game(end);
                }
            }
            _ => {}
        }
        handler.unit_exhaust(end);
        if recalculate_fog {
            handler.recalculate_fog();
        }
        Ok(path_taken.start)
    }

    pub fn exhaust_all_on_chess_board(handler: &mut EventHandler<D>, pos: Point) {
        if !handler.get_map().get_terrain(pos).and_then(|t| Some(t.is_chess())).unwrap_or(false) {
            return;
        }
        let mut to_exhaust = HashSet::new();
        handler.get_map().width_search(pos, |p| {
            if let Some(unit) = handler.get_map().get_unit(p) {
                if !unit.is_exhausted() && unit.get_owner() == Some(handler.get_game().current_player().owner_id) {
                    to_exhaust.insert(p);
                }
            }
            handler.get_map().get_terrain(p).and_then(|t| Some(t.is_chess())).unwrap_or(false)
        });
        for p in handler.get_map().all_points().into_iter().filter(|p| to_exhaust.contains(p)) {
            handler.unit_exhaust(p);
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
pub struct ChessUnit<D: Direction> {
    pub typ: ChessUnits<D>,
    pub owner: Owner,
    pub hp: Hp,
    pub exhausted: bool,
}
impl<D: Direction> ChessUnit<D> {
    pub fn new_instance(from: ChessUnits<D>, owner: Owner) -> ChessUnit<D> {
        ChessUnit {
            typ: from,
            owner,
            hp: 100.into(),
            exhausted: false,
        }
    }

    fn as_unit(&self) -> UnitType<D> {
        UnitType::Chess(self.clone())
    }

    pub fn attackable_positions(&self, game: &Game<D>, position: Point, moved: bool) -> HashSet<Point> {
        if moved {
            return HashSet::new();
        }
        match self.typ {
            ChessUnits::Pawn(d, _) => {
                pawn_attackable_positions(game.get_map(), position, d).into_iter().map(|dp| dp.0.point).collect()
            }
            _ => UnitType::Chess(self.clone()).movable_positions(game, &Path::new(position)),
        }
    }

    pub fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>> {
        let mut result = vec![];
        if path.steps.len() > 0 {
            match self.typ {
                ChessUnits::Rook(false) => {
                    if Self::find_king_for_castling(game, path.start, path.steps[0].dir().unwrap(), path.steps.len(), self.owner).is_some() {
                        result.push(UnitAction::Castle);
                    }
                    result.push(UnitAction::Wait);
                }
                ChessUnits::Pawn(d, _) => {
                    if Self::pawn_upgrades_after_path(game.get_map(), path, d.clone()) {
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Queen));
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Rook));
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Knight));
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Bishop));
                    } else {
                        result.push(UnitAction::Wait);
                    }
                }
                _ => {
                    result.push(UnitAction::Wait);
                }
            }
        }
        result
    }

    pub fn threatens(&self, _game: &Game<D>, target: &UnitType<D>) -> bool {
        match target {
            UnitType::Structure(_) => false,
            _ => true,
        }
    }

    fn find_king_for_castling(game: &Game<D>, start: Point, dir: D, path_len: usize, owner: Owner) -> Option<OrientedPoint<D>> {
        let mut rook_end = OrientedPoint::new(start, false, dir);
        for _ in 0..path_len {
            rook_end = game.get_map().get_neighbor(rook_end.point, rook_end.direction).unwrap();
        }
        if let Some(dp) = game.get_map().get_neighbor(rook_end.point, rook_end.direction) {
            if let Some(UnitType::Chess(unit)) = game.get_map().get_unit(dp.point) {
                let king = game.get_map().get_unit(dp.point).unwrap();
                let team = match game.get_team(Some(owner)) {
                    ClientPerspective::Neutral => panic!("Game with chess piece that doesn't belong to a team"),
                    ClientPerspective::Team(team) => ClientPerspective::Team(team),
                };
                if unit.typ == ChessUnits::King(false) && unit.owner == owner
                    && game.find_visible_threats(dp.point, &king, team).is_empty()
                    && game.find_visible_threats(rook_end.point, &king, team).is_empty()
                    && game.find_visible_threats(game.get_map().get_neighbor(rook_end.point, rook_end.direction.opposite_direction()).unwrap().point, &king, team).is_empty() {
                    return Some(dp);
                }
            }
        }
        None
    }

    fn pawn_dir_after_path(map: &Map<D>, path: &Path<D>, old_dir: D) -> D {
        if let Some(d) = path.steps.last().unwrap().dir() {
            let mut p = path.start;
            for step in path.steps.iter().take(path.steps.len() - 1) {
                p = step.progress(map, p).unwrap();
            }
            map.get_neighbor(p, d).unwrap().direction
        } else {
            old_dir
        }
    }
    fn pawn_upgrades_after_path(map: &Map<D>, path: &Path<D>, old_dir: D) -> bool {
        let end = path.end(map).unwrap();
        let new_dir = Self::pawn_dir_after_path(map, path, old_dir);
        map.get_terrain(end).and_then(|t| Some(t.is_chess())).unwrap_or(false)
        && map.get_neighbor(end, new_dir).is_none()
        && get_diagonal_neighbor(map, end, new_dir).is_none()
        && get_diagonal_neighbor(map, end, new_dir.rotate(true)).is_none()
    }
    fn true_vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        1
    }

    pub fn vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        match self.typ {
            ChessUnits::Pawn(_, _) => 2,
            ChessUnits::Rook(_) => 8,
            ChessUnits::Bishop => 8,
            ChessUnits::Knight => 3,
            ChessUnits::Queen => 6,
            ChessUnits::King(_) => 2,
        }
    }

    fn add_path_to_vision(&self, game: &Game<D>, start: Point, path: &[PathStep<D>], end: Point, vision: &mut HashMap<Point, FogIntensity>) {
        if path.len() <= self.true_vision_range(game, start) {
            vision.insert(end, FogIntensity::TrueSight);
        } else {
            vision.insert(end, FogIntensity::NormalVision);
        }
    }

    pub fn get_vision(&self, game: &Game<D>, pos: Point) -> HashMap<Point, FogIntensity> {
        let mut result = HashMap::new();
        result.insert(pos, FogIntensity::TrueSight);
        match self.typ {
            ChessUnits::Rook(_) => {
                for d in D::list() {
                    straight_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                }
            }
            ChessUnits::Bishop => {
                for d in D::list() {
                    diagonal_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                }
            }
            ChessUnits::Pawn(dir, _) => {
                let mut directions = vec![];
                if game.get_map().get_terrain(pos).and_then(|f| Some(f.is_chess())).unwrap_or(false) {
                    directions.push(dir);
                } else {
                    directions = D::list();
                }
                for d in directions.clone() {
                    if let Some(dp) = game.get_map().get_neighbor(pos, d) {
                        result.insert(dp.point, FogIntensity::TrueSight);
                        if game.get_map().get_terrain(pos) == Some(&Terrain::ChessPawnTile) {
                            if let Some(dp) = game.get_map().get_neighbor(dp.point, dp.direction) {
                                result.insert(dp.point, FogIntensity::NormalVision);
                            }
                        }
                    }
                }
            }
            ChessUnits::Knight => {
                for d in D::list() {
                    for turn_left in vec![true, false] {
                        if let Some(dp) = get_knight_neighbor(game.get_map(), pos, d, turn_left) {
                            result.insert(dp.point, FogIntensity::TrueSight);
                        }
                    }
                }
            }
            ChessUnits::Queen => {
                for d in D::list() {
                    straight_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                    diagonal_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                }
            }
            ChessUnits::King(_) => {
                for p in game.get_map().get_neighbors(pos, NeighborMode::FollowPipes) {
                    result.insert(p.point, FogIntensity::TrueSight);
                }
                for d in D::list() {
                    if let Some(dp) = get_diagonal_neighbor(game.get_map(), pos, d) {
                        result.insert(dp.point, FogIntensity::TrueSight);
                    }
                }
            }
        }
        result
    }
}

pub fn check_chess_unit_can_act<D: Direction>(game: &Game<D>, at: Point) -> Result<ChessUnit<D>, CommandError> {
    let terrain = game.get_map().get_terrain(at).ok_or(CommandError::InvalidPoint(at))?;
    let fog_intensity = game.get_fog_at(ClientPerspective::Team(*game.current_player().team as u8), at);
    let unit = match game.get_map().get_unit(at).and_then(|u| u.fog_replacement(terrain, fog_intensity)).ok_or(CommandError::MissingUnit)? {
        UnitType::Chess(unit) => unit,
        _ => return Err(CommandError::UnitTypeWrong),
    };
    if game.current_player().owner_id != unit.owner {
        return Err(CommandError::NotYourUnit);
    }
    if unit.exhausted {
        return Err(CommandError::UnitCannotMove);
    }
    Ok(unit)
}

fn pawn_attackable_positions<D: Direction>(map: &Map<D>, pos: Point, d: D) -> HashSet<(OrientedPoint<D>, PathStep<D>)> {
    let mut result = HashSet::new();
    if let Some(dp) = get_diagonal_neighbor(map, pos, d) {
        result.insert((OrientedPoint::new(dp.point, dp.mirrored, d), PathStep::Diagonal(d)));
    }
    if let Some(dp) = get_diagonal_neighbor(map, pos, d.rotate(true)) {
        // TODO: rotate clockwise if mirrored?
        result.insert((OrientedPoint::new(dp.point, dp.mirrored, dp.direction.rotate(dp.mirrored)), PathStep::Diagonal(d.rotate(true))));
    }
    result
}

pub fn find_king_steps<D: Direction>() -> Vec<PathStep<D>> {
    let mut result = Vec::new();
    for d in D::list() {
        result.push(PathStep::Dir(d));
        result.push(PathStep::Diagonal(d));
    }
    result
}

pub fn find_queen_steps<D: Direction>(previous_step_reversed: Option<PathStep<D>>) -> Vec<PathStep<D>> {
    let mut result = Vec::new();
    match previous_step_reversed {
        None => {
            for d in D::list() {
                result.push(PathStep::Dir(d));
                result.push(PathStep::Diagonal(d));
            }
        }
        Some(PathStep::Dir(d)) => {
            result.push(PathStep::Dir(d.opposite_direction()));
        }
        Some(PathStep::Diagonal(d)) => {
            result.push(PathStep::Diagonal(d.opposite_direction()));
        }
        _ => panic!("Queen's last step can't be {:?}", previous_step_reversed),
    }
    result
}

pub fn find_knight_steps<D: Direction>() -> Vec<PathStep<D>> {
    let mut result = Vec::new();
    for d in D::list() {
        for turn_left in vec![true, false] {
            result.push(PathStep::Knight(d, turn_left));
        }
    }
    result
}

pub fn find_rook_steps<D: Direction>(dir: Option<D>) -> Vec<PathStep<D>> {
    let mut result = Vec::new();
    match dir {
        None => {
            for d in D::list() {
                result.push(PathStep::Dir(d));
            }
        }
        Some(d) => {
            result.push(PathStep::Dir(d));
        }
    }
    result
}

pub fn find_bishop_steps<D: Direction>(dir: Option<D>) -> Vec<PathStep<D>> {
    let mut result = Vec::new();
    match dir {
        None => {
            for d in D::list() {
                result.push(PathStep::Diagonal(d));
            }
        }
        Some(d) => {
            result.push(PathStep::Diagonal(d));
        }
    }
    result
}

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 4)]
pub enum PawnUpgrade {
    Rook,
    Bishop,
    Knight,
    Queen,
}
impl PawnUpgrade {
    fn to_chess_typ<D: Direction>(&self) -> ChessUnits<D> {
        match self {
            Self::Rook => ChessUnits::Rook(true),
            Self::Bishop => ChessUnits::Bishop,
            Self::Knight => ChessUnits::Knight,
            Self::Queen => ChessUnits::Queen,
        }
    }
}

impl fmt::Display for PawnUpgrade {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Rook => write!(f, "Rook"),
            Self::Bishop => write!(f, "Bishop"),
            Self::Knight => write!(f, "Knight"),
            Self::Queen => write!(f, "Queen"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 4)]
pub enum ChessUnits<D: Direction> {
    Pawn(D, bool),
    Rook(bool),
    Bishop,
    Knight,
    Queen,
    King(bool),
}
impl<D: Direction> ChessUnits<D> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pawn(_, _) => "Pawn",
            Self::Rook(_) => "Rook",
            Self::Bishop => "Bishop",
            Self::Knight => "Knight",
            Self::Queen => "Queen",
            Self::King(_) => "King",
        }
    }
    pub fn get_movement(&self) -> MovementPoints {
        match self {
            Self::Pawn(_, _) => MovementPoints::from(0.),
            Self::Rook(_) =>MovementPoints::from(8.),
            Self::Bishop =>MovementPoints::from(8.),
            Self::Knight => MovementPoints::from(0.),
            Self::Queen =>MovementPoints::from(8.),
            Self::King(_) => MovementPoints::from(0.),
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Pawn(_, _) => (ArmorType::Infantry, 1.5),
            Self::Rook(_) => (ArmorType::Light, 1.5),
            Self::Bishop => (ArmorType::Light, 1.5),
            Self::Knight => (ArmorType::Heli, 1.5),
            Self::Queen => (ArmorType::Heavy, 2.0),
            Self::King(_) => (ArmorType::Heavy, 2.5),
        }
    }
    pub fn value(&self) -> u16 {
        match self {
            Self::Pawn(_, _) => 100,
            Self::Rook(_) => 500,
            Self::Bishop => 300,
            Self::Knight => 300,
            Self::Queen => 1200,
            Self::King(_) => 800,
        }
    }
    pub fn flip_moved_this_game(&mut self) {
        match self {
            Self::Rook(m) => *m = !*m,
            Self::King(m) => *m = !*m,
            _ => {}
        }
    }
}

// rotated slightly counter-clockwise compared to dir
pub fn get_diagonal_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D) -> Option<OrientedPoint<D>> {
    if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir) {
        if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction.rotate(dp1.mirrored)) {
            return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction.rotate(dp1.mirrored == dp2.mirrored)));
        }
    }
    if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir.rotate(false)) {
        if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction.rotate(!dp1.mirrored)) {
            return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
        }
    }
    None
}

pub fn get_knight_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D, turn_left: bool) -> Option<OrientedPoint<D>> {
    if turn_left {
        if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir) {
            if let Some(dp2) = get_diagonal_neighbor(map, dp1.point, dp1.direction) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
            }
        }
        if let Some(dp1) = get_diagonal_neighbor(map, p, dir) {
            if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
            }
        }
    } else {
        if let Some(dp1) = map.wrapping_logic().get_neighbor(p, dir) {
            if let Some(dp2) = get_diagonal_neighbor(map, dp1.point, dp1.direction.rotate(!dp1.mirrored)) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction.rotate(dp1.mirrored != dp2.mirrored)));
            }
        }
        if let Some(dp1) = get_diagonal_neighbor(map, p, dir.rotate(true)) {
            if let Some(dp2) = map.wrapping_logic().get_neighbor(dp1.point, dp1.direction.rotate(dp1.mirrored)) {
                return Some(OrientedPoint::new(dp2.point, dp1.mirrored != dp2.mirrored, dp2.direction));
            }
        }
    };
    None
}

// callback returns true if the search can be aborted
// if team is None, units will be ignored
fn straight_search<D, F>(game: &Game<D>, start: Point, direction: D, max_cost: Option<MovementPoints>, team: ClientPerspective, ignore_unseen: bool, mut callback: F)
where D: Direction, F: FnMut(Point, &Vec<PathStep<D>>) -> bool {
    let mut cost = MovementPoints::from(0.);
    let mut blocked_positions = HashMap::new();
    blocked_positions.insert(start, direction);
    let mut steps = vec![];
    let mut dp = OrientedPoint::new(start, false, direction);
    loop {
        steps.push(PathStep::Dir(dp.direction));
        if let Some(next_dp) = game.get_map().get_neighbor(dp.point, dp.direction) {
            if blocked_positions.get(&next_dp.point).and_then(|d| Some(*d == next_dp.direction || d.opposite_direction() == next_dp.direction)).unwrap_or(false) {
                break;
            }
            if let Some(c) = game.get_map().get_terrain(next_dp.point).and_then(|t| t.movement_cost(MovementType::Chess)) {
                if let Some(max_cost) = max_cost {
                    if cost + c > max_cost {
                        break;
                    }
                }
                if let ClientPerspective::Team(team) = team {
                    if let Some(unit) = game.get_map().get_unit(next_dp.point) {
                        if !ignore_unseen || game.can_see_unit_at(ClientPerspective::Team(team), next_dp.point, unit, true) {
                            if unit.killable_by_chess(team.into(), game) {
                                callback(next_dp.point, &steps);
                            }
                            break;
                        }
                    }
                }
                cost += c;
                dp = next_dp;
                if callback(dp.point, &steps) {
                    break;
                }
                blocked_positions.insert(dp.point, dp.direction);
            } else {
                break;
            }
        } else {
            break;
        }
    }

}

// callback returns true if the search can be aborted
// if team is None, units will be ignored
fn diagonal_search<D, F>(game: &Game<D>, start: Point, direction: D, max_cost: Option<MovementPoints>, team: ClientPerspective, ignore_unseen: bool, mut callback: F)
where D: Direction, F: FnMut(Point, &Vec<PathStep<D>>) -> bool {
    let mut cost = MovementPoints::from(0.);
    let mut blocked_positions = HashMap::new();
    blocked_positions.insert(start, direction);
    let mut steps = vec![];
    let mut dp = OrientedPoint::new(start, false, direction);
    loop {
        steps.push(PathStep::Diagonal(dp.direction));
        if let Some(next_dp) = get_diagonal_neighbor(game.get_map(), dp.point, dp.direction) {
            if blocked_positions.get(&next_dp.point).and_then(|d| Some(*d == next_dp.direction || d.opposite_direction() == next_dp.direction)).unwrap_or(false) {
                break;
            }
            if let Some(c) = game.get_map().get_terrain(next_dp.point).and_then(|t| t.movement_cost(MovementType::Chess)) {
                if let Some(max_cost) = max_cost {
                    if cost + c > max_cost {
                        break;
                    }
                }
                if let ClientPerspective::Team(team) = team {
                    if let Some(unit) = game.get_map().get_unit(next_dp.point) {
                        if !ignore_unseen || game.can_see_unit_at(ClientPerspective::Team(team), next_dp.point, unit, true) {
                            if unit.killable_by_chess(team.into(), game) {
                                callback(next_dp.point, &steps);
                            }
                            break;
                        }
                    }
                }
                cost += c;
                dp = next_dp;
                if callback(dp.point, &steps) {
                    break;
                }
                blocked_positions.insert(dp.point, dp.direction);
            }
        } else {
            break;
        }
    }

}

