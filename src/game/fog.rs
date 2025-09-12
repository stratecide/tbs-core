use std::fmt::Display;

use interfaces::ClientPerspective;
use rustc_hash::FxHashMap;
use zipper::*;
use zipper::zipper_derive::*;

use crate::config::file_loader::FileLoader;
use crate::config::parse::FromConfig;
use crate::map::board::{Board, BoardView};
use crate::map::map::valid_points;
use crate::map::point::Point;
use crate::units::hero::HeroMap;
use crate::units::unit::Unit;
use crate::units::UnitVisibility;

use super::Direction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Zippable, PartialOrd, Ord, Hash)]
#[zippable(bits = 2)]
pub enum FogIntensity {
    TrueSight, // even stealthed units are visible
    NormalVision, // stealth is hidden, some terrain may hide units, rest is visible
    Light, // terrain is grey, for non-structures unit types and owners are hidden
    Dark, // you see structures, other units are hidden
}

impl FromConfig for FogIntensity {
    fn from_conf<'a>(s: &'a str, _: &mut FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match base {
            "TrueSight" => Ok((Self::TrueSight, s)),
            "NormalVision" => Ok((Self::NormalVision, s)),
            "Light" => Ok((Self::Light, s)),
            "Dark" => Ok((Self::Dark, s)),
            _ => Err(crate::config::ConfigParseError::UnknownEnumMember(format!("FogIntensity::{base} - {s}")))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogSetting {
    None,
    // highest intensity is half, u8 is bonus vision range
    Light(u8),
    // highest intensity is full
    // no half-intensity
    Sharp(u8),
    // the outer-most layer of vision is replaced by half-vision
    Fade1(u8),
    // the two outer-most layers of vision are replaced by half-vision
    Fade2(u8),
    // normal vision is replaced by half-vision
    ExtraDark(u8)
}

impl Display for FogSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FogSetting::None => write!(f, "No Fog"),
            FogSetting::Light(bonus) => write!(f, "Twilight (+{bonus})"),
            FogSetting::Sharp(bonus) => write!(f, "Sharp (+{bonus})"),
            FogSetting::Fade1(bonus) => write!(f, "Fade 1 (+{bonus})"),
            FogSetting::Fade2(bonus) => write!(f, "Fade 2 (+{bonus})"),
            FogSetting::ExtraDark(bonus) => write!(f, "Extra Dark (+{bonus})"),
        }
    }
}

impl Zippable for FogSetting {
    fn zip(&self, zipper: &mut Zipper) {
        let (index, bonus_vision): (u8, Option<u8>) = match self {
            Self::None => (0, None),
            Self::Light(b) => (1, Some(*b)),
            Self::Sharp(b) => (2, Some(*b)),
            Self::Fade1(b) => (3, Some(*b)),
            Self::Fade2(b) => (4, Some(*b)),
            Self::ExtraDark(b) => (5, Some(*b)),
        };
        zipper.write_u8(index, 3);
        if let Some(bonus_vision) = bonus_vision {
            zipper.write_u8(bonus_vision, 2);
        }
    }
    fn unzip(unzipper: &mut Unzipper) -> Result<Self, ZipperError> {
        Ok(match unzipper.read_u8(3)? {
            0 => Self::None,
            1 => Self::Light(unzipper.read_u8(2)?),
            2 => Self::Sharp(unzipper.read_u8(2)?),
            3 => Self::Fade1(unzipper.read_u8(2)?),
            4 => Self::Fade2(unzipper.read_u8(2)?),
            5 => Self::ExtraDark(unzipper.read_u8(2)?),
            _ => return Err(ZipperError::EnumOutOfBounds("FogSetting".to_string())),
        })
    }
}

