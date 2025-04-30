use std::{collections::HashMap, error::Error, fmt::Display};

use lazy_static::lazy_static;
use regex::Regex;

#[derive(Debug, Clone)]
pub enum ShaderValue {
    Uint(u32),
    Int(i32),
    Float(f32),
    Bool(bool),
    /// Just define. Has no value. Treat as true
    Def,
}

#[derive(Debug, Clone, Copy)]
pub enum IfOps {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

/// `Preprocessor` is responsible for handling shader preprocessing tasks.
///
/// It supports:
/// - `#include` directives for modular shader code.
/// - `#define` directives for constants and macros.
/// - `#ifdef`, `#ifndef`, `#else`, `#endif` for conditional compilation.
/// - `#if` for conditional compilation with comparison.
/// - Define replacement. (using `${}` or `#{}`)
/// - Cycle detection for includes.
#[derive(Debug, Default, Clone)]
pub struct Preprocessor {
    include_targets: HashMap<String, String>,
}

lazy_static! {
    pub static ref IS_COMMENT: Regex = Regex::new(r"^[[:blank:]]*\/\/.*").unwrap();
    pub static ref IS_INCLUDE: Regex = Regex::new(
        r"^[[:blank:]]*#include[[:blank:]]+(?<module_name>[a-zA-Z0-9_\/\.\:]+)",
    ).unwrap();
    pub static ref IS_DEFINE: Regex = Regex::new(r"^[[:blank:]]*#define[[:blank:]]+(?<key>[[:word:]]+)([[:blank:]]+(?<value>[[:word:]]+))?").unwrap();
    pub static ref IS_IFDEF: Regex = Regex::new(r"^[[:blank:]]*#ifdef[[:blank:]]+(?<key>[[:word:]]+)").unwrap();
    pub static ref IS_IFNDEF: Regex = Regex::new(r"^[[:blank:]]*#ifndef[[:blank:]]+(?<key>[[:word:]]+)").unwrap();
    pub static ref IS_ELSE: Regex = Regex::new(r"^[[:blank:]]*#else").unwrap();
    pub static ref IS_ENDIF: Regex = Regex::new(r"^[[:blank:]]*#endif").unwrap();
    pub static ref IS_IF: Regex = Regex::new(r"^[[:blank:]]*#if[[:blank:]]+(?<lhs>[[:word:]]+)([[:blank:]]*(?<ops>[!=><]+){1,2}[[:blank:]]*(?<rhs>[[:word:]]+))?").unwrap();
    pub static ref REPLACE_DEFINE: Regex = Regex::new(r"(?<define>[#$]\{(?<key>[[:word:]]+)\})").unwrap();
}

#[derive(Debug)]
struct ProcessState<'caller, 'processor> {
    // Inputs
    defs: &'caller HashMap<String, ShaderValue>,
    include_targets: &'caller HashMap<String, String>,

    // Mutable state
    runtime_defs: &'processor mut HashMap<String, ShaderValue>,
    scope_map: &'processor mut HashMap<u32, bool>,
    current_scope: &'processor mut u32,
    include_stack: &'processor mut Vec<String>,
    line_number: &'processor mut usize,
}

impl Preprocessor {
    pub fn add_include_module(&mut self, module_name: &str, source: &str) {
        self.include_targets
            .insert(module_name.to_owned(), source.to_owned());
    }

    pub fn build<'a>(
        &self,
        shader_code: &str,
        defs: &HashMap<String, ShaderValue>,
        label: Option<&'a str>,
    ) -> Result<wgpu::ShaderModuleDescriptor<'a>, PreprocessError> {
        let mut include_stack = Vec::new();
        let mut scope_map: HashMap<u32, bool> = HashMap::from([(0, true)]);
        let mut current_scope = 0u32;
        let mut runtime_defs: HashMap<String, ShaderValue> = HashMap::default();
        let mut line_number: usize = 0;

        let mut state = ProcessState {
            defs,
            include_targets: &self.include_targets,
            runtime_defs: &mut runtime_defs,
            scope_map: &mut scope_map,
            current_scope: &mut current_scope,
            include_stack: &mut include_stack,
            line_number: &mut line_number,
        };
        let final_code = Self::process_chunks(shader_code.lines(), &mut state)?;

