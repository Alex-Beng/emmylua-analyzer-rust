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
  用顶层全局合成 `LuaObjectType` 作为 `export_type`（比原计划直接用
  `LuaTypeDecl` 更简单，且 `Object` 的成员查找/枚举天然可用）。
- `compilation/analyzer/decl/special_call.rs`（新增）：解析配置规则、匹配调用、
  合成全局 / 类声明。
- `compilation/analyzer/decl/exprs.rs`：在 `analyze_call_expr` 里接入
  `analyze_special_call_decl`。
- `compilation/analyzer/lua/call.rs`：新增 `analyze_special_call`，绑定全局值类型、
  注册类的父类型。
- `compilation/test/custom_require_test.rs`（新增）：9 个测试覆盖 T2–T5。
- 文档与 schema 更新。

已知限制 / 后续可优化：

- 合成全局 decl 的 `TextRange` 采用调用表达式范围（方案 a），跳转定位到调用行。
- 顶层全局类型若在首轮分析时未解析（跨文件类/函数），可能先为 `Unknown`；
  后续 reindex 或增量分析可补全。未注册 `UnResolve` 重试，属可接受范围。
- `registerBattleModule(name, module)` 这类**包装函数**：因内部 `registerGlobal(name,...)`
  用的是参数而非字面量，静态不可知；需在配置里直接为包装函数加规则
  （`globalDefineRules` 指向 `registerBattleModule`，`name=0,value=1`）。

