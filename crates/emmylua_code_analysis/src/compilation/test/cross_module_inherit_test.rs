#[cfg(test)]
mod test {
    use std::fs;

    use crate::{Emmyrc, LuaType, VirtualWorkspace};

    fn emmyrc_with_environment_module() -> Emmyrc {
        use crate::{EmmyrcParamRole, EmmyrcParameterRule, EmmyrcSpecialCallRule};
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
        emmyrc
    }

    /// Regression: a class whose super comes from another module
    /// (`DefineClass("BattleAI", BATTLE_BT.BattleBt)`) must inherit the super's
    /// methods, even though the super's binding is resolved later (cross-file).
    /// Without unresolve retries, the super stayed `Any` and inherited members
    /// (eg. `_doAndOrLogic`) resolved to `any`.
    #[test]
    fn test_cross_module_inherited_method_resolves() {
        let base = "D:/1_Dev/Z1/uptodate_server/script/Common/battle_core";
        let (Ok(req), Ok(bt), Ok(ai)) = (
            fs::read_to_string(format!("{}/require.lua", base)),
            fs::read_to_string(format!("{}/battle_bt.lua", base)),
            fs::read_to_string(format!("{}/battle_ai.lua", base)),
        ) else {
            eprintln!("skipping: real project files not found");
            return;
        };

        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());
        ws.def_files(vec![
            ("Common/battle_core/require.lua", req.as_str()),
            ("Common/battle_core/battle_bt.lua", bt.as_str()),
            ("Common/battle_core/battle_ai.lua", ai.as_str()),
        ]);

        // The registered module global must resolve.
        let bt_module = ws.expr_ty("BATTLE_BT");
        assert!(
            !matches!(bt_module, LuaType::Unknown | LuaType::Any),
            "BATTLE_BT should resolve to the module, got {:?}",
            bt_module
        );

        // The derived class must have the super registered.
        {
            use crate::LuaTypeDeclId;
            let db = ws.analysis.compilation.get_db();
            let ai_id = LuaTypeDeclId::global("BattleAI");
            let supers = db.get_type_index().get_super_types(&ai_id);
            assert!(
                supers
                    .as_ref()
                    .is_some_and(|s| s.iter().any(|t| matches!(t, LuaType::Def(_) | LuaType::Ref(_)))),
                "BattleAI super should be a concrete class, got {:?}",
                supers
            );
        }

        // Inherited methods must resolve to the super's definitions.
        for name in ["_doAndOrLogic", "recordTriggerId", "runFunForPerNode"] {
            let ty = ws.expr_ty(&format!("BattleAI.{}", name));
            assert!(
                !matches!(ty, LuaType::Unknown | LuaType::Any),
                "BattleAI.{} should inherit from BattleBt, got {:?}",
                name,
                ty
            );
        }
    }
}