        if *state.current_scope != 0 {
            return Err(PreprocessError::UnterminatedScope {
                final_scope_level: *state.current_scope,
            });
        }

        log::trace!("Defs = {:?}", defs);
        log::trace!("Runtime defs = {:?}", runtime_defs);

        Ok(wgpu::ShaderModuleDescriptor {
            label,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(final_code)),
        })
    }

    fn process_chunks<'a, 'b, 'c, I>(
        lines: I,
        state: &mut ProcessState<'a, 'b>,
    ) -> Result<String, PreprocessError>
    where
        I: Iterator<Item = &'c str>,
    {
        let mut output = String::new();

        for raw_line in lines {
            *state.line_number += 1;

            let replaced_line = Self::replace_defines(raw_line, state.defs, state.runtime_defs)?;
            let line_str: &str = &replaced_line;

            // Check parent scope is writable
            let parent_scope_allow_write = if *state.current_scope > 0 {
                *state
                    .scope_map
                    .get(&(*state.current_scope - 1))
                    .unwrap_or(&false)
            } else {
                true // Scope 0 always has a writable parent context
            };

            let can_write_contents = parent_scope_allow_write
                && *state.scope_map.get(state.current_scope).unwrap_or(&false);

            // Handle Directives
            if let Some(cap) = IS_DEFINE.captures(line_str) {
                // #define only takes effect if the scope it's in is active
                if can_write_contents {
                    let key = cap
                        .name("key")
                        .ok_or_else(|| PreprocessError::DirectiveSyntaxError {
                            directive: "define",
                            line: line_str.to_owned(),
                            line_number: *state.line_number,
                        })?
                        .as_str();
                    let value = if let Some(value) = cap.name("value") {
                        Self::parse_shader_value(value.as_str())?
                    } else {
                        ShaderValue::Def
                    };
                    state.runtime_defs.insert(key.to_owned(), value);
                }
            } else if let Some(cap) = IS_IFDEF.captures(line_str) {
                let key = cap
                    .name("key")
                    .ok_or_else(|| PreprocessError::DirectiveSyntaxError {
                        directive: "#ifdef",
                        line: line_str.to_owned(),
                        line_number: *state.line_number,
                    })?
                    .as_str();
                *state.current_scope += 1;
                let defined = state.defs.contains_key(key) || state.runtime_defs.contains_key(key);
                state
                    .scope_map
                    .insert(*state.current_scope, parent_scope_allow_write && defined);
            } else if let Some(cap) = IS_IFNDEF.captures(line_str) {
                let key = cap
                    .name("key")
                    .ok_or_else(|| PreprocessError::DirectiveSyntaxError {
                        directive: "#ifndef",
                        line: line_str.to_owned(),
                        line_number: *state.line_number,
                    })?
                    .as_str();
                *state.current_scope += 1;
                let defined = state.defs.contains_key(key) || state.runtime_defs.contains_key(key);
                state
                    .scope_map
                    .insert(*state.current_scope, parent_scope_allow_write && !defined);
            } else if let Some(cap) = IS_IF.captures(line_str) {
                let lhs_str = cap
                    .name("lhs")
                    .ok_or_else(|| PreprocessError::DirectiveSyntaxError {
                        directive: "#if",
                        line: line_str.to_owned(),
                        line_number: *state.line_number,
                    })?
                    .as_str();
                let lhs_value = state
                    .defs
                    .get(lhs_str)
                    .or_else(|| state.runtime_defs.get(lhs_str))
                    .cloned()
                    .map(Ok)
                    .unwrap_or_else(|| Self::parse_shader_value(lhs_str))?;

                let condition_result = if let Some(ops_match) = cap.name("ops") {
                    let ops = Self::parse_ops(ops_match.as_str())?;
                    let rhs_str = cap
                        .name("rhs")
                        .ok_or_else(|| PreprocessError::DirectiveSyntaxError {
                            directive: "#if",
                            line: line_str.to_owned(),
                            line_number: *state.line_number,
                        })?
                        .as_str();
                    // Resolve RHS similarly to LHS
                    let rhs_value = state
                        .defs
                        .get(rhs_str)
                        .or_else(|| state.runtime_defs.get(rhs_str))
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| Self::parse_shader_value(rhs_str))?;

                    ops.check(lhs_value, rhs_value)?
                } else {
                    IfOps::Ne.check(lhs_value, ShaderValue::Uint(0))?
                };

                *state.current_scope += 1;
                state.scope_map.insert(
                    *state.current_scope,
                    parent_scope_allow_write && condition_result,
                );
            } else if IS_ELSE.is_match(line_str) {
                if *state.current_scope == 0 {
                    return Err(PreprocessError::InvalidScope {
                        line: line_str.to_owned(),
                        line_number: *state.line_number,
                    });
                }
                let current_state = state.scope_map.get(state.current_scope).unwrap_or(&false);
                state.scope_map.insert(
                    *state.current_scope,
                    parent_scope_allow_write && !current_state,
                );
            } else if IS_ENDIF.captures(line_str).is_some() {
                if *state.current_scope == 0 {
                    return Err(PreprocessError::InvalidScope {
                        line: line_str.to_owned(),
                        line_number: *state.line_number,
                    });
                }
                state.scope_map.remove(state.current_scope);
                *state.current_scope -= 1;
            } else if let Some(cap) = IS_INCLUDE.captures(line_str) {
                // Include Handling
                if can_write_contents {
                    let module_name = cap
                        .name("module_name")
                        .ok_or_else(|| PreprocessError::DirectiveSyntaxError {
                            directive: "#include",
                            line: line_str.to_owned(),
                            line_number: *state.line_number,
                        })?
                        .as_str()
                        .to_owned();
                    if state.include_stack.contains(&module_name.to_owned()) {
                        return Err(PreprocessError::IncludeCycleDetected {
                            module_name,
                            stack: state.include_stack.clone(),
                        });
                    }
                    state.include_stack.push(module_name.clone());
                    let included_source =
                        state.include_targets.get(&module_name).ok_or_else(|| {
                            PreprocessError::IncludeTargetNotDefined {
                                module_name: module_name.to_owned(),
                            }
                        })?;
                    let included_content = Self::process_chunks(included_source.lines(), state)?;
                    output.push_str(&included_content);
                    state.include_stack.pop();
                }
            } else if IS_COMMENT.captures(line_str).is_some() {
                // Remove all comment lines from source
            } else if can_write_contents {
                output.push_str(line_str);
                output.push('\n');
            }
        }

        Ok(output)
    }

    fn replace_defines(
        line: &str,
        defs: &HashMap<String, ShaderValue>,
        runtime_defs: &HashMap<String, ShaderValue>,
    ) -> Result<String, PreprocessError> {
        let mut output = String::new();
        let mut offset = 0usize;

        for cap in REPLACE_DEFINE.captures_iter(line) {
            let key_match = cap
                .name("key")
                .ok_or(PreprocessError::DefineKeyNotDefined {
                    line: line.to_owned(),
                    defs: defs.clone(),
                    runtime_defs: runtime_defs.clone(),
                })?;
            let key = key_match.as_str();
            let value = defs.get(key).or(runtime_defs.get(key)).ok_or(
                PreprocessError::DefineValueNotFound {
                    key: key.to_owned(),
                    defs: defs.clone(),
                    runtime_defs: runtime_defs.clone(),
                },
            )?;
            let value_str = value.to_string();
            let define_match = cap
                .name("define")
                .ok_or(PreprocessError::DefineKeyNotDefined {
                    line: line.to_owned(),
                    defs: defs.clone(),
                    runtime_defs: runtime_defs.clone(),
                })?;
            output.push_str(&line[offset..define_match.start()]);
            output.push_str(&value_str);
            offset = define_match.end();
        }
        output.push_str(&line[offset..]);

        Ok(output)
    }

    fn parse_shader_value(value: &str) -> Result<ShaderValue, PreprocessError> {
        if let Ok(v) = value.parse::<u32>() {
            Ok(ShaderValue::Uint(v))
        } else if let Ok(v) = value.parse::<i32>() {
            Ok(ShaderValue::Int(v))
        } else if let Ok(v) = value.parse::<f32>() {
            Ok(ShaderValue::Float(v))
        } else if value.to_lowercase() == "true" {
            Ok(ShaderValue::Bool(true))
        } else if value.to_lowercase() == "false" {
            Ok(ShaderValue::Bool(false))
        } else {
            Err(PreprocessError::InvalidDefineValue {
                value: value.to_owned(),
            })
        }
    }

    fn parse_ops(ops: &str) -> Result<IfOps, PreprocessError> {
        match ops {
            "==" => Ok(IfOps::Eq),
            "!=" => Ok(IfOps::Ne),
            ">" => Ok(IfOps::Gt),
            ">=" => Ok(IfOps::Ge),
            "<" => Ok(IfOps::Lt),
            "<=" => Ok(IfOps::Le),
            _ => Err(PreprocessError::IfInvalidOperation {
                ops: ops.to_owned(),
            }),
        }
    }
}

