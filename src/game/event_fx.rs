use interfaces::ClientPerspective;
use rhai::{FuncRegistration, Module};
use zipper::*;
use zipper::zipper_derive::*;

use crate::config::effect_config::{EffectConfig, EffectDataType};
use crate::config::parse::FromConfig;
use crate::handle::Handle;
use crate::map::point::Point;
use crate::units::unit::Unit;
use crate::units::movement::MAX_PATH_LENGTH;
use crate::terrain::terrain::*;
use crate::tokens::token::Token;
use crate::map::direction::Direction;
use crate::units::movement::PathStep;
use crate::config::environment::Environment;
use crate::units::UnitVisibility;

use super::fog::FogIntensity;
use super::game::Game;
use super::game_view::GameView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectType(pub usize);

impl EffectType {
    const GLITCH: Self = Self(0);
    const FOG_SURPRISE: Self = Self(1);
}

impl FromConfig for EffectType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.effects.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::MissingUnit(base.to_string()))
        }
    }
}

impl SupportedZippable<&Environment> for EffectType {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        let bits = bits_needed_for_max_value(environment.config.effect_count() as u32 - 1);
        zipper.write_u32(self.0 as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(environment.config.effect_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index >= environment.config.effect_count() {
            return Err(ZipperError::EnumOutOfBounds(format!("EffectType index {}", index)))
        }
        Ok(Self(index))
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 1, support_ref = Environment)]
pub enum EffectStep<D: Direction> {
    Simple(Point, PathStep<D>),
    Replace(Point, PathStep<D>, Option<EffectWithoutPosition<D>>),
}

impl<D: Direction> EffectStep<D> {
    pub fn get_start(&self) -> Point {
        match self {
            Self::Simple(p, _) => *p,
            Self::Replace(p, _, _) => *p,
        }
    }

