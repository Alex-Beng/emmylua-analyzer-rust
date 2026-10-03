# 自定义 require / 全局注册 / 类定义 支持计划

## 背景

项目（如 `uptodate_server`）使用运行时框架约定，emmylua_ls 无法静态识别：

- `import("Common/battle_core/xxx")` / `kg_require("...")`：自定义加载器，返回**模块环境表**（模块内顶层全局成为其成员）。
- `registerGlobal("NAME", value)`：把 `value` 注册为全局 `NAME`。
- `DefineClass("Name", Super)` / `DefineComponent(...)`：运行时定义类。

这些模块文件**没有 `return`**，因此 `export_type` 为空，`import(...)` 推断失败；`BATTLE_CORE` 这类全局名也无定义。

## 目标

不修改用户代码、不要求注解，通过**配置 + 改 emmylua_ls 源码**实现：

| # | 需求 | 效果 |
| --- | --- | --- |
| 3a | 模块无 return 时，顶层全局合成模块导出表 | `import(...)` 后可见模块 global 变量/函数 |
| 3b | 识别 `registerGlobal("X", v)` | `BATTLE_CORE.xxx` 可见成员 |
| 4 | 识别 `DefineClass("Name", Super)` | 自动定义类、方法归属、继承 |

## 设计决策

- 配置采用**位置感知规则**（固定索引 + 角色），应对不同项目参数顺序不一致。
- 角色：`name`（声明名，必为字符串常量）、`value`（值/模块）、`super`（父类）。
- 复用 analyzer 现有 `LuaTypeDecl`（class 机制）与 `LuaMemberOwner::Type`，不新增 `LuaType` 变体。
- 虚拟 decl 的位置采用**方案 a**：用调用表达式的 `TextRange` 冒充（跳转定位到调用行）。
- `className` 全局唯一；名称参数必为字符串常量（不存在变量情况）。
- 默认行为生效：`registerGlobal` 默认 `name=0,value=1`；`DefineClass` 默认 `name=0,super=1`。

## 配置示例

```jsonc
{
  "runtime": {
    "requireLikeFunction": ["import", "kg_require"],
    "environmentModulePattern": ["Common/battle_core/**"],
    "globalDefineRules": [
      {
        "function": "registerGlobal",
        "params": [
          { "role": "name",  "index": 0 },
          { "role": "value", "index": 1 }
        ]
      }
    ],
    "classDefineRules": [
      {
        "function": "DefineClass",
        "params": [
          { "role": "name",  "index": 0 },
          { "role": "super", "index": 1 }
        ]
      }
    ]
  }
}
```

## 任务清单

### T0：准备与基线
- [x] 确认构建/测试命令（`cargo build`、`cargo test -p emmylua_code_analysis`）
- [x] 用 `VirtualWorkspace` 写复现测试：模块无 return 时 `import` 推断失败
- [x] 确认 `script` 目录被 `ignoreDir` 排除的影响

### T1：配置层（位置感知规则）
- [x] `config/configs/runtime.rs` 新增：
  - `EmmyrcParamRole`（`name` / `value` / `super`）
  - `EmmyrcParameterRule { role, index }`
  - `EmmyrcSpecialCallRule { function, params }`
  - `runtime.globalDefineRules: Vec<EmmyrcSpecialCallRule>`
  - `runtime.classDefineRules: Vec<EmmyrcSpecialCallRule>`
  - `runtime.environmentModulePattern: Vec<String>`
- [x] 保留 `requireLikeFunction` 向后兼容
- [x] 补 `Default` 与 JSON Schema
- [x] 单测：配置反序列化

### T2：`import`/`kg_require` → require
- [x] 复用 `requireLikeFunction` → `SpecialFunction::Require`（`config/mod.rs:79`）
- [x] 验证 `/`→`.` 归一化命中（`db_index/module/mod.rs:220`）
- [x] 测试：`import("Common/battle_core/x")` 能 find_module

### T3：模块全局合成导出表
- [x] 改 `compilation/analyzer/lua/module.rs::analyze_chunk_return`
- [x] 无 return 且匹配 `environmentModulePattern`：收集顶层全局 → 合成 `LuaObjectType` → 设 `export_type`
- [x] 测试：`import(...)` 后模块 global 可见（需求 3a）

### T4：`registerGlobal` 识别
- [x] 在调用分析中识别 `registerGlobal(name, value)`（配置规则驱动）
- [x] 注册全局 decl（range 用调用处，方案 a）+ 绑定 value 类型
- [x] 测试：`BATTLE_CORE.xxx` 可见（需求 3b）