#[derive(Debug)]
pub enum PreprocessError {
    DirectiveSyntaxError {
        directive: &'static str,
        line: String,
        line_number: usize,
    },
    InvalidScope {
        line: String,
        line_number: usize,
    },
    IncludeCycleDetected {
        module_name: String,
        stack: Vec<String>,
    },
    IncludeTargetNotDefined {
        module_name: String,
    },
    DefineKeyNotDefined {
        line: String,
        defs: HashMap<String, ShaderValue>,
        runtime_defs: HashMap<String, ShaderValue>,
    },
    DefineValueNotFound {
        key: String,
        defs: HashMap<String, ShaderValue>,
        runtime_defs: HashMap<String, ShaderValue>,
    },
    InvalidDefineValue {
        value: String,
    },
    IfInvalidOperation {
        ops: String,
    },
    UnsupportedIfOperation {
        lhs: ShaderValue,
        ops: IfOps,
        rhs: ShaderValue,
    },
    UnterminatedScope {
        final_scope_level: u32,
    },
}

impl Display for ShaderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match *self {
            ShaderValue::Uint(v) => v.to_string(),
            ShaderValue::Int(v) => v.to_string(),
            ShaderValue::Float(v) => v.to_string(),
            ShaderValue::Bool(v) => v.to_string(),
            ShaderValue::Def => "true".to_owned(),
        };
        f.write_str(&str)
    }
}

