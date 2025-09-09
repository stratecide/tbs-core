use std::cell::RefCell;
use std::marker::PhantomData;
use std::ptr::with_exposed_provenance_mut;

use interfaces::ClientPerspective;
use rhai::*;

use crate::config::environment::Environment;
use crate::game::fog::{FogIntensity, FogSetting};
use crate::game::game::Game;
use crate::map::map::{get_unit, Map};
use crate::map::pipe::PipeState;
use crate::player::Player;
use crate::script::executor::Executor;
use crate::script::CONST_NAME_BOARD;
use crate::tokens::Token;
use crate::map::direction::*;
use crate::map::point::*;
use crate::map::wrapping_map::WrappingMap;
use crate::terrain::terrain::Terrain;
use crate::units::movement::Path;
use crate::units::unit::Unit;


pub trait BoardView<D: Direction> {
    fn environment(&self) -> &Environment;
    fn wrapping_logic(&self) -> &WrappingMap<D>;

    fn get_pipes(&self, p: Point) -> &[PipeState<D>];
    fn get_terrain(&self, p: Point) -> Option<&Terrain<D>>;
    fn get_tokens(&self, p: Point) -> &[Token<D>];
    fn get_unit(&self, p: Point) -> Option<&Unit<D>>;

    fn current_owner(&self) -> i8;
    fn get_owning_player(&self, owner: i8) -> Option<&Player<D>>;
    fn get_team(&self, owner: i8) -> ClientPerspective;

    fn get_fog_setting(&self) -> FogSetting;
    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity;
}


pub enum Board<'a, D: Direction> {
    Base {
        base: &'a dyn BoardView<D>,
        limits: Limits
    },
    UnitPath {
        base: &'a Self,
        start: Point,
    },
    PutUnit {
        base: &'a Self,
        pos: Point,
        unit: Option<Unit<D>>,
    },
    IgnoreUnits {
        base: &'a Self,
    },
}

#[derive(Default)]
pub struct Limits {
    attack: RefCell<Option<usize>>,
    unit: RefCell<Option<usize>>,
    terrain: RefCell<Option<usize>>,
}

impl<'a, D: Direction> Board<'a, D> {
    pub(crate) fn new(base: &'a (dyn BoardView<D> + 'a)) -> Self {
        Self::Base {
            base,
            limits: Default::default(),
        }
    }

    pub fn executor<'b>(&'b self, mut scope: Scope<'b>) -> Executor<'b> {
        scope.push_constant(CONST_NAME_BOARD, BoardPointer::from(self));
        Executor::new(scope, self.environment().clone())
    }

    pub fn unit_path_without_placing(&'a self, unload_index: Option<usize>, path: &Path<D>) -> Option<(Self, Point, Unit<D>)> {
        if let Some(mut unit) = get_unit(self, path.start, unload_index).cloned() {
            // TODO: update fog, funds after path, ...
            // would be better to somehow wrap EventHandler, i guess?
            let (end, _) = path.end(self).unwrap();
            unit.transformed_by_path(self, path);
            Some((Self::UnitPath { base: self, start: path.start }, end, unit))
        } else {
            None
        }
    }

    pub fn replace_unit(&'a self, pos: Point, unit: Option<Unit<D>>) -> Self {
        Self::PutUnit { base: self, pos, unit }
    }

    pub fn ignore_units(&'a self) -> Self {
        Self::IgnoreUnits { base: self }
    }

    fn parent(&self) -> &dyn BoardView<D> {
        match self {
            Self::Base { base, .. } => *base,
            Self::UnitPath { base, .. } => *base,
            Self::PutUnit { base, .. } => *base,
            Self::IgnoreUnits { base } => *base,
        }
    }

    fn limits(&self) -> &Limits {
        match self {
            Self::Base { limits, .. } => limits,
            Self::UnitPath { base, .. } => base.limits(),
            Self::PutUnit { base, .. } => base.limits(),
            Self::IgnoreUnits { base } => base.limits(),
        }
    }
    pub fn get_attack_config_limit(&self) -> Option<usize> {
        *self.limits().attack.borrow()
    }
    pub fn set_attack_config_limit(&self, value: Option<usize>) {
        *self.limits().attack.borrow_mut() = value;
    }
    pub fn get_unit_config_limit(&self) -> Option<usize> {
        *self.limits().unit.borrow()
    }
    pub fn set_unit_config_limit(&self, value: Option<usize>) {
        *self.limits().unit.borrow_mut() = value;
    }
    pub fn get_terrain_config_limit(&self) -> Option<usize> {
        *self.limits().terrain.borrow()
    }
    pub fn set_terrain_config_limit(&self, value: Option<usize>) {
        *self.limits().terrain.borrow_mut() = value;
    }
}

impl<'a, D: Direction> From<&'a Map<D>> for Board<'a, D> {
    fn from(value: &'a Map<D>) -> Self {
        Self::new(value)
    }
}

impl<'a, D: Direction> From<&'a Game<D>> for Board<'a, D> {
    fn from(value: &'a Game<D>) -> Self {
        Self::new(value)
    }
}

impl<'a, D: Direction> BoardView<D> for Board<'a, D> {
    fn environment(&self) -> &Environment {
        self.parent().environment()
    }
    fn wrapping_logic(&self) -> &WrappingMap<D> {
        self.parent().wrapping_logic()
    }

    fn get_pipes(&self, p: Point) -> &[PipeState<D>] {
        self.parent().get_pipes(p)
    }
    fn get_terrain(&self, p: Point) -> Option<&Terrain<D>> {
        self.parent().get_terrain(p)
    }
    fn get_tokens(&self, p: Point) -> &[Token<D>] {
        self.parent().get_tokens(p)
    }
    fn get_unit(&self, p: Point) -> Option<&Unit<D>> {
        match self {
            Self::UnitPath { start, .. } => {
                if p == *start {
                    return None;
                }
            }
            Self::PutUnit { pos, unit, .. } => {
                if p == *pos {
                    return unit.as_ref();
                }
            }
            Self::IgnoreUnits { .. } => return None,
            _ => ()
        }
        self.parent().get_unit(p)
    }

    fn current_owner(&self) -> i8 {
        self.parent().current_owner()
    }
    fn get_owning_player(&self, owner: i8) -> Option<&Player<D>> {
        self.parent().get_owning_player(owner)
    }
    fn get_team(&self, owner: i8) -> ClientPerspective {
        self.parent().get_team(owner)
    }

    fn get_fog_setting(&self) -> FogSetting {
        self.parent().get_fog_setting()
    }
    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        self.parent().get_fog_at(team, position)
    }
}

/// Newtype wrapping a reference (pointer) cast into 'usize'
#[derive(Clone)]
pub(crate) struct BoardPointer<D: Direction> {
    ptr: usize,
    _pd: PhantomData<D>,
}

impl<D: Direction> BoardPointer<D> {
    pub(crate) fn from(value: *const Board<D>) -> Self {
        let ptr = value.expose_provenance();
        Self {
            ptr,
            _pd: PhantomData,
        }
    }

    pub(crate) fn as_ref<'a>(&'a self) -> &'a Board<'a, D> {
        let ptr: *const Board<'a, D> = with_exposed_provenance_mut(self.ptr);
        unsafe {ptr.as_ref()}
            .unwrap()
    }
}

pub fn current_team<D: Direction>(board: &impl BoardView<D>) -> ClientPerspective {
    match board.get_owning_player(board.current_owner()) {
        Some(player) => player.get_team(),
        None => ClientPerspective::Neutral,
    }
}
