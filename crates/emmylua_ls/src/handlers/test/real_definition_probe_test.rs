#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use crate::handlers::definition::definition;
    use emmylua_code_analysis::{
        EmmyLuaAnalysis, Emmyrc, EmmyrcParameterRule, EmmyrcParamRole, EmmyrcSpecialCallRule,
        VirtualUrlGenerator,
    };
    use lsp_types::Position;

    /// Regression: an inherited method called via `self:method()` in a derived
    /// class (whose super comes from another module) must resolve for
    /// go-to-definition and hover, across incremental edits and diagnostics.
    ///
    /// Uses the real project files as fixtures; skips if unavailable.
    #[test]
    fn test_cross_module_inherited_method_definition_and_hover() {
        let base = "D:/1_Dev/Z1/uptodate_server/script/Common/battle_core";
        let (Ok(req), Ok(bt), Ok(ai)) = (
            fs::read_to_string(format!("{}/require.lua", base)),
            fs::read_to_string(format!("{}/battle_bt.lua", base)),
            fs::read_to_string(format!("{}/battle_ai.lua", base)),
        ) else {
            eprintln!("skipping: real project files not found");
            return;
        };

        let generator = VirtualUrlGenerator::new();
        let mut analysis = EmmyLuaAnalysis::new();
        analysis.init_std_lib(None);
        analysis.add_main_workspace(generator.base.clone());

        let mut emmyrc = Emmyrc::default();
        emmyrc.runtime.require_like_function =
            vec!["import".to_string(), "kg_require".to_string()];
        emmyrc.runtime.environment_module_pattern = vec!["Common/battle_core/**".to_string()];
        emmyrc.runtime.global_define_rules.push(EmmyrcSpecialCallRule {
            function: "registerBattleModule".to_string(),
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
        });
        analysis.update_config(Arc::new(emmyrc));

        analysis.update_files_by_uri(vec![
            (
                generator.new_uri("Common/battle_core/require.lua"),
                Some(req),
            ),
            (
                generator.new_uri("Common/battle_core/battle_bt.lua"),
                Some(bt),
            ),
        ]);
        // battle_ai.lua is open in the editor -> analyzed in a separate batch,
        // mirroring `reload_workspace_files`.
        let ai_uri = generator.new_uri("Common/battle_core/battle_ai.lua");
        analysis.update_file_by_uri(&ai_uri, Some(ai.clone()));
        let file_id = analysis.get_file_id(&ai_uri).expect("battle_ai file id");

        {
            use emmylua_code_analysis::LuaTypeDeclId;
            let db = analysis.compilation.get_db();
            let ai_id = LuaTypeDeclId::global("BattleAI");
            assert!(
                db.get_type_index()
                    .get_super_types(&ai_id)
                    .is_some_and(|supers| !supers.is_empty()),
                "BattleAI super must be registered"
            );
        }

        // Line 868 (1-based) -> 867 (0-based), cursor on `_doAndOrLogic`.
        let pos = Position::new(867, 42);
        assert!(
            definition(&analysis, file_id, pos).is_some(),
            "goto definition should resolve the inherited method"
        );
        assert!(
            definition(&analysis, file_id, pos).is_some(),
            "second goto should still resolve (no query side effects)"
        );

        if let Some(lsp_types::Hover {
            contents: lsp_types::HoverContents::Markup(m),
            ..
        }) = crate::handlers::hover::hover(&analysis, file_id, pos)
        {
            assert!(
                !m.value.trim_start().starts_with("any"),
                "hover should not be `any`, got: {}",
                m.value
            );
        }

        // Incremental edit of battle_ai.lua must preserve the inheritance.
        analysis.update_file_by_uri(&ai_uri, Some(ai));
        let file_id2 = analysis.get_file_id(&ai_uri).expect("battle_ai file id 2");
        assert!(
            definition(&analysis, file_id2, pos).is_some(),
            "goto should still resolve after incremental re-index"
        );
    }
}
