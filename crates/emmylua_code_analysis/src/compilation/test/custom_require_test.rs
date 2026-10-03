#[cfg(test)]
mod test {
    use crate::{Emmyrc, LuaType, VirtualWorkspace};

    fn emmyrc_with_require_like() -> Emmyrc {
        let mut emmyrc = Emmyrc::default();
        emmyrc.runtime.require_like_function =
            vec!["import".to_string(), "kg_require".to_string()];
        emmyrc
    }

    #[test]
    fn test_import_recognized_as_require() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_require_like());

        ws.def_files(vec![(
            "Common/battle_core/common/battle_const.lua",
            r#"
                BATTLE_STATE_INIT = 1
                return { BATTLE_STATE_INIT = BATTLE_STATE_INIT }
                "#,
        )]);

        let ty = ws.expr_ty(r#"import("Common/battle_core/common/battle_const")"#);
        // Should resolve to the module export (a table const or ref), not unknown.
        assert!(
            !matches!(ty, LuaType::Unknown | LuaType::Nil),
            "import(...) should be recognized as require, got {:?}",
            ty
        );
    }

    #[test]
    fn test_kg_require_recognized_as_require() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_require_like());

        ws.def_files(vec![(
            "Common/battle_core/common/battle_const.lua",
            r#"
                return { BATTLE_STATE_INIT = 1 }
                "#,
        )]);

        let ty = ws.expr_ty(r#"kg_require("Common.battle_core.common.battle_const")"#);
        assert!(
            !matches!(ty, LuaType::Unknown | LuaType::Nil),
            "kg_require(...) should be recognized as require, got {:?}",
            ty
        );
    }

    #[test]
    fn test_import_without_config_is_not_require() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        // No requireLikeFunction configured.
        ws.def_files(vec![(
            "Common/battle_core/common/battle_const.lua",
            r#"
                return { BATTLE_STATE_INIT = 1 }
                "#,
        )]);

        let ty = ws.expr_ty(r#"import("Common/battle_core/common/battle_const")"#);
        assert!(
            matches!(ty, LuaType::Unknown),
            "import(...) without config should be unknown, got {:?}",
            ty
        );
    }

    fn emmyrc_with_environment_module() -> Emmyrc {
        let mut emmyrc = Emmyrc::default();
        emmyrc.runtime.require_like_function =
            vec!["import".to_string(), "kg_require".to_string()];
        emmyrc.runtime.environment_module_pattern =
            vec!["Common/battle_core/**".to_string()];
        emmyrc
    }

    #[test]
    fn test_environment_module_globals_become_members() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());

        ws.def_files(vec![(
            "Common/battle_core/common/battle_const.lua",
            r#"
                ---@module BATTLE_CONST
                BATTLE_STATE_INIT = 1
                BATTLE_STATE_START = 4
                function getStateName(id)
                    return "state"
                end
                "#,
        )]);

        // The synthesized export is a table-like object; member access resolves.
        let integer = ws.ty("integer");
        let init_ty = ws.expr_ty(r#"import("Common/battle_core/common/battle_const").BATTLE_STATE_INIT"#);
        assert!(
            ws.check_type(&init_ty, &integer),
            "BATTLE_STATE_INIT should be integer, got {:?}",
            init_ty
        );

        let fn_ty = ws.expr_ty(r#"import("Common/battle_core/common/battle_const").getStateName"#);
        assert!(
            !matches!(fn_ty, LuaType::Unknown),
            "getStateName should be visible, got {:?}",
            fn_ty
        );
    }

    #[test]
    fn test_non_matching_module_has_no_synthesized_export() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());

        ws.def_files(vec![(
            "Other/plain.lua",
            r#"
                SOME_GLOBAL = 1
                "#,
        )]);

        let ty = ws.expr_ty(r#"import("Other/plain").SOME_GLOBAL"#);
        assert!(
            matches!(ty, LuaType::Unknown | LuaType::Any),
            "non-matching module should not synthesize export, got {:?}",
            ty
        );
    }

    #[test]
    fn test_register_global_defines_global() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        // Default rule: registerGlobal(name=0, value=1)
        ws.def(
            r#"
            local module = { foo = 1, bar = "x" }
            registerGlobal("BATTLE_CORE", module)
            "#,
        );
        ws.def(
            r#"
            local t = BATTLE_CORE.foo
            "#,
        );

        let foo_ty = ws.expr_ty("BATTLE_CORE.foo");
        let integer = ws.ty("integer");
        assert!(
            ws.check_type(&foo_ty, &integer),
            "BATTLE_CORE.foo should be integer, got {:?}",
            foo_ty
        );

        let bar_ty = ws.expr_ty("BATTLE_CORE.bar");
        let string = ws.ty("string");
        assert!(
            ws.check_type(&bar_ty, &string),
            "BATTLE_CORE.bar should be string, got {:?}",
            bar_ty
        );
    }

    #[test]
    fn test_register_global_custom_param_order() {
        use crate::{EmmyrcParamRole, EmmyrcParameterRule, EmmyrcSpecialCallRule};

        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        let mut emmyrc = Emmyrc::default();
        emmyrc.runtime.global_define_rules = vec![EmmyrcSpecialCallRule {
            function: "myRegister".to_string(),
            params: vec![
                EmmyrcParameterRule {
                    role: EmmyrcParamRole::Value,
                    index: 0,
                },
                EmmyrcParameterRule {
                    role: EmmyrcParamRole::Name,
                    index: 1,
                },
            ],
        }];
        ws.update_emmyrc(emmyrc);

        ws.def(
            r#"
            myRegister({ foo = 1 }, "REVERSED")
            "#,
        );

        let ty = ws.expr_ty("REVERSED.foo");
        let integer = ws.ty("integer");
        assert!(
            ws.check_type(&ty, &integer),
            "REVERSED.foo should be integer (custom order), got {:?}",
            ty
        );
    }

    #[test]
    fn test_define_class_creates_class() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        // Default rule: DefineClass(name=0, super=1)
        ws.def(
            r#"
            DefineClass("BattleBt")

            function BattleBt:ctor()
                self.hp = 100
            end

            function BattleBt:run()
            end
            "#,
        );

        // The class name is a global whose type is the class definition.
        let ty = ws.expr_ty("BattleBt");
        assert!(
            matches!(ty, LuaType::Def(_) | LuaType::Ref(_)),
            "BattleBt should be a class definition, got {:?}",
            ty
        );

        // Methods defined with `:` belong to the class.
        let run_ty = ws.expr_ty("BattleBt.run");
        assert!(
            !matches!(run_ty, LuaType::Unknown),
            "BattleBt.run method should be visible, got {:?}",
            run_ty
        );
    }

    #[test]
    fn test_define_class_with_super() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        ws.def(
            r#"
            DefineClass("Base")
            function Base:baseMethod()
            end

            DefineClass("Derived", Base)
            function Derived:derivedMethod()
            end
            "#,
        );

        // Inherited method should be reachable on Derived.
        let ty = ws.expr_ty("Derived.baseMethod");
        assert!(
            !matches!(ty, LuaType::Unknown),
            "Derived should inherit baseMethod from Base, got {:?}",
            ty
        );
    }
}
