use std::collections::HashMap;

use emmylua_parser::{LuaFeatures, LuaLanguageLevel, LuaVersionNumber, SpecialFunction};
use schemars::JsonSchema;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmmyrcRuntime {
    /// Lua version.
    #[serde(default)]
    pub version: EmmyrcLuaVersion,
    #[serde(default)]
    /// Functions that like require.
    pub require_like_function: Vec<String>,
    #[serde(default)]
    /// Framework versions.
    pub framework_versions: Vec<String>,
    #[serde(default)]
    /// file Extensions. eg: .lua, .lua.txt
    pub extensions: Vec<String>,
    #[serde(default)]
    /// Require pattern. eg. "?.lua", "?/init.lua"
    pub require_pattern: Vec<String>,
    /// Non-standard symbols.
    #[serde(default)]
    pub nonstandard_symbol: Vec<EmmyrcNonStdSymbol>,
    /// Special symbols.
    #[serde(default)]
    pub special: HashMap<String, EmmyrcSpecialSymbol>,
    /// Glob patterns of files treated as environment modules.
    /// For a matched file without a top-level `return`, all top-level globals
    /// are synthesized into the module export table. eg. ["Common/battle_core/**"]
    #[serde(default)]
    pub environment_module_pattern: Vec<String>,
    /// Rules for functions that define a global variable at runtime, eg. registerGlobal("NAME", value).
    #[serde(default)]
    pub global_define_rules: Vec<EmmyrcSpecialCallRule>,
    /// Rules for functions that define a class at runtime, eg. DefineClass("NAME", Super).
    #[serde(default)]
    pub class_define_rules: Vec<EmmyrcSpecialCallRule>,
    /// Declare the types of common instance fields per class, eg. `self.battle: BattleCore`.
    /// Field assignments inferred from untyped constructor params otherwise degrade to `any`.
    #[serde(default)]
    pub class_field_type_rules: Vec<EmmyrcClassFieldTypeRule>,
}

impl Default for EmmyrcRuntime {
    fn default() -> Self {
        Self {
            version: EmmyrcLuaVersion::default(),
            require_like_function: Vec::new(),
            framework_versions: Vec::new(),
            extensions: Vec::new(),
            require_pattern: Vec::new(),
            nonstandard_symbol: Vec::new(),
            special: HashMap::new(),
            environment_module_pattern: Vec::new(),
            global_define_rules: default_global_define_rules(),
            class_define_rules: default_class_define_rules(),
            class_field_type_rules: Vec::new(),
        }
    }
}

/// Declares field types for a specific (globally unique) class.
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmmyrcClassFieldTypeRule {
    /// Exact class name, eg. "BattleBt".
    pub class: String,
    /// Field type declarations for this class.
    #[serde(default)]
    pub fields: Vec<EmmyrcFieldTypeRule>,
}

/// Declares the type of a single instance field.
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmmyrcFieldTypeRule {
    /// Exact field name, eg. "battle".
    pub name: String,
    /// Type name, resolved to a class reference, eg. "BattleCore".
    #[serde(rename = "type")]
    pub r#type: String,
    /// Whether the field may be nil. Defaults to true (nullable).
    #[serde(default = "default_true")]
    pub optional: bool,
}

fn default_global_define_rules() -> Vec<EmmyrcSpecialCallRule> {
    vec![EmmyrcSpecialCallRule {
        function: "registerGlobal".to_string(),
        params: vec![
            EmmyrcParameterRule {
                role: EmmyrcParamRole::Name,
                index: 0,
            },
            EmmyrcParameterRule {
                role: EmmyrcParamRole::Value,
                index: 1,
            },
        ],
    }]
}

fn default_class_define_rules() -> Vec<EmmyrcSpecialCallRule> {
    vec![
        EmmyrcSpecialCallRule {
            function: "DefineClass".to_string(),
            params: vec![
                EmmyrcParameterRule {
                    role: EmmyrcParamRole::Name,
                    index: 0,
                },
                EmmyrcParameterRule {
                    role: EmmyrcParamRole::Super,
                    index: 1,
                },
            ],
        },
        EmmyrcSpecialCallRule {
            function: "DefineComponent".to_string(),
            params: vec![
                EmmyrcParameterRule {
                    role: EmmyrcParamRole::Name,
                    index: 0,
                },
                EmmyrcParameterRule {
                    role: EmmyrcParamRole::Super,
                    index: 1,
                },
            ],
        },
    ]
}

