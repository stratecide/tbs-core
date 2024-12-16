use std::error::Error;
use std::hash::Hash;
use std::ops::RangeInclusive;

use rhai::*;
use rustc_hash::FxHashMap as HashMap;

use crate::script::create_base_engine;

use super::parse::{FromConfig, GLOBAL_SCRIPT};
use super::ConfigParseError;

pub struct FunctionPointer {
    pub index: usize,
    pub parameter_count: usize,
}

pub struct FileLoader {
    load_file: Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>,
    engine: Engine,
    unoptimized_asts: HashMap<String, Shared<AST>>,
    rhai_functions: Vec<(String, String, usize)>,
    pub movement_types: Vec<String>,
    pub unit_types: Vec<String>,
    pub terrain_types: Vec<String>,
    pub token_types: Vec<String>,
    pub commander_types: Vec<String>,
    pub hero_types: Vec<String>,
    pub flags: Vec<String>,
    pub tags: Vec<String>,
    pub effects: Vec<String>,
}

impl FileLoader {
    pub(super) fn new(load_file: Box<dyn Fn(&str) -> Result<String, Box<dyn Error>>>) -> Self {
        let mut engine = create_base_engine();
        // preserve constants
        engine.set_optimization_level(OptimizationLevel::None);
        engine.set_strict_variables(false);
        Self {
            load_file,
            engine,
            unoptimized_asts: HashMap::default(),
            rhai_functions: Vec::new(),
            movement_types: Vec::new(),
            unit_types: Vec::new(),
            terrain_types: Vec::new(),
            token_types: Vec::new(),
            commander_types: Vec::new(),
            hero_types: Vec::new(),
            flags: Vec::new(),
            tags: Vec::new(),
            effects: Vec::new(),
        }
    }

    // TODO: delete this function
    pub(super) fn load_config(&mut self, filename: &str) -> Result<String, Box<dyn Error>> {
        (self.load_file)(filename)
        .map_err(|e| ConfigParseError::FileMissing(format!("{filename}: {e}")).into())
    }

    pub(super) fn table_key_value(&mut self, filename: &str, mut f: impl FnMut(&str, &str, &mut Self) -> Result<(), Box<dyn Error>>) -> Result<(), Box<dyn Error>> {
        let data = self.load_config(filename)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').has_headers(false).from_reader(data.as_bytes());
        for line in reader.records() {
            let line = line?;
            let mut line = line.iter();
            let Some(key) = line.next() else {
                continue;
            };
            let Some(value) = line.next() else {
                continue;
            };
            f(key.trim(), value.trim(), self)?;
        }
        Ok(())
    }

    pub(super) fn table_with_headers<
        Header: FromConfig + PartialEq + Eq + Hash + Clone,
        Line: TableLine<Header=Header>,
    >(&mut self, filename: &str, mut f: impl FnMut(Line) -> Result<(), Box<dyn Error>>) -> Result<(), Box<dyn Error>> {
        let data = self.load_config(filename)?;
        let mut reader = csv::ReaderBuilder::new().delimiter(b';').from_reader(data.as_bytes());
        let mut headers: Vec<Header> = Vec::new();
        for h in reader.headers()? {
            let header = Header::from_conf(h, self)?.0;
            if headers.contains(&header) {
                return Err(Box::new(ConfigParseError::DuplicateHeader(h.to_string())))
            }
            headers.push(header);
        }
        for line in reader.records() {
            let mut map = HashMap::default();
            let line = line?;
            for (i, s) in line.iter().enumerate().take(headers.len()) {
                map.insert(headers[i].clone(), s);
            }
            let line = Line::parse(&map, self)?;
            line.simple_validation()?;
            f(line)?;
        }
        Ok(())
    }

    pub(super) fn rhai_function(&mut self, name: &str, parameter_count: RangeInclusive<usize>) -> Result<FunctionPointer, ConfigParseError> {
        let Some((filename, name)) = name.split_once('>') else {
            return Err(ConfigParseError::ScriptNeedsFileAndFunctionName(name.to_string()));
        };
        let filename = filename.trim();
        let name = name.trim();
        if filename == GLOBAL_SCRIPT {
            return Err(ConfigParseError::DontCallGlobalScriptDirectly(name.to_string()));
        }
        if let Some(index) = self.rhai_functions.iter()
        .position(|(f, n, pc)| f.as_str() == filename && n.as_str() == name && parameter_count.contains(pc)) {
            Ok(FunctionPointer {
                index,
                parameter_count: self.rhai_functions[index].2,
            })
        } else {
            let filename = filename.to_string();
            let ast = self.load_rhai_module(&filename)?;
            // check if a function with that name and correct parameter-count exists (parameter types can't be verified)
            let Some(parameter_count) = ast.iter_functions()
            .filter(|f| f.name == name)
            .map(|f| f.params.len())
            .filter(|count| parameter_count.contains(count))
            .next() else {
                return Err(ConfigParseError::ScriptFunctionNotFound(filename, name.to_string()));
            };
            self.rhai_functions.push((filename, name.to_string(), parameter_count));
            Ok(FunctionPointer {
                index: self.rhai_functions.len() - 1,
                parameter_count,
            })
        }
    }

    pub(super) fn load_rhai_module(&mut self, filename: &String) -> Result<Shared<AST>, ConfigParseError> {
        if let Some(ast) = self.unoptimized_asts.get(filename) {
            Ok(ast.clone())
        } else {
            let path = format!("scripts/{filename}.rhai");
            let script = (self.load_file)(&path)
                .map_err(|_| ConfigParseError::FileMissing(path.clone()))?;
            let ast = self.engine.compile(script)
                .map_err(|e| ConfigParseError::ScriptCompilation(path.clone(), e.to_string()))?;
            let ast = Shared::new(ast);
            if filename != GLOBAL_SCRIPT {
                self.unoptimized_asts.insert(filename.clone(), ast.clone());
            }
            Ok(ast)
        }
    }

    pub(super) fn finish(self) -> (Vec<AST>, Vec<(usize, String)>) {
        let mut indices = HashMap::default();
        let mut asts = Vec::with_capacity(self.unoptimized_asts.len());
        for (i, (filename, ast)) in self.unoptimized_asts.into_iter().enumerate() {
            match filename.as_str() {
                GLOBAL_SCRIPT => (),
                _ => {
                    asts.push(Shared::into_inner(ast).unwrap());
                    indices.insert(filename, i);
                }
            }
        }
        let functions: Vec<(usize, String)> = self.rhai_functions.into_iter().map(|(filename, name, _)| {
            (*indices.get(&filename).unwrap(), name)
        }).collect();
        (
            asts,
            functions,
        )
    }
}

pub(super) trait TableLine: Sized {
    type Header: FromConfig;
    fn parse(data: &HashMap<Self::Header, &str>, loader: &mut FileLoader) -> Result<Self, Box<dyn Error>>;
    fn simple_validation(&self) -> Result<(), Box<dyn Error>>;
}