impl FogSetting {
    pub const GRADIENT_WITH_NONE: &'static [Self] = &[
        Self::None,
        Self::Sharp(2),
        Self::Sharp(1),
        Self::Sharp(0),
    ];
    pub const GRADIENT_DARK: &'static [Self] = &[
        Self::Fade1(2),
        Self::Fade2(1),
        Self::ExtraDark(0),
    ];
    pub const GRADIENT_LIGHT: &'static [Self] = &[
        Self::Light(0),
        Self::Fade2(3),
        Self::Fade2(1),
    ];
    pub const GRADIENT_LARGE: &'static [Self] = &[
        Self::Light(0),
        Self::Fade2(3),
        Self::Fade2(2),
        Self::Fade2(1),
        Self::Fade2(0),
        Self::ExtraDark(0),
    ];

    pub fn intensity(&self) -> FogIntensity {
        match self {
            Self::None => FogIntensity::TrueSight,
            Self::Light(_) => FogIntensity::Light,
            _ => FogIntensity::Dark,
        }
    }
}

pub type FogDuration = I<1, 255>;

#[derive(Debug, Clone, PartialEq, Eq, Zippable)]
#[zippable(bits = 4)]
pub enum FogMode {
    Constant(FogSetting),
    GradientWithNone(FogDuration, FogDuration, bool),
    GradientDark(FogDuration, FogDuration, bool),
    GradientLight(FogDuration, FogDuration, bool),
    GradientLarge(FogDuration, FogDuration, bool),
}

impl Display for FogMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let gradient = match self {
            Self::Constant(setting) => return write!(f, "{setting}"),
            Self::GradientWithNone(_, _, _) => FogSetting::GRADIENT_WITH_NONE,
            Self::GradientDark(_, _, _) => FogSetting::GRADIENT_DARK,
            Self::GradientLight(_, _, _) => FogSetting::GRADIENT_LIGHT,
            Self::GradientLarge(_, _, _) => FogSetting::GRADIENT_LARGE,
        };
        write!(f, "{} <-> {}", gradient[0], gradient[gradient.len() - 1])
    }
}

impl FogMode {
    pub fn is_foggy(&self, turn: usize, player_count: usize) -> bool {
        self.fog_setting(turn, player_count) != FogSetting::None
    }

    // should never return FogIntensity::NormalVision
    pub fn fog_setting(&self, turn: usize, player_count: usize) -> FogSetting {
        let (bright_duration, dark_duration, start_dark, gradient) = match self {
            Self::Constant(setting) => return *setting,
            Self::GradientWithNone(bright_duration, dark_duration, start_dark) => (bright_duration, dark_duration, start_dark, FogSetting::GRADIENT_WITH_NONE),
            Self::GradientDark(bright_duration, dark_duration, start_dark) => (bright_duration, dark_duration, start_dark, FogSetting::GRADIENT_DARK),
            Self::GradientLight(bright_duration, dark_duration, start_dark) => (bright_duration, dark_duration, start_dark, FogSetting::GRADIENT_LIGHT),
            Self::GradientLarge(bright_duration, dark_duration, start_dark) => (bright_duration, dark_duration, start_dark, FogSetting::GRADIENT_LARGE),
        };
        gradient_progress(gradient, *bright_duration, *dark_duration, *start_dark, turn, player_count)
    }

    pub fn turns_until_repeat(&self, player_count: usize) -> usize {
        let (bright_duration, dark_duration, gradient) = match self {
            Self::Constant(_) => return 1,
            Self::GradientWithNone(bright_duration, dark_duration, _) => (bright_duration, dark_duration, FogSetting::GRADIENT_WITH_NONE),
            Self::GradientDark(bright_duration, dark_duration, _) => (bright_duration, dark_duration, FogSetting::GRADIENT_DARK),
            Self::GradientLight(bright_duration, dark_duration, _) => (bright_duration, dark_duration, FogSetting::GRADIENT_LIGHT),
            Self::GradientLarge(bright_duration, dark_duration, _) => (bright_duration, dark_duration, FogSetting::GRADIENT_LARGE),
        };
        **bright_duration as usize + **dark_duration as usize + 2 * intermediate_turns(gradient, player_count)
    }
}