### T5：`DefineClass` 识别
- [x] 识别 `DefineClass(name, super)`
- [x] 合成 class 类型 + 全局绑定 + 继承
- [x] 测试：`Bt:method()` 归属到类 / 继承可见

### T6：文档与回归
- [x] 更新 `docs/config/emmyrc_json_CN.md` / `_EN.md`
- [x] 重新生成 `resources/schema.json`
- [x] 全量 `cargo test -p emmylua_code_analysis` 回归（1102 passed）
- [x] `cargo clippy -p emmylua_code_analysis` 无新增告警

## 验收标准

| 需求 | 验收 |
| --- | --- |
| 3a | `import("Common/battle_core/battle_const")` 后 `.BATTLE_STATE_INIT` 可补全/跳转 |
| 3b | `BATTLE_CORE.BattleCore` 等成员可补全/跳转 |
| 4 | `DefineClass("X")` 后 `X:` 补全方法 |
| 通用性 | 参数顺序可通过配置调整 |

## 关键代码位置索引

- 模块导出：`crates/emmylua_code_analysis/src/compilation/analyzer/lua/module.rs:10`
- require 推断：`crates/emmylua_code_analysis/src/semantic/infer/infer_call/infer_require.rs:32`
- 模块查找：`crates/emmylua_code_analysis/src/db_index/module/mod.rs:219`
- require 函数识别：`crates/emmylua_code_analysis/src/config/mod.rs:79`
- 特殊函数：`crates/emmylua_parser/src/parser/parser_config.rs`
- 类声明分析：`crates/emmylua_code_analysis/src/compilation/analyzer/doc/type_def_tags.rs:22`
- 类型声明构造：`crates/emmylua_code_analysis/src/compilation/analyzer/decl/docs.rs:181`
- 成员归属：`crates/emmylua_code_analysis/src/compilation/analyzer/lua/stats.rs:257`
- 成员查找：`crates/emmylua_code_analysis/src/semantic/member/find_members.rs:153`
- 全局索引：`crates/emmylua_code_analysis/src/db_index/global/mod.rs`

## 实现记录

实际改动文件（相对计划有少量调整）：

- `config/configs/runtime.rs`：新增 `EmmyrcParamRole` / `EmmyrcParameterRule` /
  `EmmyrcSpecialCallRule`；`EmmyrcRuntime` 改为手写 `Default`，默认含
  `registerGlobal` 全局规则与 `DefineClass`/`DefineComponent` 类规则；新增
  `environment_module_pattern`。
- `config/configs/mod.rs`、`config/mod.rs`：导出新类型。
- `compilation/analyzer/lua/module.rs`：新增
  `analyze_environment_module_exports`，对匹配 glob 且无 `return` 的文件，
  用顶层全局合成模块导出。
  - **修订（v2）**：从 `LuaObjectType` 改为**合成的文件作用域 `LuaTypeDecl`(Class)
    + 成员注册**。因为 `Object` 字段只有类型、没有 `LuaMemberId`/源码位置，
    hover 可用但**无法跳转到定义**。现在每个顶层全局都注册为
    `LuaMemberOwner::Type(file-scoped id)` 的成员，`member_id` 指向该全局声明，
    故 Go to Definition 可精确跳到模块文件内的定义行。
  - 类名用 `LuaTypeDeclId::file(file_id, "@module:{id}")`（文件作用域），
    避免污染全局命名空间与用户类名冲突。
- `compilation/analyzer/decl/special_call.rs`（新增）：解析配置规则、匹配调用、
  合成全局 / 类声明。
  - **修订（v3）**：合成的类设置 `LuaTypeFlag::Open`，且 P1 修复了 flag 与 id
    一致性；对象/类成员访问对动态括号键与 `nil` 占位成员回退 `any`。
- `db_index/type/type_decl.rs`：`LuaTypeFlag` 底层类型 `u8`→`u16`，新增 `Open`；
  新增 `LuaTypeDecl::is_open()`。
- `semantic/infer/infer_index/mod.rs`：`infer_custom_type_member` 对 open 类型：
  - 括号键（`obj[...]`）访问已存在成员时返回 `any`（运行时可能缺失）；
  - 命中成员类型为 `nil` 时返回 `any`（占位赋值）；
  - 未命中成员返回 `any`（不回退报错）。
- `compilation/analyzer/lua/module.rs`、`lua/call.rs`：合成类型的成员/全局值类型
  用 `widen_literal_type` 宽化字面量（`1`→`integer` 等），避免常量折叠误报。
