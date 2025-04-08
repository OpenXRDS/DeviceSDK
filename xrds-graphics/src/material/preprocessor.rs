use std::{collections::HashMap, error::Error, fmt::Display};

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
#[derive(Debug, Clone)]
pub struct Preprocessor {
    include_targets: HashMap<String, String>,
    is_comment: Regex,
    is_include: Regex,
    is_define: Regex,
    is_ifdef: Regex,
    is_ifndef: Regex,
    is_else: Regex,
    is_endif: Regex,
    is_if: Regex,
    replace_define: Regex,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self {
            include_targets: HashMap::default(),
            is_comment: Regex::new(r"^[[:blank:]]*\/\/.*").unwrap(),
            is_include: Regex::new(
                r"^[[:blank:]]*#include[[:blank:]]+(?<module_name>[a-zA-Z0-9_\/\.\:]+)",
            ).unwrap(),
            is_define: Regex::new(r"^[[:blank:]]*#define[[:blank:]]+(?<key>[[:word:]]+)([[:blank:]]+(?<value>[[:word:]]+))?").unwrap(),
            is_ifdef: Regex::new(r"^[[:blank:]]*#ifdef[[:blank:]]+(?<key>[[:word:]]+)").unwrap(),
            is_ifndef: Regex::new(r"^[[:blank:]]*#ifndef[[:blank:]]+(?<key>[[:word:]]+)").unwrap(),
            is_else: Regex::new(r"^[[:blank:]]*#else").unwrap(),
            is_endif: Regex::new(r"^[[:blank:]]*#endif").unwrap(),
            is_if: Regex::new(r"^[[:blank:]]*#if[[:blank:]]+(?<lhs>[[:word:]]+)([[:blank:]]*(?<ops>[!=><]+){1,2}[[:blank:]]*(?<rhs>[[:word:]]+))?").unwrap(),
            replace_define: Regex::new(r"(?<define>[#$]\{(?<key>[[:word:]]+)\})").unwrap(),
        }
    }
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

        // Merge all includes
        let included = self.process_include(shader_code, &mut include_stack)?;

        let mut lines = included.lines();
        let mut output = String::new();
        let mut scope_map: HashMap<u32, bool> = HashMap::default();
        let mut current_scope = 0u32;
        scope_map.insert(0, true); // always write scope level 0

        // Interpret shader codes
        let mut runtime_defs: HashMap<String, ShaderValue> = HashMap::default();
        while let Some(raw_line) = lines.next() {
            let replaced_line = self.replace_defines(raw_line, defs, &runtime_defs)?;
            let line_str: &str = &replaced_line;

            // Check parent scope is writable
            let parent_scope_write = if current_scope > 0 {
                let parent_scope = current_scope - 1;
                *scope_map.get(&parent_scope).unwrap()
            } else {
                // Scope 0 not has parent. Set as true
                true
            };
            let current_scope_write = *scope_map.get(&current_scope).unwrap(); // must be exists

            if let Some(cap) = self.is_define.captures(line_str) {
                // #define {KEY}
                let key = cap
                    .name("key")
                    .ok_or(PreprocessError::DefineKeyNotDefined {
                        line: line_str.to_owned(),
                        defs: defs.clone(),
                        runtime_defs: runtime_defs.clone(),
                    })?
                    .as_str();
                if let Some(value) = cap.name("value") {
                    runtime_defs.insert(key.to_owned(), Self::parse_shader_value(value.as_str())?);
                } else {
                    runtime_defs.insert(key.to_owned(), ShaderValue::Def);
                }
            } else if let Some(cap) = self.is_ifdef.captures(line_str) {
                // #ifdef {KEY}
                let key = cap
                    .name("key")
                    .ok_or(PreprocessError::IfdefKeyNotDefined {
                        line: line_str.to_owned(),
                        defs: defs.clone(),
                        runtime_defs: runtime_defs.clone(),
                    })?
                    .as_str();
                current_scope += 1;
                scope_map.insert(
                    current_scope,
                    current_scope_write
                        && (defs.contains_key(key) || runtime_defs.contains_key(key)),
                );
            } else if let Some(cap) = self.is_ifndef.captures(line_str) {
                // #ifndef {KEY}
                let key = cap
                    .name("key")
                    .ok_or(PreprocessError::IfndefKeyNotDefined {
                        line: line_str.to_owned(),
                        defs: defs.clone(),
                        runtime_defs: runtime_defs.clone(),
                    })?
                    .as_str();
                current_scope += 1;
                scope_map.insert(
                    current_scope,
                    current_scope_write
                        && !(defs.contains_key(key) || runtime_defs.contains_key(key)),
                );
            } else if let Some(cap) = self.is_if.captures(line_str) {
                let lhs = cap
                    .name("lhs")
                    .ok_or(PreprocessError::IfLhsNotDefined {
                        line: line_str.to_owned(),
                    })?
                    .as_str();
                let lhs_value = Self::parse_shader_value(lhs)?;
                let (ops, rhs_value) = if let Some(ops_match) = cap.name("ops") {
                    let ops = Self::parse_ops(ops_match.as_str())?;
                    let rhs = cap.name("rhs").ok_or(PreprocessError::IfRhsNotDefined {
                        line: line_str.to_owned(),
                    })?;
                    let rhs_value = Self::parse_shader_value(rhs.as_str())?;
                    (ops, rhs_value)
                } else {
                    // check lhs != 0
                    (IfOps::Ne, ShaderValue::Uint(0))
                };
                current_scope += 1;
                scope_map.insert(
                    current_scope,
                    current_scope_write && ops.check(lhs_value, rhs_value)?,
                );
            } else if let Some(_) = self.is_endif.captures(line_str) {
                if current_scope == 0 {
                    return Err(PreprocessError::InvalidScope {
                        line: line_str.to_owned(),
                    });
                }
                scope_map.remove(&current_scope);
                current_scope -= 1;
            } else if let Some(_) = self.is_else.captures(line_str) {
                scope_map.insert(current_scope, !current_scope_write && parent_scope_write);
            } else if let Some(_) = self.is_comment.captures(line_str) {
                // Remove all comment lines from source
            } else if let Some(write) = scope_map.get(&current_scope) {
                if *write {
                    output.push_str(line_str);
                    output.push_str("\n");
                }
            }
        }

        log::trace!("Final code = {}\n", output);
        log::trace!("Defs = {:?}", defs);
        log::trace!("Runtime defs = {:?}", runtime_defs);

        Ok(wgpu::ShaderModuleDescriptor {
            label: label,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(output)),
        })
    }

    fn process_include(
        &self,
        shader_code: &str,
        include_stack: &mut Vec<String>,
    ) -> Result<String, PreprocessError> {
        let mut lines = shader_code.lines();
        let mut output = String::new();

        while let Some(line) = lines.next() {
            if let Some(cap) = self.is_include.captures(line) {
                let module_name = cap
                    .name("module_name")
                    .ok_or(PreprocessError::IncludeModulenameNotExists {
                        line: line.to_owned(),
                    })?
                    .as_str()
                    .to_owned();

                if include_stack.contains(&module_name) {
                    // Cycle detected
                    return Err(PreprocessError::IncludeCycleDetected {
                        module_name,
                        stack: include_stack.clone(),
                    });
                }
                include_stack.push(module_name.clone());

                let included_source = self
                    .include_targets
                    .get(&module_name)
                    .ok_or(PreprocessError::IncludeTargetNotDefined { module_name })?;

                let included_content = self.process_include(included_source, include_stack)?;

                output.push_str(&included_content);
                output.push_str("\n");

                include_stack.pop();
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }

        Ok(output)
    }

    fn replace_defines(
        &self,
        line: &str,
        defs: &HashMap<String, ShaderValue>,
        runtime_defs: &HashMap<String, ShaderValue>,
    ) -> Result<String, PreprocessError> {
        let mut output = String::new();
        let mut offset = 0usize;

        for cap in self.replace_define.captures_iter(line) {
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
    IncludeModulenameNotExists {
        line: String,
    },
    IncludeFileNotFound(std::io::Error),
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
    IfdefKeyNotDefined {
        line: String,
        defs: HashMap<String, ShaderValue>,
        runtime_defs: HashMap<String, ShaderValue>,
    },
    IfndefKeyNotDefined {
        line: String,
        defs: HashMap<String, ShaderValue>,
        runtime_defs: HashMap<String, ShaderValue>,
    },
    IfLhsNotDefined {
        line: String,
    },
    IfRhsNotDefined {
        line: String,
    },
    IfInvalidOperation {
        ops: String,
    },
    InvalidScope {
        line: String,
    },
    UnsupportedIfOperation {
        lhs: ShaderValue,
        ops: IfOps,
        rhs: ShaderValue,
    },
}

impl ShaderValue {
    fn to_string(&self) -> String {
        match self {
            ShaderValue::Uint(v) => v.to_string(),
            ShaderValue::Int(v) => v.to_string(),
            ShaderValue::Float(v) => v.to_string(),
            ShaderValue::Bool(v) => v.to_string(),
            ShaderValue::Def => "true".to_owned(),
        }
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
        f.write_fmt(format_args!("{:?}", self))
    }
}