/// Role of a parameter within a special call rule.
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmmyrcParamRole {
    /// The declared name (must be a string literal).
    Name,
    /// The value / module argument.
    Value,
    /// A super class.
    Super,
}

/// A single parameter rule: which role a positional argument plays.
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq, Eq)]
pub struct EmmyrcParameterRule {
    /// The role of the argument.
    pub role: EmmyrcParamRole,
    /// Zero-based index of the argument.
    pub index: usize,
}

/// Rule describing a runtime framework call, eg. registerGlobal / DefineClass.
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq, Eq)]
pub struct EmmyrcSpecialCallRule {
    /// Fully qualified function name being matched.
    pub function: String,
    /// Parameter rules.
    #[serde(default)]
    pub params: Vec<EmmyrcParameterRule>,
}

impl EmmyrcSpecialCallRule {
    /// Return the index of the argument playing the given role.
    pub fn get_index(&self, role: EmmyrcParamRole) -> Option<usize> {
        self.params
            .iter()
            .find(|p| p.role == role)
            .map(|p| p.index)
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmmyrcLuaVersion {
    /// Lua 5.1
    #[serde(rename = "Lua5.1", alias = "Lua 5.1")]
    Lua51,
    /// LuaJIT
    #[serde(rename = "LuaJIT")]
    LuaJIT,
    #[serde(rename = "LuaJIT2", alias = "LuaJIT 2")]
    LuaJIT2,
    #[serde(rename = "LuaJIT3", alias = "LuaJIT 3")]
    LuaJIT3,
    /// Lua 5.2
    #[serde(rename = "Lua5.2", alias = "Lua 5.2")]
    Lua52,
    /// Lua 5.3
    #[serde(rename = "Lua5.3", alias = "Lua 5.3")]
    Lua53,
    /// Lua 5.4
    #[serde(rename = "Lua5.4", alias = "Lua 5.4")]
    Lua54,
    /// Lua 5.5
    #[serde(rename = "Lua5.5", alias = "Lua 5.5")]
    Lua55,
    /// Lua Latest
    #[serde(rename = "LuaLatest", alias = "Lua Latest")]
    #[default]
    LuaLatest,
}

impl EmmyrcLuaVersion {
    pub fn to_lua_version_number(&self) -> LuaVersionNumber {
        match self {
            EmmyrcLuaVersion::Lua51 => LuaVersionNumber::new(5, 1, 0),
            EmmyrcLuaVersion::LuaJIT => LuaVersionNumber::LUA_JIT,
            EmmyrcLuaVersion::LuaJIT2 => LuaVersionNumber::LUA_JIT,
            EmmyrcLuaVersion::LuaJIT3 => LuaVersionNumber::LUA_JIT,
            EmmyrcLuaVersion::Lua52 => LuaVersionNumber::new(5, 2, 0),
            EmmyrcLuaVersion::Lua53 => LuaVersionNumber::new(5, 3, 0),
            EmmyrcLuaVersion::Lua54 => LuaVersionNumber::new(5, 4, 0),
            EmmyrcLuaVersion::LuaLatest => LuaVersionNumber::new(5, 4, 0),
            EmmyrcLuaVersion::Lua55 => LuaVersionNumber::new(5, 5, 0),
        }
    }

    pub fn get_language_level(&self) -> LuaLanguageLevel {
        match self {
            EmmyrcLuaVersion::Lua51 => LuaLanguageLevel::Lua51,
            EmmyrcLuaVersion::LuaJIT => LuaLanguageLevel::LuaJIT,
            EmmyrcLuaVersion::LuaJIT2 => LuaLanguageLevel::LuaJIT2,
            EmmyrcLuaVersion::LuaJIT3 => LuaLanguageLevel::LuaJIT3,
            EmmyrcLuaVersion::Lua52 => LuaLanguageLevel::Lua52,
            EmmyrcLuaVersion::Lua53 => LuaLanguageLevel::Lua53,
            EmmyrcLuaVersion::Lua54 => LuaLanguageLevel::Lua54,
            EmmyrcLuaVersion::LuaLatest => LuaLanguageLevel::Lua55,
            EmmyrcLuaVersion::Lua55 => LuaLanguageLevel::Lua55,
        }
    }

    pub fn is_luajit(&self) -> bool {
        matches!(
            self,
            EmmyrcLuaVersion::LuaJIT | EmmyrcLuaVersion::LuaJIT2 | EmmyrcLuaVersion::LuaJIT3
        )
    }
}

#[allow(unused)]
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EmmyrcNonStdSymbol {
    #[serde(rename = "//")]
    DoubleSlash = 1, // "//"
    #[serde(rename = "/**/")]
    SlashStar, // "/**/"
    #[serde(rename = "`")]
    Backtick, // "`"
    #[serde(rename = "+=")]
    PlusAssign, // "+="
    #[serde(rename = "-=")]
    MinusAssign, // "-="
    #[serde(rename = "*=")]
    StarAssign, // "*="
    #[serde(rename = "/=")]
    SlashAssign, // "/="
    #[serde(rename = "%=")]
    PercentAssign, // "%="
    #[serde(rename = "^=")]
    CaretAssign, // "^="
    #[serde(rename = "//=")]
    DoubleSlashAssign, // "//="
    #[serde(rename = "|=")]
    PipeAssign, // "|="
    #[serde(rename = "&=")]
    AmpAssign, // "&="
    #[serde(rename = "<<=")]
    ShiftLeftAssign, // "<<="
    #[serde(rename = ">>=")]
    ShiftRightAssign, // ">>="
    #[serde(rename = "||")]
    DoublePipe, // "||"
    #[serde(rename = "&&")]
    DoubleAmp, // "&&"
    #[serde(rename = "!")]
    Exclamation, // "!"
    #[serde(rename = "!=")]
    NotEqual, // "!="
    #[serde(rename = "continue")]
    Continue, // "continue"
}

impl From<EmmyrcNonStdSymbol> for LuaFeatures {
    fn from(symbol: EmmyrcNonStdSymbol) -> Self {
        match symbol {
            EmmyrcNonStdSymbol::DoubleSlash => LuaFeatures::DoubleSlash,
            EmmyrcNonStdSymbol::SlashStar => LuaFeatures::SlashStar,
            EmmyrcNonStdSymbol::Backtick => LuaFeatures::StringInterpolation,
            EmmyrcNonStdSymbol::PlusAssign => LuaFeatures::PlusAssign,
            EmmyrcNonStdSymbol::MinusAssign => LuaFeatures::MinusAssign,
            EmmyrcNonStdSymbol::StarAssign => LuaFeatures::StarAssign,
            EmmyrcNonStdSymbol::SlashAssign => LuaFeatures::SlashAssign,
            EmmyrcNonStdSymbol::PercentAssign => LuaFeatures::PercentAssign,
            EmmyrcNonStdSymbol::CaretAssign => LuaFeatures::CaretAssign,
            EmmyrcNonStdSymbol::DoubleSlashAssign => LuaFeatures::DoubleSlashAssign,
            EmmyrcNonStdSymbol::PipeAssign => LuaFeatures::PipeAssign,
            EmmyrcNonStdSymbol::AmpAssign => LuaFeatures::AmpAssign,
            EmmyrcNonStdSymbol::ShiftLeftAssign => LuaFeatures::ShiftLeftAssign,
            EmmyrcNonStdSymbol::ShiftRightAssign => LuaFeatures::ShiftRightAssign,
            EmmyrcNonStdSymbol::DoublePipe => LuaFeatures::DoublePipeOr,
            EmmyrcNonStdSymbol::DoubleAmp => LuaFeatures::DoubleAmpAnd,
            EmmyrcNonStdSymbol::Exclamation => LuaFeatures::Exclamation,
            EmmyrcNonStdSymbol::NotEqual => LuaFeatures::NotEqual,
            EmmyrcNonStdSymbol::Continue => LuaFeatures::Continue,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EmmyrcSpecialSymbol {
    #[serde(rename = "none")]
    None,
    Require,
    Error,
    Assert,
    Type,
    Setmetatable,
}

impl<'de> Deserialize<'de> for EmmyrcSpecialSymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 首先尝试使用默认的 derive 实现
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "none" => Ok(EmmyrcSpecialSymbol::None),
            "require" => Ok(EmmyrcSpecialSymbol::Require),
            "error" => Ok(EmmyrcSpecialSymbol::Error),
            "assert" => Ok(EmmyrcSpecialSymbol::Assert),
            "type" => Ok(EmmyrcSpecialSymbol::Type),
            "setmetatable" => Ok(EmmyrcSpecialSymbol::Setmetatable),
            // 对于任何不匹配的值，返回 None
            _ => Ok(EmmyrcSpecialSymbol::None),
        }
    }
}

impl From<EmmyrcSpecialSymbol> for Option<SpecialFunction> {
    fn from(symbol: EmmyrcSpecialSymbol) -> Self {
        match symbol {
            EmmyrcSpecialSymbol::None => None,
            EmmyrcSpecialSymbol::Require => Some(SpecialFunction::Require),
            EmmyrcSpecialSymbol::Error => Some(SpecialFunction::Error),
            EmmyrcSpecialSymbol::Assert => Some(SpecialFunction::Assert),
            EmmyrcSpecialSymbol::Type => Some(SpecialFunction::Type),
            EmmyrcSpecialSymbol::Setmetatable => Some(SpecialFunction::Setmetaatable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emmyrc_runtime() {
        let json1 = r#"{
            "version": "Lua5.1"
        }"#;
        let runtime: EmmyrcRuntime = serde_json::from_str(json1).unwrap();
        assert_eq!(runtime.version, EmmyrcLuaVersion::Lua51);

        let json2 = r#"{
            "version": "Lua 5.1"
        }"#;

        let runtime: EmmyrcRuntime = serde_json::from_str(json2).unwrap();
        assert_eq!(runtime.version, EmmyrcLuaVersion::Lua51);
    }

    #[test]
    fn test_default_special_call_rules() {
        let runtime = EmmyrcRuntime::default();
        assert_eq!(runtime.global_define_rules.len(), 1);
        let global_rule = &runtime.global_define_rules[0];
        assert_eq!(global_rule.function, "registerGlobal");
        assert_eq!(global_rule.get_index(EmmyrcParamRole::Name), Some(0));
        assert_eq!(global_rule.get_index(EmmyrcParamRole::Value), Some(1));

        let class_rule = runtime
            .class_define_rules
            .iter()
            .find(|r| r.function == "DefineClass")
            .unwrap();
        assert_eq!(class_rule.get_index(EmmyrcParamRole::Name), Some(0));
        assert_eq!(class_rule.get_index(EmmyrcParamRole::Super), Some(1));
    }

    #[test]
    fn test_custom_special_call_rules_deserialize() {
        let json = r#"{
            "globalDefineRules": [
                {
                    "function": "myRegister",
                    "params": [
                        { "role": "value", "index": 0 },
                        { "role": "name", "index": 1 }
                    ]
                }
            ],
            "classDefineRules": [
                {
                    "function": "MyClass",
                    "params": [ { "role": "name", "index": 0 } ]
                }
            ],
            "environmentModulePattern": ["Common/battle_core/**"]
        }"#;
        let runtime: EmmyrcRuntime = serde_json::from_str(json).unwrap();
        assert_eq!(runtime.global_define_rules.len(), 1);
        let rule = &runtime.global_define_rules[0];
        assert_eq!(rule.function, "myRegister");
        assert_eq!(rule.get_index(EmmyrcParamRole::Name), Some(1));
        assert_eq!(rule.get_index(EmmyrcParamRole::Value), Some(0));
        assert_eq!(runtime.class_define_rules[0].function, "MyClass");
        assert_eq!(
            runtime.environment_module_pattern,
            vec!["Common/battle_core/**".to_string()]
        );
    }

    #[test]
    fn test_class_field_type_rules_deserialize() {
        let json = r#"{
            "classFieldTypeRules": [
                {
                    "class": "BattleBt",
                    "fields": [
                        { "name": "battle", "type": "BattleCore" },
                        { "name": "gamer", "type": "BattleGamer", "optional": false }
                    ]
                }
            ]
        }"#;
        let runtime: EmmyrcRuntime = serde_json::from_str(json).unwrap();
        assert_eq!(runtime.class_field_type_rules.len(), 1);
        let rule = &runtime.class_field_type_rules[0];
        assert_eq!(rule.class, "BattleBt");
        assert_eq!(rule.fields.len(), 2);
        assert_eq!(rule.fields[0].name, "battle");
        assert_eq!(rule.fields[0].r#type, "BattleCore");
        assert!(rule.fields[0].optional, "optional should default to true");
        assert!(!rule.fields[1].optional);
    }
}