/**
 * Every intermediate settings is used K times, such that the result
 * is the biggest value <= player_count - 1
 */
fn intermediate_turns(gradient: &[FogSetting], player_count: usize) -> usize {
    if gradient.len() <= 2 {
        panic!("not much of a fog gradient when there are only {} steps", gradient.len());
    }
    let intermediate_settings = gradient.len() - 2;
    ((player_count - 1) / intermediate_settings).max(1) * intermediate_settings
}

fn gradient_progress(gradient: &[FogSetting], bright_duration: FogDuration, dark_duration: FogDuration, start_dark: bool, turn: usize, player_count: usize) -> FogSetting {
    let gradient_duration = intermediate_turns(gradient, player_count);
    let mut progress = turn;
    if start_dark {
        progress += *bright_duration as usize + gradient_duration;
    }
    let cycle_duration = *bright_duration as usize + *dark_duration as usize + 2 * gradient_duration;
    let progress = progress % cycle_duration;
    if progress < *bright_duration as usize {
        gradient[0]
    } else if progress < *bright_duration as usize + gradient_duration {
        let progress = progress - *bright_duration as usize;
        gradient[1 + progress * (gradient.len() - 2) / gradient_duration]
    } else if progress < *bright_duration as usize + gradient_duration + *dark_duration as usize {
        gradient[gradient.len() - 1]
    } else {
        let progress = cycle_duration - progress - 1;
        gradient[1 + progress * (gradient.len() - 2) / gradient_duration]
    }
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy)]
    pub enum VisionMode {
        Normal,
        Movement,
    }
}

impl VisionMode {
    pub fn see_while_moving(&self) -> bool {
        match self {
            Self::Normal => true,
            Self::Movement => false,
        }
    }
}

pub fn is_foggy<D: Direction>(board: &impl BoardView<D>) -> bool {
    board.get_fog_setting().intensity() != FogIntensity::TrueSight
}

pub fn get_visible_unit<D: Direction>(board: &Board<D>, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
    board.get_unit(p)
    .and_then(|u| {
        // use base's fog instead of game.get_fog_at
        // when the server verifies a unit's available actions, units invisible to the player shouldn't have an influence
        // but maybe it should be possible to predict the fog
        u.fog_replacement(board, p, board.get_fog_at(team, p))
    })
}

pub fn recalculate_fog<D: Direction>(game: &Board<D>, perspective: ClientPerspective) -> FxHashMap<Point, FogIntensity> {
    let mut fog = FxHashMap::default();
    let strongest_intensity = game.get_fog_setting().intensity();
    for p in valid_points(game) {
        fog.insert(p, strongest_intensity);
    }
    if !is_foggy(game) {
        return fog;
    }
    let heroes = HeroMap::new(game, None);
    for p in valid_points(game) {
        let terrain = game.get_terrain(p).unwrap();
        let terrain_heroes = if terrain.get_team() != ClientPerspective::Neutral {
            heroes.get(p, terrain.get_owner_id())
        } else {
            &[]
        };
        for (p, v) in terrain.get_vision(game, p, terrain_heroes, perspective) {
            fog.insert(p, v.min(fog.get(&p).clone().unwrap().clone()));
        }
        if let Some(unit) = game.get_unit(p) {
            if perspective != ClientPerspective::Neutral && perspective == unit.get_team() {
                for (p, v) in unit.get_vision(game, p, &heroes) {
                    fog.insert(p, v.min(fog.get(&p).clone().unwrap().clone()));
                }
            }
        }
        for token in game.get_tokens(p) {
            for (p, v) in token.get_vision(game, p, perspective) {
                fog.insert(p, v.min(fog.get(&p).clone().unwrap().clone()));
            }
        }
    }
    fog
}

pub fn add_vision(vision: &mut FxHashMap<Point, FogIntensity>, to_add: &FxHashMap<Point, FogIntensity>) {
    for (p, intensity) in to_add {
        let old = vision.remove(p).unwrap_or(FogIntensity::Dark);
        vision.insert(*p, old.min(*intensity));
    }
}