- `compilation/analyzer/decl/exprs.rs`：在 `analyze_call_expr` 里接入
  `analyze_special_call_decl`。
- `compilation/analyzer/lua/call.rs`：新增 `analyze_special_call`，绑定全局值类型、
  注册类的父类型。
- `compilation/test/custom_require_test.rs`（新增）：13 个测试覆盖 T2–T5、成员定位、
  开放兜底、flag 一致性。
- 文档与 schema 更新。

### v3 诊断对比（ON vs OFF，真实项目 `uptodate_server/script`）

修复前启用功能会导致 `undefined-field` 252→2630 等大量误报；修复后：

| 诊断 | OFF | ON |
| --- | --- | --- |
| inconsistent-type-access-modifier | 0 | 0 |
| undefined-field | 252 | 146 |
| unnecessary-if | 460 | 413 |
| call-non-callable | 616 | 387 |
| need-check-nil | 982 | 347 |
| assign-type-mismatch | 178 | 187 |

绝大多数诊断低于基线，仅 `duplicate-type`(+9)、`return-type-mismatch`(+16) 等少量
新增，多为真实存在的重复类/类型问题。

### v4 修复：跨文件定时序（继承方法丢失）

**现象**：`DefineClass("BattleAI", BATTLE_BT.BattleBt)` 后，`self:_doAndOrLogic()`
（父类方法）补全缺失、类型解析为 `any`、无法跳转。

**根因**：跨文件时序。`battle_ai.lua` 分析时 `BATTLE_BT`（由 `require.lua` 的
`registerBattleModule` 定义、值为 `import(...)`）尚未解析：
- `bind_global_define` 一次性 `infer_expr` 得到 `Unknown` 后**永久绑定**，无重试；
- `bind_class_define` 的父类 `BATTLE_BT.BattleBt` 因此为 `Unknown` → `add_super_type`
  **被跳过**，导致 `BattleAI` super 缺失 → 继承成员全部解析为 `any`
  （命中 Open 兜底）。而 find reference 走同名引用索引 + origin 回溯，故仍可用。

**修复**：
- `lua/call.rs::bind_global_define`：值表达式推断失败/未解析时注册 `UnResolveDecl`
  重试（复用现有机制）。
- 新增 `UnResolveSuperType` 变体（`unresolve/mod.rs` + `resolve.rs::try_resolve_super_type`）：
  `bind_class_define` 的父类表达式未解析时注册重试，分析全部结束后再补 `add_super_type`。

修复后 `BattleAI super_types => Some([Def(BattleBt)])`，继承方法
（`_doAndOrLogic`/`recordTriggerId`/...）均可补全与跳转。诊断数量不回归。

回归测试：`compilation/test/cross_module_inherit_test.rs`（加载真实项目三文件，
缺失时自动跳过）。

### v5 修复：忽略 `.d.lua` stub 文件

**现象**：真实项目全量加载（10151 文件）下，`self:_doAndOrLogic` 的跳转/hover
仍解析为 `any`；但只加载 3 个文件时正常。

**根因**：项目里的手写 stub `Common/define_class.d.lua` 也声明了同名全局
（`BattleBt = {}`、`BattleAI = BATTLE_BT.BattleBt`），与我们从 `DefineClass`
合成的全局/类**竞争**。全量加载时该 stub 被当普通 `.lua` 索引，导致
`BattleAI` 的全局/类型解析被其干扰（取决于解析顺序，时好时坏）。

**修复**：
- 删除项目里的 `.d.lua` stub（用户侧）。
- 分析器**默认忽略 `**/*.d.lua`**（`vfs/collect_workspace_files.rs` 的
  `calculate_include_and_exclude`）。`.d.lua` 是 LuaLS 风格的手写声明文件，
  常与真实代码重复声明，属默认应排除。

修复后全量加载下 `BattleAI` 的 super 与继承方法均正确解析。
回归测试：`vfs::collect_workspace_files` 新增
`dot_d_lua_files_are_ignored_by_default`；LS 层
`real_definition_probe_test` 覆盖增量编辑/诊断后仍可跳转。


已知限制 / 后续可优化：

- 合成全局 decl 的 `TextRange` 采用调用表达式范围（方案 a），跳转定位到调用行。
- `registerBattleModule(name, module)` 这类**包装函数**：因内部 `registerGlobal(name,...)`
  用的是参数而非字面量，静态不可知；需在配置里直接为包装函数加规则
  （`globalDefineRules` 指向 `registerBattleModule`，`name=0,value=1`）。

