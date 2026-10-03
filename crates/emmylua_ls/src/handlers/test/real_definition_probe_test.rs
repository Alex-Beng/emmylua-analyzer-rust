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

    #[test]
    fn test_real_battle_ai_definition_and_hover() {
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
            (
                generator.new_uri("Common/battle_core/battle_ai.lua"),
                Some(ai),
            ),
        ]);

        let ai_uri = generator.new_uri("Common/battle_core/battle_ai.lua");
        let file_id = analysis.get_file_id(&ai_uri).expect("battle_ai file id");

        // Line 868 (1-based) -> 867 (0-based), cursor on `_doAndOrLogic`.
        // `local ret = self:_doAndOrLogic` -> name starts at column 41.
        let pos = Position::new(867, 42);
        let result = definition(&analysis, file_id, pos);
        eprintln!("definition result => {:?}", result);
        assert!(result.is_some(), "goto definition should resolve inherited method");

        // Hover on the same member.
        let hover_result = crate::handlers::hover::hover(&analysis, file_id, pos);
        if let Some(h) = hover_result {
            if let lsp_types::HoverContents::Markup(m) = h.contents {
                eprintln!("hover => {}", m.value);
                assert!(
                    !m.value.trim_start().starts_with("any"),
                    "hover should not be `any`"
                );
            }
        } else {
            eprintln!("hover => none");
        }
    }
}
