use emmylua_parser::{LuaCallExpr, LuaExpr, LuaLiteralToken, LuaSyntaxKind};

use crate::{
    LuaTypeDeclId,
    config::{EmmyrcParamRole, EmmyrcSpecialCallRule},
    db_index::{LuaDecl, LuaDeclExtra, LuaDeclTypeKind, LuaTypeDecl, LuaTypeFlag},
};

use super::DeclAnalyzer;

/// Result of interpreting a framework call against the configured rules.
pub enum SpecialCallKind {
    /// A global variable definition, eg. registerGlobal("NAME", value).
    GlobalDefine { name: String },
    /// A class definition, eg. DefineClass("NAME", Super).
    ClassDefine {
        name: String,
        supers: Vec<LuaExpr>,
    },
}

/// Match a call expression against the configured special-call rules.
pub fn match_special_call(
    analyzer: &DeclAnalyzer,
    call_expr: &LuaCallExpr,
) -> Option<SpecialCallKind> {
    let func_name = get_call_func_name(call_expr)?;
    let emmyrc = analyzer.db.get_emmyrc();

    if let Some(rule) = emmyrc
        .runtime
        .global_define_rules
        .iter()
        .find(|r| r.function == func_name)
    {
        return match_global_define(call_expr, rule);
    }

    if let Some(rule) = emmyrc
        .runtime
        .class_define_rules
        .iter()
        .find(|r| r.function == func_name)
    {
        return match_class_define(call_expr, rule);
    }

    None
}

fn match_global_define(
    call_expr: &LuaCallExpr,
    rule: &EmmyrcSpecialCallRule,
) -> Option<SpecialCallKind> {
    let arg_exprs = collect_args(call_expr);

    let name_idx = rule.get_index(EmmyrcParamRole::Name)?;
    let _ = rule.get_index(EmmyrcParamRole::Value)?;

    let name = get_string_literal_arg(&arg_exprs, name_idx)?;

    Some(SpecialCallKind::GlobalDefine { name })
}

fn match_class_define(
    call_expr: &LuaCallExpr,
    rule: &EmmyrcSpecialCallRule,
) -> Option<SpecialCallKind> {
    let arg_exprs = collect_args(call_expr);

    let name_idx = rule.get_index(EmmyrcParamRole::Name)?;
    let name = get_string_literal_arg(&arg_exprs, name_idx)?;

    let supers = if let Some(super_idx) = rule.get_index(EmmyrcParamRole::Super) {
        arg_exprs.iter().skip(super_idx).cloned().collect()
    } else {
        Vec::new()
    };

    Some(SpecialCallKind::ClassDefine { name, supers })
}

fn collect_args(call_expr: &LuaCallExpr) -> Vec<LuaExpr> {
    call_expr
        .get_args_list()
        .map(|args| args.get_args().collect())
        .unwrap_or_default()
}

fn get_call_func_name(call_expr: &LuaCallExpr) -> Option<String> {
    match call_expr.get_prefix_expr()? {
        LuaExpr::NameExpr(name_expr) => name_expr.get_name_text(),
        _ => None,
    }
}

fn get_string_literal_arg(args: &[LuaExpr], index: usize) -> Option<String> {
    let LuaExpr::LiteralExpr(literal_expr) = args.get(index)? else {
        return None;
    };
    match literal_expr.get_literal()? {
        LuaLiteralToken::String(token) => Some(token.get_value()),
        _ => None,
    }
}

/// Create a synthesized global declaration for a matched special call.
///
/// The declaration's range uses the call expression range (plan scheme A), so
/// "go to definition" lands on the call site.
pub fn add_synthetic_global_decl(
    analyzer: &mut DeclAnalyzer,
    name: &str,
    range: rowan::TextRange,
) -> crate::LuaDeclId {
    let file_id = analyzer.get_file_id();
    let decl = LuaDecl::new(
        name,
        file_id,
        range,
        LuaDeclExtra::Global {
            kind: LuaSyntaxKind::NameExpr.into(),
        },
        None,
    );
    analyzer.add_decl(decl)
}

/// Create a synthesized class type declaration for a matched class call.
pub fn add_synthetic_class_decl(
    analyzer: &mut DeclAnalyzer,
    name: &str,
    range: rowan::TextRange,
) -> LuaTypeDeclId {
    let file_id = analyzer.get_file_id();
    let type_id = LuaTypeDeclId::global(name);
    let type_decl = LuaTypeDecl::new(
        file_id,
        range,
        name.to_string(),
        LuaDeclTypeKind::Class,
        // `Open` lets members that are attached at runtime (via `Name:method`)
        // resolve as `any` instead of reporting a missing field.
        LuaTypeFlag::Open.into(),
        type_id.clone(),
    );
    analyzer
        .db
        .get_type_index_mut()
        .add_type_decl(file_id, type_decl);

    type_id
}
