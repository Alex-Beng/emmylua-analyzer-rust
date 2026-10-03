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
    fn test_environment_module_member_has_definition_location() {
        use crate::{LuaMemberKey, LuaMemberOwner, LuaType, LuaTypeDeclId};

        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());

        let module_file = ws.def_file(
            "Common/battle_core/common/battle_const.lua",
            r#"
                BATTLE_STATE_INIT = 1
                BATTLE_STATE_START = 4
                "#,
        );

        // The module export type is a synthesized file-scoped class.
        let export_type = ws
            .analysis
            .compilation
            .get_db()
            .get_module_index()
            .get_module(module_file)
            .and_then(|m| m.export_type.clone())
            .expect("module should have synthesized export");
        let LuaType::Def(type_id) = export_type else {
            panic!("expected Def export type, got {export_type:?}");
        };
        assert!(
            matches!(type_id.get_id(), crate::LuaTypeIdentifier::File(_, _)),
            "synthesized type should be file-scoped, got {type_id:?}"
        );

        // The member must be registered under the synthesized class owner,
        // pointing at the global's declaration in the module file.
        let db = ws.analysis.compilation.get_db();
        let member_index = db.get_member_index();
        let owner = LuaMemberOwner::Type(LuaTypeDeclId::file(
            module_file,
            type_id.get_name(),
        ));
        let key = LuaMemberKey::Name("BATTLE_STATE_INIT".into());
        let member = member_index
            .get_member_item(&owner, &key)
            .and_then(|_| member_index.get_members(&owner))
            .and_then(|members| {
                members
                    .iter()
                    .find(|m| m.get_key() == &key)
                    .map(|m| m.get_id())
            })
            .expect("BATTLE_STATE_INIT member must be registered");

        let location_file = member.file_id;
        assert_eq!(
            location_file, module_file,
            "member definition must point into the module file"
        );
    }

    #[test]
    fn test_environment_module_unknown_member_falls_back_to_any() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());

        ws.def_files(vec![(
            "Common/battle_core/common/battle_const.lua",
            r#"
                BATTLE_STATE_INIT = 1
                "#,
        )]);

        // A member that is not a top-level global (eg. defined at runtime)
        // should resolve to `any` instead of reporting a missing field.
        let ty = ws.expr_ty(
            r#"import("Common/battle_core/common/battle_const").SOME_RUNTIME_MEMBER"#,
        );
        assert!(
            matches!(ty, LuaType::Any),
            "unknown module member should fall back to any, got {:?}",
            ty
        );

        // `undefined-field` should not fire for open module types.
        let mut ws2 = VirtualWorkspace::new_with_init_std_lib();
        ws2.update_emmyrc(emmyrc_with_environment_module());
        ws2.def_files(vec![(
            "Common/battle_core/common/battle_const.lua",
            r#"
                BATTLE_STATE_INIT = 1
                "#,
        )]);
        assert!(
            ws2.has_no_diagnostic(
                crate::DiagnosticCode::UndefinedField,
                r#"local x = import("Common/battle_core/common/battle_const").SOME_RUNTIME_MEMBER"#,
            ),
            "undefined-field should not fire for open module exports"
        );
    }

    #[test]
    fn test_environment_module_type_flags_are_consistent() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());

        let module_file = ws.def_file(
            "Common/battle_core/common/battle_const.lua",
            r#"
                BATTLE_STATE_INIT = 1
                "#,
        );

        let db = ws.analysis.compilation.get_db();
        let type_id = db
            .get_module_index()
            .get_module(module_file)
            .and_then(|m| m.export_type.clone())
            .and_then(|t| match t {
                LuaType::Def(id) => Some(id),
                _ => None,
            })
            .expect("synthesized export type");
        let type_decl = db
            .get_type_index()
            .get_type_decl(&type_id)
            .expect("type decl must exist");
        assert!(type_decl.is_open(), "module export type should be open");
    }

    #[test]
    fn test_environment_module_index_member_not_always_truthy() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());

        ws.def_files(vec![
            (
                "Common/battle_core/common/formula.lua",
                r#"
                    skillFormula101 = function(a, b) return a + b end
                    "#,
            ),
            (
                "virtual_use.lua",
                r#"
                    local funcName = "skillFormula101"
                    local func = import("Common/battle_core/common/formula")[funcName]
                    if func then
                        return func(1, 2)
                    end
                    "#,
            ),
        ]);

        // `func` comes from an open module export indexed by a dynamic key,
        // so it must not be treated as always-truthy.
        let ty = ws.expr_ty(
            r#"local fname = "skillFormula101" local x = import("Common/battle_core/common/formula")[fname] x"#,
        );
        assert!(
            !ty.is_always_truthy(),
            "dynamic module member should not be always-truthy, got {:?}",
            ty
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

    #[test]
    fn test_define_class_method_body_errors_still_registers_member() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        // Mirrors battle_bt.lua: a method whose body references a method defined
        // later in the file (forward reference), and uses a value that may fail
        // inference. The member must still be registered.
        ws.def(
            r#"
            DefineClass("BattleBt")

            function BattleBt:init()
                self.triggerType = nil
            end

            function BattleBt:_doAndOrLogic(idList, andor, cmpFunc, triggerId)
                andor = andor or '&'
                if #idList > 0 then
                    if andor == '&' then
                        for _, id in ipairs(idList) do
                            if not cmpFunc(id) then
                                return false
                            end
                        end
                        return true
                    elseif andor == '|' then
                        for _, id in ipairs(idList) do
                            if cmpFunc(id) then
                                self:recordTriggerId(triggerId, id)
                                return true
                            end
                        end
                        return false
                    end
                end
                return false
            end

            function BattleBt:recordTriggerId(triggerId, recordId)
            end
            "#,
        );

        // Both members must be registered on the class.
        let init_ty = ws.expr_ty("BattleBt.init");
        assert!(
            !matches!(init_ty, LuaType::Unknown),
            "BattleBt.init should be registered, got {:?}",
            init_ty
        );
        let do_ty = ws.expr_ty("BattleBt._doAndOrLogic");
        assert!(
            !matches!(do_ty, LuaType::Unknown | LuaType::Any),
            "BattleBt._doAndOrLogic should be registered with a concrete type, got {:?}",
            do_ty
        );
    }

    #[test]
    fn test_derived_self_sees_inherited_method() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        ws.def(
            r#"
            DefineClass("BattleBt")
            function BattleBt:_doAndOrLogic(idList, andor, cmpFunc, triggerId)
                return false
            end

            DefineClass("BattleAI", BattleBt)
            function BattleAI:run()
                return self:_doAndOrLogic({}, "&", function(id) return true end, 1)
            end
            "#,
        );

        // `self:_doAndOrLogic` inside a derived method resolves to the
        // inherited method, not `any`.
        let ty = ws.expr_ty("BattleAI._doAndOrLogic");
        assert!(
            !matches!(ty, LuaType::Unknown | LuaType::Any),
            "BattleAI should inherit _doAndOrLogic, got {:?}",
            ty
        );
    }

    #[test]
    fn test_self_colon_call_inside_same_class_resolves() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();

        // Directly mirror the real file: method A calls self:methodB where
        // methodB is defined *after* A.
        ws.def_file(
            "battle_bt.lua",
            r#"
            DefineClass("BattleBt")

            function BattleBt:getX()
                return self:_doAndOrLogic({1,2}, "&", function(id) return true end, 1)
            end

            function BattleBt:_doAndOrLogic(idList, andor, cmpFunc, triggerId)
                andor = andor or '&'
                if #idList > 0 then
                    if andor == '&' then
                        for _, id in ipairs(idList) do
                            if not cmpFunc(id) then
                                return false
                            end
                        end
                        return true
                    end
                end
                return false
            end
            "#,
        );

        // The call result should be boolean, proving `_doAndOrLogic` resolves.
        let ty = ws.expr_ty("BattleBt:_doAndOrLogic({1}, '&', function(q) return true end, 1)");
        assert!(
            !matches!(ty, LuaType::Unknown | LuaType::Any),
            "self:_doAndOrLogic should resolve, got {:?}",
            ty
        );
    }

    #[test]
    fn test_cross_module_super_method_visible() {
        let mut ws = VirtualWorkspace::new_with_init_std_lib();
        ws.update_emmyrc(emmyrc_with_environment_module());

        ws.def_files(vec![
            (
                "Common/battle_core/battle_bt.lua",
                r#"
                ---@module BATTLE_BT
                DefineClass("BattleBt")
                function BattleBt:_doAndOrLogic(idList, andor, cmpFunc, triggerId)
                    return false
                end
                "#,
            ),
            (
                "Common/battle_core/battle_ai.lua",
                r#"
                ---@module BATTLE_AI
                local BATTLE_BT = import("Common/battle_core/battle_bt")
                DefineClass("BattleAI", BATTLE_BT.BattleBt)
                function BattleAI:run()
                    return self:_doAndOrLogic({}, "&", function(id) return true end, 1)
                end
                "#,
            ),
        ]);

        let ty = ws.expr_ty("import('Common/battle_core/battle_bt').BattleBt._doAndOrLogic");
        assert!(
            !matches!(ty, LuaType::Unknown | LuaType::Any),
            "cross-module _doAndOrLogic should be visible, got {:?}",
            ty
        );
    }
}