impl IfOps {
    fn check(&self, lhs: ShaderValue, rhs: ShaderValue) -> Result<bool, PreprocessError> {
        // Preprocess bool to uint first
        let lhs_non_bool = match lhs {
            ShaderValue::Bool(l) => ShaderValue::Uint(if l { 1 } else { 0 }),
            _ => lhs,
        };
        let rhs_non_bool = match rhs {
            ShaderValue::Bool(l) => ShaderValue::Uint(if l { 1 } else { 0 }),
            _ => rhs,
        };

        let casted_rhs = match lhs_non_bool {
            ShaderValue::Uint(_) => match rhs_non_bool {
                ShaderValue::Uint(_) => rhs_non_bool,
                ShaderValue::Int(r) => ShaderValue::Uint(r as u32),
                ShaderValue::Float(r) => ShaderValue::Uint(r as u32),
                _ => {
                    return Err(PreprocessError::UnsupportedIfOperation {
                        lhs: lhs_non_bool,
                        ops: *self,
                        rhs: rhs_non_bool,
                    })
                }
            },
            ShaderValue::Int(_) => match rhs_non_bool {
                ShaderValue::Uint(r) => ShaderValue::Int(r as i32),
                ShaderValue::Int(_) => rhs_non_bool,
                ShaderValue::Float(r) => ShaderValue::Int(r as i32),
                _ => {
                    return Err(PreprocessError::UnsupportedIfOperation {
                        lhs: lhs_non_bool,
                        ops: *self,
                        rhs: rhs_non_bool,
                    })
                }
            },
            ShaderValue::Float(_) => match rhs_non_bool {
                ShaderValue::Uint(r) => ShaderValue::Float(r as f32),
                ShaderValue::Int(r) => ShaderValue::Float(r as f32),
                ShaderValue::Float(_) => rhs_non_bool,
                _ => {
                    return Err(PreprocessError::UnsupportedIfOperation {
                        lhs: lhs_non_bool,
                        ops: *self,
                        rhs: rhs_non_bool,
                    })
                }
            },
            _ => {
                return Err(PreprocessError::UnsupportedIfOperation {
                    lhs: lhs_non_bool,
                    ops: *self,
                    rhs: rhs_non_bool,
                })
            }
        };

        // lhs and rhs has same type
        Ok(match (self, lhs_non_bool, casted_rhs) {
            (IfOps::Eq, ShaderValue::Uint(l), ShaderValue::Uint(r)) => l == r,
            (IfOps::Eq, ShaderValue::Int(l), ShaderValue::Int(r)) => l == r,
            (IfOps::Eq, ShaderValue::Float(l), ShaderValue::Float(r)) => l == r,
            (IfOps::Ne, ShaderValue::Uint(l), ShaderValue::Uint(r)) => l != r,
            (IfOps::Ne, ShaderValue::Int(l), ShaderValue::Int(r)) => l != r,
            (IfOps::Ne, ShaderValue::Float(l), ShaderValue::Float(r)) => l != r,
            (IfOps::Gt, ShaderValue::Uint(l), ShaderValue::Uint(r)) => l > r,
            (IfOps::Gt, ShaderValue::Int(l), ShaderValue::Int(r)) => l > r,
            (IfOps::Gt, ShaderValue::Float(l), ShaderValue::Float(r)) => l > r,
            (IfOps::Ge, ShaderValue::Uint(l), ShaderValue::Uint(r)) => l >= r,
            (IfOps::Ge, ShaderValue::Int(l), ShaderValue::Int(r)) => l >= r,
            (IfOps::Ge, ShaderValue::Float(l), ShaderValue::Float(r)) => l >= r,
            (IfOps::Lt, ShaderValue::Uint(l), ShaderValue::Uint(r)) => l < r,
            (IfOps::Lt, ShaderValue::Int(l), ShaderValue::Int(r)) => l < r,
            (IfOps::Lt, ShaderValue::Float(l), ShaderValue::Float(r)) => l < r,
            (IfOps::Le, ShaderValue::Uint(l), ShaderValue::Uint(r)) => l <= r,
            (IfOps::Le, ShaderValue::Int(l), ShaderValue::Int(r)) => l <= r,
            (IfOps::Le, ShaderValue::Float(l), ShaderValue::Float(r)) => l <= r,
            _ => false,
        })
    }
}

