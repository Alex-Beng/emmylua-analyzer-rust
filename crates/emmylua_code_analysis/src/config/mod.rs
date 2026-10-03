mod config_loader;
mod configs;
mod flatten_config;
mod lua_loader;
mod pre_process;

use std::{collections::HashMap, path::Path};

pub use config_loader::{load_configs, load_configs_raw};
pub use configs::{
    DiagnosticSeveritySetting, DocSyntax, EmmyLibraryConfig, EmmyLibraryItem,
    EmmyrcClassFieldTypeRule, EmmyrcCodeAction, EmmyrcCodeLens, EmmyrcCompletion, EmmyrcDiagnostic,
    EmmyrcDoc, EmmyrcDocumentColor, EmmyrcExternalTool, EmmyrcFieldTypeRule, EmmyrcFilenameConvention,
    EmmyrcHover, EmmyrcInlayHint, EmmyrcInlineValues, EmmyrcLuaVersion, EmmyrcParamRole,
    EmmyrcParameterRule, EmmyrcReference, EmmyrcReformat, EmmyrcResource, EmmyrcRuntime,
    EmmyrcSemanticToken, EmmyrcSignature, EmmyrcSpecialCallRule, EmmyrcStrict, EmmyrcWorkspace,
    EmmyrcWorkspaceModuleMap, EmmyrcWorkspacePathConfig, EmmyrcWorkspacePathItem,
};
use emmylua_parser::{LuaFeaturesSet, LuaLanguageLevel, ParserConfig, SpecialFunction};
use rowan::NodeCache;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::pre_process::PreProcessContext;

/// Well-known file name (in the workspace root) for runtime-dumped field type
/// hints, consumed during `pre_process_emmyrc`.
pub const FIELD_TYPE_HINTS_FILE_NAME: &str = ".emmyrc-fieldtypes.json";

#[derive(Serialize, Deserialize, Debug, JsonSchema, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Emmyrc {
    #[serde(rename = "$schema")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default)]
    pub completion: EmmyrcCompletion,
    #[serde(default)]
    pub diagnostics: EmmyrcDiagnostic,
    #[serde(default)]
    pub signature: EmmyrcSignature,
    #[serde(default)]
    pub hint: EmmyrcInlayHint,
    #[serde(default)]
    pub runtime: EmmyrcRuntime,
    #[serde(default)]
    pub workspace: EmmyrcWorkspace,
    #[serde(default)]
    pub resource: EmmyrcResource,
    #[serde(default)]
    pub code_lens: EmmyrcCodeLens,
    #[serde(default)]
    pub strict: EmmyrcStrict,
    #[serde(default)]
    pub semantic_tokens: EmmyrcSemanticToken,
    #[serde(default)]
    pub references: EmmyrcReference,
    #[serde(default)]
    pub hover: EmmyrcHover,
    #[serde(default)]
    pub document_color: EmmyrcDocumentColor,
    #[serde(default)]
    pub code_action: EmmyrcCodeAction,
    #[serde(default)]
    pub inline_values: EmmyrcInlineValues,
    #[serde(default)]
    pub doc: EmmyrcDoc,
    #[serde(default)]
    pub format: EmmyrcReformat,
}