pub fn can_see_unit_at<D: Direction>(game: &Board<D>, team: ClientPerspective, position: Point, unit: &Unit<D>, accept_unknowns: bool) -> bool {
    match unit.fog_replacement(game, position, game.get_fog_at(team, position)) {
        None => false,
        Some(unit) => accept_unknowns || unit.typ() != unit.environment().config.unknown_unit(),
    }
}

pub fn is_unit_visible<D: Direction>(board: &Board<D>, unit: &Unit<D>, p: Point, team: ClientPerspective) -> bool {
    let fog_intensity = board.get_fog_at(team, p);
    let unit_visibility = unit.visibility(board, p, None);
    match fog_intensity {
        FogIntensity::TrueSight => true,
        FogIntensity::NormalVision => unit_visibility != UnitVisibility::Stealth,
        FogIntensity::Light => unit_visibility == UnitVisibility::AlwaysVisible,
        FogIntensity::Dark => unit_visibility == UnitVisibility::AlwaysVisible,
    }
}

pub fn is_unit_attribute_visible(fog_intensity: FogIntensity, unit_visibility: UnitVisibility, attribute_visibility: UnitVisibility) -> bool {
    match fog_intensity {
        FogIntensity::TrueSight => true,
        FogIntensity::NormalVision => {
            unit_visibility != UnitVisibility::Stealth && attribute_visibility != UnitVisibility::Stealth
        }
        FogIntensity::Light => {
            match unit_visibility {
                UnitVisibility::Stealth => false,
                // unit gets replaced with (?) icon, flag invisible unless visibility is forced
                UnitVisibility::Normal => attribute_visibility == UnitVisibility::AlwaysVisible,
                UnitVisibility::AlwaysVisible => attribute_visibility != UnitVisibility::Stealth,
            }
        }
        FogIntensity::Dark => {
            unit_visibility == UnitVisibility::AlwaysVisible && attribute_visibility != UnitVisibility::Stealth
        }
    }
}

pub fn visible_unit_with_attribute<D: Direction>(game: &Board<D>, team: ClientPerspective, pos: Point, attribute_visibility: UnitVisibility) -> bool {
    let unit = game.get_unit(pos).unwrap();
    let fog_intensity = game.get_fog_at(team, pos);
    let unit_visibility = unit.visibility(game, pos, None);
    is_unit_attribute_visible(fog_intensity, unit_visibility, attribute_visibility)
}

pub fn visible_unit_with_attribute_transported<D: Direction>(game: &Board<D>, team: ClientPerspective, pos: Point, unload_index: usize, attribute_visibility: UnitVisibility) -> Option<usize> {
    let transporter = game.get_unit(pos).unwrap();
    let fog_intensity = game.get_fog_at(team, pos);
    let transporter_visibility = transporter.visibility(game, pos, None);
    let transport_visibility = transporter.environment().unit_transport_visibility(game, &transporter, pos, &[]);
    if !is_unit_attribute_visible(fog_intensity, transporter_visibility, transport_visibility) {
        return None;
    }
    transporter.get_transported().iter()
    .take(unload_index + 1)
    .enumerate()
    .filter_map(|(i, unit)| {
        let unit_visibility = unit.visibility(game, pos, Some((transporter, i)));
        match fog_intensity {
            FogIntensity::TrueSight => (),
            FogIntensity::NormalVision |
            FogIntensity::Light => {
                if unit_visibility == UnitVisibility::Stealth {
                    return None
                }
            }
            FogIntensity::Dark => {
                if unit_visibility != UnitVisibility::AlwaysVisible {
                    return None
                }
            }
        }
        Some((i, unit_visibility))
    }).position(|(i, unit_visibility)| {
        i == unload_index && is_unit_attribute_visible(fog_intensity, unit_visibility, attribute_visibility)
    })
}
