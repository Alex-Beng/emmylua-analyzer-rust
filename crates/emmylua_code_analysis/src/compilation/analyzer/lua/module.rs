use emmylua_parser::{LuaAstNode, LuaChunk, LuaExpr};
use wax::Pattern;

use crate::{
    InferFailReason, LuaDecl, LuaDeclId, LuaMember, LuaMemberFeature, LuaMemberId, LuaMemberKey,
    LuaMemberOwner, LuaSemanticDeclId, LuaSignatureId, LuaType, LuaTypeCache, LuaTypeDecl,
    LuaTypeDeclId,
    compilation::analyzer::unresolve::UnResolveModule,
    db_index::{LuaDeclTypeKind, LuaTypeFlag},
    infer_expr,
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
/// or `function Foo:bar() end`), synthesize a class type whose members are those
/// globals. This mirrors runtime loaders that execute a module in an isolated
/// environment and return that environment table.
///
/// Using a real `LuaTypeDecl` (instead of a plain `LuaObjectType`) registers the
/// module members in the member index, so both hover and go-to-definition work.
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

    let decls = {
        let Some(tree) = analyzer.db.get_decl_index().get_decl_tree(&file_id) else {
            return;
        };
        tree.get_decls()
            .values()
            .filter(|decl| decl.is_global())
            .cloned()
            .collect::<Vec<_>>()
    };

    if decls.is_empty() {
        return;
    }

    // Synthesize a file-scoped class for this module's export surface.
    // A file-scoped id avoids polluting the global namespace and prevents
    // conflicts with user-defined classes.
    let export_name = format!("@module:{}", file_id.id);
    let type_id = LuaTypeDeclId::file(file_id, &export_name);
    let decl_range = {
        let chunk = analyzer
            .db
            .get_vfs()
            .get_syntax_tree(&file_id)
            .map(|tree| tree.get_chunk_node().get_range())
            .unwrap_or_default();
        chunk
    };
    analyzer.db.get_type_index_mut().add_type_decl(
        file_id,
        LuaTypeDecl::new(
            file_id,
            decl_range,
            export_name,
            LuaDeclTypeKind::Class,
            LuaTypeFlag::File | LuaTypeFlag::Open,
            type_id.clone(),
        ),
    );

    let owner = LuaMemberOwner::Type(type_id.clone());

    let mut member_count = 0usize;
    for decl in &decls {
        let name = decl.get_name().to_string();
        let field_type = infer_global_decl_type(analyzer, decl);

        // Register the global as a member of the synthesized class so that
        // go-to-definition can resolve back to the global's declaration.
        let syntax_id = decl.get_syntax_id();
        let member_id = LuaMemberId::new(syntax_id, file_id);
        let key = LuaMemberKey::Name(name.into());
        analyzer.db.get_type_index_mut().bind_type(
            member_id.into(),
            LuaTypeCache::InferType(field_type.clone()),
        );
        analyzer.db.get_member_index_mut().add_member(
            owner.clone(),
            LuaMember::new(member_id, key, LuaMemberFeature::FileDefine, None),
        );

        member_count += 1;
    }

    if member_count == 0 {
        return;
    }

    if let Some(module_info) = analyzer.db.get_module_index_mut().get_module_mut(file_id) {
        module_info.export_type = Some(LuaType::Def(type_id));
    }
}

/// Infer the type of a top-level global declaration, preferring the type already
/// bound during declaration analysis and falling back to the value expression.
///
/// Literal types are widened (`1` -> `integer`, `"x"` -> `string`, ...). Module
/// exports are runtime values that can be assigned to freely, so keeping the
/// literal would make downstream checks (eg. always-truthy conditions and
/// assign-type-mismatch) fire spuriously.
fn infer_global_decl_type(analyzer: &mut LuaAnalyzer, decl: &LuaDecl) -> LuaType {
    let decl_id = decl.get_id();
    if let Some(type_cache) = analyzer.db.get_type_index().get_type_cache(&decl_id.into()) {
        let ty = type_cache.as_type().clone();
        if !ty.is_unknown() {
            return widen_type(ty);
        }
    }

    // Fall back to the bound value expression, if any.
    if let Some(value_syntax_id) = decl.get_value_syntax_id() {
        if let Some(expr) = find_expr_by_syntax_id(analyzer, value_syntax_id) {
            return widen_type(analyzer.infer_expr(&expr).unwrap_or(LuaType::Unknown));
        }
    }

    LuaType::Unknown
}

fn widen_type(ty: LuaType) -> LuaType {
    crate::widen_literal_type(ty)
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