impl Emmyrc {
    pub fn get_parse_config<'cache>(
        &self,
        node_cache: &'cache mut NodeCache,
    ) -> ParserConfig<'cache> {
        let lua_language_level = self.get_language_level();
        let mut special_like = HashMap::new();
        for (name, func) in self.runtime.special.iter() {
            if let Some(func) = (*func).into() {
                special_like.insert(name.clone(), func);
            }
        }
        for name in self.runtime.require_like_function.iter() {
            special_like.insert(name.clone(), SpecialFunction::Require);
        }
        let mut non_std_symbols = LuaFeaturesSet::default();
        for symbol in self.runtime.nonstandard_symbol.iter() {
            non_std_symbols.add((*symbol).into());
        }

        ParserConfig::new(
            lua_language_level,
            Some(node_cache),
            special_like,
            non_std_symbols,
            true,
        )
    }

    pub fn get_language_level(&self) -> LuaLanguageLevel {
        self.runtime.version.get_language_level()
    }

    pub fn pre_process_emmyrc(&mut self, workspace_root: &Path) {
        let mut context = PreProcessContext::new(workspace_root.to_path_buf());

        self.workspace.workspace_roots =
            context.process_and_dedup_string(self.workspace.workspace_roots.iter());

        self.workspace.library =
            context.process_and_dedup_workspace_path_items(self.workspace.library.iter());

        self.workspace.packages =
            context.process_and_dedup_workspace_path_items(self.workspace.packages.iter());

        self.workspace.ignore_dir =
            context.process_and_dedup_string(self.workspace.ignore_dir.iter());

        self.resource.paths = context.process_and_dedup_string(self.resource.paths.iter());

        self.load_field_type_hints(workspace_root);
    }

    /// Load runtime-dumped field type hints from the well-known file
    /// `<workspace_root>/.emmyrc-fieldtypes.json` (shape: `{ class: { field: type } }`).
    ///
    /// These are merged *before* the hand-written `classFieldTypeRules`, so an
    /// explicit rule overrides a dumped one for the same field.
    fn load_field_type_hints(&mut self, workspace_root: &Path) {
        let path = workspace_root.join(FIELD_TYPE_HINTS_FILE_NAME);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };

        let parsed: HashMap<String, HashMap<String, String>> = match serde_json::from_str(&text) {
            Ok(parsed) => parsed,
            Err(err) => {
                log::warn!("failed to parse {}: {}", FIELD_TYPE_HINTS_FILE_NAME, err);
                return;
            }
        };

        let mut rules: Vec<EmmyrcClassFieldTypeRule> = parsed
            .into_iter()
            .map(|(class, fields)| EmmyrcClassFieldTypeRule {
                class,
                fields: fields
                    .into_iter()
                    .map(|(name, r#type)| EmmyrcFieldTypeRule {
                        name,
                        r#type,
                        // Runtime-dumped types are exact and usually non-nil.
                        optional: false,
                    })
                    .collect(),
            })
            .collect();
        rules.sort_by(|a, b| a.class.cmp(&b.class));

        // Dumped rules come first; hand-written rules keep precedence.
        rules.extend(std::mem::take(&mut self.runtime.class_field_type_rules));
        self.runtime.class_field_type_rules = rules;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "emmylua-fieldtype-hints-{}-{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn test_pre_process_loads_field_type_hints_file() {
        let root = temp_dir();
        fs::write(
            root.join(FIELD_TYPE_HINTS_FILE_NAME),
            r#"{ "BattleBt": { "battle": "BattleCore", "warrior": "BattleWarrior" } }"#,
        )
        .unwrap();

        let mut emmyrc = Emmyrc::default();
        // A hand-written rule for the same class/field must take precedence.
        emmyrc.runtime.class_field_type_rules = vec![EmmyrcClassFieldTypeRule {
            class: "BattleBt".to_string(),
            fields: vec![EmmyrcFieldTypeRule {
                name: "battle".to_string(),
                r#type: "MyOverride".to_string(),
                optional: false,
            }],
        }];

        emmyrc.pre_process_emmyrc(&root);

        let rules = &emmyrc.runtime.class_field_type_rules;
        let bt_rules: Vec<&EmmyrcClassFieldTypeRule> =
            rules.iter().filter(|r| r.class == "BattleBt").collect();
        assert_eq!(bt_rules.len(), 2, "dumped + hand-written rules");

        // Dumped rule first, hand-written rule last (so hand-written wins in
        // `inject_class_field_types`, which keeps the last field declaration).
        let dumped = bt_rules[0];
        let handwritten = bt_rules[1];
        assert_eq!(handwritten.fields[0].r#type, "MyOverride");

        let warrior = dumped.fields.iter().find(|f| f.name == "warrior").unwrap();
        assert_eq!(warrior.r#type, "BattleWarrior");
        assert!(!warrior.optional, "dumped fields default to non-optional");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_pre_process_without_field_type_hints_file() {
        let root = temp_dir();
        let mut emmyrc = Emmyrc::default();
        emmyrc.pre_process_emmyrc(&root);
        assert!(emmyrc.runtime.class_field_type_rules.is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
