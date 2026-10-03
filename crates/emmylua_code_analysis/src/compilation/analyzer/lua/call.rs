use emmylua_parser::{LuaAstNode, LuaCallExpr, LuaExpr};

use crate::{
    InferFailReason, LuaBuiltinAttributeKind, LuaType, LuaTypeCache, LuaTypeDeclId,
    compilation::analyzer::{
        lua::LuaAnalyzer,
        unresolve::{UnResolveCall, UnResolveConstructor},
    },
    config::{EmmyrcParamRole, EmmyrcSpecialCallRule},
};

pub fn analyze_call(analyzer: &mut LuaAnalyzer, call_expr: LuaCallExpr) -> Option<()> {
    analyze_special_call(analyzer, &call_expr);

    let prefix_expr = call_expr.clone().get_prefix_expr()?;
    // Constructor discovery only needs the callee's declared signature. Full
    // flow inference here replays narrowing for every call in call-dense files.
    match analyzer.infer_expr_no_flow(&prefix_expr) {
        Ok(Some(LuaType::Signature(signature_id))) => {
            let signature = analyzer.db.get_signature_index().get(&signature_id)?;
            for (idx, param_info) in signature.param_docs.iter() {
                if param_info
                    .get_builtin_attribute(LuaBuiltinAttributeKind::Constructor)
                    .is_some()
                {
                    let unresolve = UnResolveConstructor {
                        file_id: analyzer.file_id,
                        call_expr: call_expr.clone(),
                        signature_id,
                        param_idx: *idx,
                    };
                    analyzer
                        .context
                        .add_unresolve(unresolve.into(), InferFailReason::None);
                    return Some(());
                }
            }
        }
        Err(InferFailReason::UnResolveDeclType(id)) => {
            let unresolve = UnResolveCall {
                file_id: analyzer.file_id,
                call_expr: call_expr.clone(),
            };
            analyzer
                .context
                .add_unresolve(unresolve.into(), InferFailReason::UnResolveDeclType(id));
        }
        _ => {}
    }
    Some(())
}

/// Bind types for calls matched by `runtime.globalDefineRules` /
/// `runtime.classDefineRules`. The declarations themselves are synthesized
/// during declaration analysis; here we resolve their value / super types.
fn analyze_special_call(analyzer: &mut LuaAnalyzer, call_expr: &LuaCallExpr) {
    let Some(func_name) = get_call_func_name(call_expr) else {
        return;
    };
    let emmyrc = analyzer.get_emmyrc().clone();

    if let Some(rule) = emmyrc
        .runtime
        .global_define_rules
        .iter()
        .find(|r| r.function == func_name)
    {
        bind_global_define(analyzer, call_expr, rule);
        return;
    }

    if let Some(rule) = emmyrc
        .runtime
        .class_define_rules
        .iter()
        .find(|r| r.function == func_name)
    {
        bind_class_define(analyzer, call_expr, rule);
    }
}

fn get_call_func_name(call_expr: &LuaCallExpr) -> Option<String> {
    match call_expr.clone().get_prefix_expr()? {
        LuaExpr::NameExpr(name_expr) => name_expr.get_name_text(),
        _ => None,
    }
}

fn collect_arg_exprs(call_expr: &LuaCallExpr) -> Vec<LuaExpr> {
    call_expr
        .get_args_list()
        .map(|args| args.get_args().collect())
        .unwrap_or_default()
}

fn get_string_arg(args: &[LuaExpr], index: usize) -> Option<String> {
    let LuaExpr::LiteralExpr(literal_expr) = args.get(index)? else {
        return None;
    };
    match literal_expr.get_literal()? {
        emmylua_parser::LuaLiteralToken::String(token) => Some(token.get_value()),
        _ => None,
    }
}

fn find_synthetic_global_decl(
    analyzer: &LuaAnalyzer,
    name: &str,
    range: rowan::TextRange,
) -> Option<crate::LuaDeclId> {
    let file_id = analyzer.file_id;
    let decl_ids = analyzer.db.get_global_index().get_global_decl_ids(name)?;
    // Prefer the decl synthesized in this file at the call expression.
    for decl_id in decl_ids {
        if decl_id.file_id == file_id {
            if let Some(decl) = analyzer.db.get_decl_index().get_decl(decl_id) {
                if decl.get_range() == range {
                    return Some(*decl_id);
                }
            }
        }
    }
    // Fall back to any global decl in this file with the same start position.
    for decl_id in decl_ids {
        if decl_id.file_id == file_id
            && analyzer
                .db
                .get_decl_index()
                .get_decl(decl_id)
                .is_some()
        {
            return Some(*decl_id);
        }
    }
    None
}

fn bind_global_define(
    analyzer: &mut LuaAnalyzer,
    call_expr: &LuaCallExpr,
    rule: &EmmyrcSpecialCallRule,
) {
    let args = collect_arg_exprs(call_expr);
    let Some(name_idx) = rule.get_index(EmmyrcParamRole::Name) else {
        return;
    };
    let Some(value_idx) = rule.get_index(EmmyrcParamRole::Value) else {
        return;
    };
    let Some(name) = get_string_arg(&args, name_idx) else {
        return;
    };
    let Some(value_expr) = args.get(value_idx).cloned() else {
        return;
    };

    let value_type = analyzer.infer_expr(&value_expr).unwrap_or(LuaType::Unknown);
    let value_type = value_type.get_result_slot_type(0).unwrap_or(value_type);

    let range = call_expr.syntax().text_range();
    if let Some(decl_id) = find_synthetic_global_decl(analyzer, &name, range) {
        analyzer
            .db
            .get_type_index_mut()
            .bind_type(decl_id.into(), LuaTypeCache::InferType(value_type));
    }
}

fn bind_class_define(
    analyzer: &mut LuaAnalyzer,
    call_expr: &LuaCallExpr,
    rule: &EmmyrcSpecialCallRule,
) {
    let args = collect_arg_exprs(call_expr);
    let Some(name_idx) = rule.get_index(EmmyrcParamRole::Name) else {
        return;
    };
    let Some(name) = get_string_arg(&args, name_idx) else {
        return;
    };

    let type_id = LuaTypeDeclId::global(&name);
    if analyzer
        .db
        .get_type_index()
        .get_type_decl(&type_id)
        .is_none()
    {
        return;
    }

    // Register super types.
    if let Some(super_idx) = rule.get_index(EmmyrcParamRole::Super) {
        for super_expr in args.iter().skip(super_idx) {
            let super_type = analyzer.infer_expr(super_expr).unwrap_or(LuaType::Unknown);
            if super_type.is_unknown() {
                continue;
            }
            analyzer.db.get_type_index_mut().add_super_type(
                type_id.clone(),
                analyzer.file_id,
                super_type,
            );
        }
    }
}