impl Error for PreprocessError {}

impl Display for PreprocessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
           PreprocessError::DirectiveSyntaxError { directive, line, line_number } => write!(f, "Syntax error in {} directive: {}:{}", directive, line, line_number),
           PreprocessError::UnterminatedScope { final_scope_level } => write!(f, "Unterminated conditional scope; ended at level {}", final_scope_level),
           PreprocessError::IncludeCycleDetected { module_name, stack } => write!(f, "Include cycle detected for '{}'. Stack: {:?}", module_name, stack),
           PreprocessError::IncludeTargetNotDefined { module_name } => write!(f, "Include target '{}' not defined", module_name),
           PreprocessError::DefineKeyNotDefined { line, .. } => write!(f, "Define key not found in line: {}", line),
           PreprocessError::DefineValueNotFound { key, .. } => write!(f, "Define value not found for key '{}'", key),
           PreprocessError::InvalidDefineValue { value } => write!(f, "Invalid define value: {}", value),
           PreprocessError::IfInvalidOperation { ops } => write!(f, "#if invalid operation: {}", ops),
           PreprocessError::InvalidScope { line, line_number } => write!(f, "Invalid scope operation (#else/#endif without matching #if/#ifdef/#ifndef) near line: {}:{}", line, line_number),
           PreprocessError::UnsupportedIfOperation { lhs, ops, rhs } => write!(f, "Unsupported #if operation between {:?} {:?} {:?}", lhs, ops, rhs),

       }
    }
}