    pub fn get_step(&self) -> PathStep<D> {
        match self {
            Self::Simple(_, step) => *step,
            Self::Replace(_, step, _) => *step,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EffectData<D: Direction> {
    None,
    Int(i32),
    Direction(D),
    Terrain(Terrain<D>),
    Token(Token<D>),
    Unit(Unit<D>),
    Visibility(UnitVisibility),
}

impl<D: Direction> EffectData<D> {
    pub fn fog_replacement(&self, game: &impl GameView<D>, p: Point, fog_intensity: FogIntensity) -> Option<Self> {
        match self {
            EffectData::Terrain(inner) => Some(Self::Terrain(inner.fog_replacement(fog_intensity))),
            EffectData::Token(inner) => Some(Self::Token(inner.fog_replacement(fog_intensity)?)),
            EffectData::Unit(inner) => Some(Self::Unit(inner.fog_replacement(game, p, fog_intensity)?)),
            EffectData::Visibility(inner) => inner.visible_in_fog(fog_intensity).then_some(self.clone()),
            _ => None,
        }
    }

    fn export(&self, zipper: &mut Zipper, environment: &Environment, typ: EffectType) {
        match self {
            Self::None => (),
            Self::Int(value) => {
                let Some(EffectDataType::Int { min, max }) = environment.config.effect_data(typ) else {
                    panic!("EffectData::Int has wrong data type for {}", environment.config.effect_name(typ))
                };
                let bits = bits_needed_for_max_value((max - min) as u32);
                zipper.write_u32((*value - min) as u32, bits);
            }
            Self::Direction(inner) => inner.export(zipper, environment),
            Self::Terrain(inner) => inner.export(zipper, environment),
            Self::Token(inner) => inner.export(zipper, environment),
            Self::Unit(inner) => inner.export(zipper, environment),
            Self::Visibility(inner) => inner.zip(zipper),
        }
    }

    fn import(unzipper: &mut Unzipper, environment: &Environment, typ: EffectType) -> Result<Self, ZipperError> {
        let Some(data_type) = environment.config.effect_data(typ) else {
            return Ok(Self::None)
        };
        Ok(match data_type {
            EffectDataType::Int { min, max } => {
                let bits = bits_needed_for_max_value((max - min) as u32);
                Self::Int((unzipper.read_u32(bits)? as i32 + min).min(max))
            }
            EffectDataType::Direction => Self::Direction(D::unzip(unzipper)?),
            EffectDataType::Terrain => Self::Terrain(Terrain::import(unzipper, environment)?),
            EffectDataType::Token => Self::Token(Token::import(unzipper, environment)?),
            EffectDataType::Unit => Self::Unit(Unit::import(unzipper, environment)?),
            EffectDataType::Visibility => Self::Visibility(UnitVisibility::unzip(unzipper)?),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectWithoutPosition<D: Direction> {
    pub typ: EffectType,
    pub data: EffectData<D>,
}

impl<D: Direction> EffectWithoutPosition<D> {
    fn new(typ: usize, data: EffectData<D>) -> Self {
        Self {
            typ: EffectType(typ),
            data,
        }
    }
}

impl<D: Direction> SupportedZippable<&Environment> for EffectWithoutPosition<D> {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        self.typ.export(zipper, environment);
        self.data.export(zipper, environment, self.typ);
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let typ = EffectType::import(unzipper, environment)?;
        let data = EffectData::import(unzipper, environment, typ)?;
        Ok(Self {
            typ,
            data,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Zippable)]
#[zippable(bits = 2, support_ref = Environment)]
pub enum Effect<D: Direction> {
    Global(EffectWithoutPosition<D>),
    Point(EffectWithoutPosition<D>, Point),
    Path(Option<EffectWithoutPosition<D>>, LVec<EffectStep<D>, {MAX_PATH_LENGTH}>),
}

impl<D: Direction> Effect<D> {
    pub fn new_glitch() -> Self {
        Self::Global(EffectWithoutPosition {
            typ: EffectType::GLITCH,
            data: EffectData::None,
        })
    }

    pub fn new_fog_surprise(p: Point) -> Self {
        Self::Point(EffectWithoutPosition {
            typ: EffectType::FOG_SURPRISE,
            data: EffectData::None,
        }, p)
    }

    pub fn fog_replacement(&self, game: &Handle<Game<D>>, team: ClientPerspective) -> Option<Self> {
        let typ = match self {
            Self::Global(eff) => eff.typ,
            Self::Point(eff, _) => eff.typ,
            Self::Path(eff, _) => eff.as_ref().expect("fog_replacement should only be called on fully visible effect").typ,
        };
        let visibility = game.environment().config.effect_visibility(typ);
        match self {
            Self::Global(eff) => {
                let eff = visibility.fog_replacement(&eff, None, None, game, team)?;
                Some(Self::Global(
                    EffectWithoutPosition {
                        typ: eff.typ,
                        data: eff.data,
                    },
                ))
            }
            Self::Point(eff, p) => {
                let eff = visibility.fog_replacement(&eff, Some(*p), Some(*p), game, team)?;
                Some(Self::Point(
                    EffectWithoutPosition {
                        typ: eff.typ,
                        data: eff.data,
                    },
                    *p,
                ))
            }
            Self::Path(eff, steps) => {
                let eff = eff.as_ref().unwrap();
                let start = steps.first()?.get_start();
                let end = steps.last()?;
                let end = end.get_step().progress(game, end.get_start()).ok()?.0;
                let mut transformed = Vec::with_capacity(steps.len() + 1);
                for step in steps {
                    transformed.push(visibility.fog_replacement(eff, Some(start), Some(step.get_start()), game, team))
                }
                transformed.push(visibility.fog_replacement(eff, Some(start), Some(end), game, team));
                let steps: Vec<EffectStep<D>> = steps.iter().enumerate()
                .filter(|(i, _)| transformed[*i].is_some() || transformed[*i + 1].is_some())
                .map(|(i, step)| {
                    if transformed[i] == transformed[i + 1] {
                        EffectStep::Simple(step.get_start(), step.get_step())
                    } else {
                        EffectStep::Replace(step.get_start(), step.get_step(), transformed[i + 1].clone())
                    }
                }).collect();
                if steps.len() == 0 {
                    return None;
                }
                Some(Self::Path(
                    transformed.swap_remove(0),
                    steps.try_into().unwrap(),
                ))
            }
        }
    }
}

pub(crate) fn effect_constructor_module<D: Direction>(definitions: &[EffectConfig]) -> rhai::Shared<Module> {
    let mut module = Module::new();
    for (i, conf) in definitions.iter().enumerate() {
        let f = FuncRegistration::new(format!("FX_{}", conf.name))
        .in_global_namespace();
        match conf.data_type {
            None => f.set_into_module(&mut module, move || {
                EffectWithoutPosition::new(i, EffectData::<D>::None)
            }),
            Some(EffectDataType::Direction) => f.set_into_module(&mut module, move |value: D| {
                EffectWithoutPosition::new(i, EffectData::Direction(value))
            }),
            Some(EffectDataType::Int { min, max }) => f.set_into_module(&mut module, move |value: i32| {
                EffectWithoutPosition::new(i, EffectData::<D>::Int(value.max(min).min(max)))
            }),
            Some(EffectDataType::Terrain) => f.set_into_module(&mut module, move |value: Terrain<D>| {
                EffectWithoutPosition::new(i, EffectData::Terrain(value))
            }),
            Some(EffectDataType::Token) => f.set_into_module(&mut module, move |value: Token<D>| {
                EffectWithoutPosition::new(i, EffectData::Token(value))
            }),
            Some(EffectDataType::Unit) => f.set_into_module(&mut module, move |value: Unit<D>| {
                EffectWithoutPosition::new(i, EffectData::Unit(value))
            }),
            Some(EffectDataType::Visibility) => f.set_into_module(&mut module, move |value: UnitVisibility| {
                EffectWithoutPosition::new(i, EffectData::<D>::Visibility(value))
            }),
        };
    }
    module.into()
}
