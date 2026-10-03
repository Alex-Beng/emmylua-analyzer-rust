use emmylua_parser::{LuaAstNode, LuaChunk, LuaExpr};
use wax::Pattern;

use crate::{
    InferFailReason, LuaDecl, LuaDeclId, LuaMemberKey, LuaSemanticDeclId, LuaSignatureId, LuaType,
    compilation::analyzer::unresolve::UnResolveModule, db_index::LuaObjectType, infer_expr,
};

use super::{LuaAnalyzer, LuaReturnPoint, analyze_func_body_returns_with};

pub fn analyze_chunk_return(analyzer: &mut LuaAnalyzer, chunk: LuaChunk) -> Option<()> {
    let block = chunk.get_block()?;
    let file_id = analyzer.file_id;
    let cache = analyzer.context.infer_manager.get_infer_cache(file_id);
    let return_exprs = analyze_func_body_returns_with(block, &mut |expr| {
        Ok(infer_expr(analyzer.db, cache, expr.clone()).unwrap_or(LuaType::Unknown))
    })
    .unwrap_or_default();
    for point in return_exprs {
        if let LuaReturnPoint::Expr(expr) = point {
            // Module export selection follows the first return candidate.
            // It does not refine `pred()`-style call conditions.
            let expr_type = match analyzer.infer_expr(&expr) {
                Ok(expr_type) => expr_type,
                Err(InferFailReason::None) => LuaType::Unknown,
                Err(reason) => {
                    let unresolve = UnResolveModule {
                        file_id: analyzer.file_id,
                        expr,
                    };
                    analyzer.context.add_unresolve(unresolve.into(), reason);
                    return None;
                }
            };

            let semantic_id = get_semantic_id(analyzer, expr.clone());

            let visibility = semantic_id.as_ref().and_then(|id| {
                analyzer
                    .db
                    .get_property_index()
                    .get_property(id)
                    .map(|p| p.visibility.clone())
            });

            let module_info = analyzer
                .db
                .get_module_index_mut()
                .get_module_mut(analyzer.file_id)?;
            module_info.export_type = Some(expr_type.get_result_slot_type(0).unwrap_or(expr_type));
            module_info.semantic_id = semantic_id;
            if let Some(visibility) = visibility {
                module_info.merge_visibility(visibility);
            }
            break;
        }
    }

    // When a file has no top-level `return` but matches an environment-module
    // pattern, synthesize the export table from its top-level globals.
    analyze_environment_module_exports(analyzer);

    Some(())
}

/// For files matched by `runtime.environment_module_pattern` that expose their
/// module members through top-level global assignments (eg. `BATTLE_STATE_INIT = 1`
/// or `function Foo:bar() end`), synthesize a table type whose fields are those
/// globals. This mirrors runtime loaders that execute a module in an isolated
/// environment and return that environment table.
fn analyze_environment_module_exports(analyzer: &mut LuaAnalyzer) {
    let file_id = analyzer.file_id;

    // Do nothing if the module already has an explicit export.
    if analyzer
        .db
        .get_module_index()
        .get_module(file_id)
        .is_some_and(|m| m.export_type.is_some())
    {
        return;
    }

    if !matches_environment_module_pattern(analyzer, file_id) {
        return;
    }

    let decl_ids = {
        let Some(tree) = analyzer.db.get_decl_index().get_decl_tree(&file_id) else {
            return;
        };
        tree.get_decls()
            .values()
            .filter(|decl| decl.is_global())
            .map(|decl| decl.get_id())
            .collect::<Vec<_>>()
    };

    if decl_ids.is_empty() {
        return;
    }

    let mut fields = Vec::new();
    for decl_id in decl_ids {
        let Some(decl) = analyzer.db.get_decl_index().get_decl(&decl_id).cloned() else {
            continue;
        };
        let name = decl.get_name().to_string();
        let field_type = infer_global_decl_type(analyzer, &decl);
        fields.push((LuaMemberKey::Name(name.into()), field_type));
    }

    if fields.is_empty() {
        return;
    }

    let object = LuaObjectType::new_with_fields(fields.into_iter().collect(), Vec::new());
    if let Some(module_info) = analyzer
        .db
        .get_module_index_mut()
        .get_module_mut(file_id)
    {
        module_info.export_type = Some(LuaType::Object(object.into()));
    }
}

/// Infer the type of a top-level global declaration, preferring the type already
/// bound during declaration analysis and falling back to the value expression.
fn infer_global_decl_type(analyzer: &mut LuaAnalyzer, decl: &LuaDecl) -> LuaType {
    let decl_id = decl.get_id();
    if let Some(type_cache) = analyzer.db.get_type_index().get_type_cache(&decl_id.into()) {
        let ty = type_cache.as_type().clone();
        if !ty.is_unknown() {
            return ty;
        }
    }

    // Fall back to the bound value expression, if any.
    if let Some(value_syntax_id) = decl.get_value_syntax_id() {
        if let Some(expr) = find_expr_by_syntax_id(analyzer, value_syntax_id) {
            return analyzer.infer_expr(&expr).unwrap_or(LuaType::Unknown);
        }
    }

    LuaType::Unknown
}

fn find_expr_by_syntax_id(
    analyzer: &LuaAnalyzer,
    syntax_id: emmylua_parser::LuaSyntaxId,
) -> Option<LuaExpr> {
    let tree = analyzer.db.get_vfs().get_syntax_tree(&analyzer.file_id)?;
    let root = tree.get_chunk_node();
    root.descendants::<LuaExpr>()
        .find(|node| node.get_syntax_id() == syntax_id)
}

fn matches_environment_module_pattern(analyzer: &LuaAnalyzer, file_id: crate::FileId) -> bool {
    let patterns = &analyzer.get_emmyrc().runtime.environment_module_pattern;
    if patterns.is_empty() {
        return false;
    }

    let Some(path) = analyzer.db.get_vfs().get_file_path(&file_id) else {
        return false;
    };
    let path_str = path.to_string_lossy().replace('\\', "/");

    let Ok(pattern) = wax::any(patterns.iter().map(|s| s.as_str())) else {
        return false;
    };

    // Match both the absolute path and the tail, so patterns like
    // "Common/battle_core/**" work regardless of the workspace root.
    if pattern.is_match(path_str.as_str()) {
        return true;
    }

    let relative = analyzer
        .db
        .get_module_index()
        .get_module(file_id)
        .map(|m| m.full_module_name.replace('.', "/"))
        .unwrap_or_default();
    !relative.is_empty() && pattern.is_match(relative.as_str())
}

fn get_semantic_id(analyzer: &LuaAnalyzer, expr: LuaExpr) -> Option<LuaSemanticDeclId> {
    match expr {
        LuaExpr::NameExpr(name_expr) => {
            let name = name_expr.get_name_text()?;
            let tree = analyzer
                .db
                .get_decl_index()
                .get_decl_tree(&analyzer.file_id)?;
            let decl = tree.find_local_decl(&name, name_expr.get_position())?;

            Some(LuaSemanticDeclId::LuaDecl(decl.get_id()))
        }
        LuaExpr::ClosureExpr(closure) => Some(LuaSemanticDeclId::Signature(
            LuaSignatureId::from_closure(analyzer.file_id, &closure),
        )),
        // `return {}`
        LuaExpr::TableExpr(table_expr) => Some(LuaSemanticDeclId::LuaDecl(LuaDeclId::new(
            analyzer.file_id,
            table_expr.get_position(),
        ))),
        _ => None,
    }
}
