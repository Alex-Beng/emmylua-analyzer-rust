# EmmyLua 配置指南

[English](./emmyrc_json_EN.md)

EmmyLua Analyzer Rust 推荐把配置写在项目根目录的 `.emmyrc.json` 中。

兼容的配置文件：

- `.emmyrc.json`：推荐，功能最完整
- `.luarc.json`：兼容已有 LuaLS 配置
- `.emmyrc.lua`：适合动态生成配置

## 快速开始

把下面这份最小配置放到项目根目录即可：

```json
{
  "$schema": "https://raw.githubusercontent.com/EmmyLuaLs/emmylua-analyzer-rust/refs/heads/main/crates/emmylua_code_analysis/resources/schema.json",
  "runtime": {
    "version": "LuaLatest"
  },
  "workspace": {
    "workspaceRoots": ["./src"]
  }
}
```

## Schema 支持

添加 `$schema` 后，编辑器可以提供：

- 配置项补全
- 字段类型校验
- 枚举值提示
- 悬浮说明

```json
{
  "$schema": "https://raw.githubusercontent.com/EmmyLuaLs/emmylua-analyzer-rust/refs/heads/main/crates/emmylua_code_analysis/resources/schema.json"
}
```

## 路径规则

`workspace` 与 `resource` 中的路径会在加载时自动展开。

| 写法 | 含义 |
| --- | --- |
| `./libs` | 相对于工作区根目录 |
| `libs/runtime` | 也会按工作区相对路径处理 |
| `~/lua` | 相对于用户 Home 目录 |
| `${workspaceFolder}` 或 `{workspaceFolder}` | 工作区根目录 |
| `{env:NAME}` | 环境变量 `NAME` |
| `$NAME` | 环境变量 `NAME` |
| `{luarocks}` | LuaRocks deploy lua 目录 |

示例：

```json
{
  "workspace": {
    "library": [
      "${workspaceFolder}/types",
      "{env:HOME}/.lua",
      "{luarocks}"
    ],
    "workspaceRoots": [
      "./src",
      "./test"
    ]
  }
}
```

## 推荐模板

这份模板适合大多数 Lua 项目：

```json
{
  "$schema": "https://raw.githubusercontent.com/EmmyLuaLs/emmylua-analyzer-rust/refs/heads/main/crates/emmylua_code_analysis/resources/schema.json",
  "completion": {
    "autoRequire": true,
    "callSnippet": false,
    "postfix": "@"
  },
  "diagnostics": {
    "globals": [],
    "disable": ["undefined-global"],
  },
  "doc": {
    "syntax": "md"
  },
  "runtime": {
    "version": "LuaLatest",
    "requirePattern": ["?.lua", "?/init.lua"]
  }
}
```

## 完整配置示例

> 注意：当前格式化配置的顶层键名是 `format`，不是 `reformat`。

<details>
<summary>点击展开完整示例</summary>

```json
{
  "$schema": "https://raw.githubusercontent.com/EmmyLuaLs/emmylua-analyzer-rust/refs/heads/main/crates/emmylua_code_analysis/resources/schema.json",
  "codeAction": {
    "insertSpace": null
  },
  "codeLens": {
    "enable": true
  },
  "completion": {
    "enable": true,
    "autoRequire": true,
    "autoRequireFunction": "require",
    "autoRequireNamingConvention": "keep",
    "autoRequireSeparator": ".",
    "callSnippet": false,
    "postfix": "@",
    "baseFunctionIncludesName": true
  },
  "diagnostics": {
    "enable": true,
    "disable": [],
    "enables": [],
    "globals": [],
    "globalsRegex": [],
    "severity": {},
    "diagnosticInterval": 500
  },
  "doc": {
    "privateName": [],
    "knownTags": [],
    "syntax": "md"
  },
  "documentColor": {
    "enable": true
  },
  "format": {
    "externalTool": null,
    "externalToolRangeFormat": null,
    "useDiff": false
  },
  "hint": {
    "enable": true,
    "paramHint": true,
    "indexHint": true,
    "localHint": true,
    "overrideHint": true,
    "metaCallHint": true,
    "enumParamHint": false
  },
  "hover": {
    "enable": true,
    "customDetail": null
  },
  "inlineValues": {
    "enable": true
  },
  "references": {
    "enable": true,
    "fuzzySearch": true,
    "shortStringSearch": false
  },
  "resource": {
    "paths": []
  },
  "runtime": {
    "version": "LuaLatest",
    "requireLikeFunction": [],
    "frameworkVersions": [],
    "extensions": [],
    "requirePattern": [],
    "nonstandardSymbol": [],
    "special": {},
    "environmentModulePattern": [],
    "globalDefineRules": [],
    "classDefineRules": [],
    "classFieldTypeRules": []
  },
  "semanticTokens": {
    "enable": true,
    "renderDocumentationMarkup": true
  },
  "signature": {
    "detailSignatureHelper": true
  },
  "strict": {
    "requirePath": false,
    "typeCall": false,
    "arrayIndex": true,
    "metaOverrideFileDefine": true,
    "docBaseConstMatchBaseType": true,
    "requireExportGlobal": false
  },
  "workspace": {
    "ignoreDir": [],
    "ignoreGlobs": [],
    "library": [],
    "packageDirs": [],
    "workspaceRoots": [],
    "preloadFileSize": 0,
    "encoding": "utf-8",
    "moduleMap": [],
    "reindexDuration": 5000,
    "enableReindex": false
  }
}
```

</details>

## 顶层分组速览

| 分组 | 用途 | 常用字段 |
| --- | --- | --- |
| `completion` | 补全与自动 require | `autoRequire`、`callSnippet`、`postfix` |
| `diagnostics` | 诊断开关、白名单、级别覆盖 | `disable`、`globals`、`severity` |
| `doc` | 文档注释解析与渲染 | `syntax`、`knownTags`、`privateName` |
| `runtime` | Lua 版本、扩展语法与 require 规则 | `version`、`extensions`、`requirePattern` |
| `workspace` | 工作区目录、库目录、忽略规则 | `library`、`workspaceRoots`、`ignoreGlobs` |
| `strict` | 更严格的类型和可见性约束 | `arrayIndex`、`requireExportGlobal` |
| `format` | 外部格式化工具对接 | `externalTool`、`externalToolRangeFormat` |
| `hint` | 内联提示 | `paramHint`、`localHint`、`enumParamHint` |
| `hover` | 悬浮说明 | `enable`、`customDetail` |
| `references` | 引用查找 | `fuzzySearch`、`shortStringSearch` |

## 配置参考

### completion

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `enable` | `boolean` | `true` | 启用补全 |
| `autoRequire` | `boolean` | `true` | 自动插入跨模块符号需要的 require |
| `autoRequireFunction` | `string` | `"require"` | 自动 require 使用的函数名 |
| `autoRequireNamingConvention` | `string` | `"keep"` | 文件名转换方式：`keep`、`snake-case`、`pascal-case`、`camel-case`、`keep-class` |
| `autoRequireSeparator` | `string` | `"."` | 自动 require 路径分隔符 |
| `callSnippet` | `boolean` | `false` | 补全函数时是否带调用片段 |
| `postfix` | `string` | `"@"` | 后缀补全触发符 |
| `baseFunctionIncludesName` | `boolean` | `true` | 生成函数模板时包含函数名 |

### diagnostics

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `enable` | `boolean` | `true` | 启用诊断 |
| `disable` | `string[]` | `[]` | 禁用的诊断规则 |
| `enables` | `string[]` | `[]` | 额外启用的诊断规则 |
| `globals` | `string[]` | `[]` | 全局变量白名单 |
| `globalsRegex` | `string[]` | `[]` | 全局变量正则白名单 |
| `severity` | `object` | `{}` | 自定义规则级别 |
| `diagnosticInterval` | `number | null` | `500` | 文件变化后触发诊断的延迟，单位毫秒 |

严重程度可选值：`error`、`warning`、`information`、`hint`。

示例：

```json
{
  "diagnostics": {
    "disable": ["undefined-global"],
    "severity": {
      "undefined-global": "warning",
      "unused": "hint"
    },
    "enables": ["unknown-doc-tag"]
  }
}
```

<details>
<summary>查看诊断规则</summary>

默认是 `error`：

- `syntax-error`
- `doc-syntax-error`
- `undefined-global`
- `local-const-reassign`
- `annotation-usage-error`
- `iter-variable-reassign`（Lua 5.5 及以上默认启用）

默认是 `hint`：

- `unreachable-code`
- `unused`
- `deprecated`
- `redefined-local`
- `duplicate-require`
- `preferred-local-alias`

默认关闭：

- `code-style-check`
- `incomplete-signature-doc`
- `missing-global-doc`
- `unknown-doc-tag`
- `non-literal-expressions-in-assert`

其余规则默认级别为 `warning`：

- `type-not-found`
- `missing-return`
- `param-type-mismatch`
- `missing-parameter`
- `redundant-parameter`
- `access-invisible`
- `discard-returns`
- `undefined-field`
- `duplicate-type`
- `redefined-label`
- `need-check-nil`
- `await-in-sync`
- `return-type-mismatch`
- `missing-return-value`
- `redundant-return-value`
- `undefined-doc-param`
- `duplicate-doc-field`
- `missing-fields`
- `inject-field`
- `circle-doc-class`
- `assign-type-mismatch`
- `unbalanced-assignments`
- `unnecessary-assert`
- `unnecessary-if`
- `duplicate-set-field`
- `duplicate-index`
- `generic-constraint-mismatch`
- `cast-type-mismatch`
- `unresolved-require`
- `require-module-not-visible`
- `enum-value-mismatch`
- `read-only`
- `global-in-non-module`
- `attribute-param-type-mismatch`
- `attribute-missing-parameter`
- `attribute-redundant-parameter`
- `invert-if`
- `call-non-callable`

</details>

### doc

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `privateName` | `string[]` | `[]` | 把符合模式的字段视为私有成员，例如 `m_*` |
| `knownTags` | `string[]` | `[]` | 额外识别的文档标签 |
| `syntax` | `string` | `"md"` | 文档语法：`none`、`md`、`myst`、`rst` |
| `rstPrimaryDomain` | `string | null` | `null` | `myst` 或 `rst` 下的主 domain |
| `rstDefaultRole` | `string | null` | `null` | `myst` 或 `rst` 下的默认 role |

### runtime

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `version` | `string` | `"LuaLatest"` | Lua 版本：`Lua5.1`、`LuaJIT`、`Lua5.2`、`Lua5.3`、`Lua5.4`、`Lua5.5`、`LuaLatest` |
| `requireLikeFunction` | `string[]` | `[]` | 视为 require 的函数名 |
| `frameworkVersions` | `string[]` | `[]` | 框架版本标识 |
| `extensions` | `string[]` | `[]` | 额外识别的 Lua 文件扩展名 |
| `requirePattern` | `string[]` | `[]` | require 搜索模式，例如 `?.lua`、`?/init.lua` |
| `nonstandardSymbol` | `string[]` | `[]` | 允许的非标准语法符号 |
| `special` | `object` | `{}` | 特殊函数映射 |
| `environmentModulePattern` | `string[]` | `[]` | 命中这些 glob 且顶层无 `return` 的文件，其顶层全局变量将被合成为模块导出表 |
| `globalDefineRules` | `object[]` | `registerGlobal` 规则 | 运行时注册全局变量的函数规则 |
| `classDefineRules` | `object[]` | `DefineClass`/`DefineComponent` 规则 | 运行时定义类的函数规则 |
| `classFieldTypeRules` | `object[]` | `[]` | 按类名声明常用实例字段的类型 |

`nonstandardSymbol` 支持的值：

- `//`
- `/**/`
- `` ` ``
- `+=`
- `-=`
- `*=`
- `/=`
- `%=`
- `^=`
- `//=`
- `|=`
- `&=`
- `<<=`
- `>>=`
- `||`
- `&&`
- `!`
- `!=`
- `continue`

`special` 支持的值：`none`、`require`、`error`、`assert`、`type`、`setmetatable`。

#### 框架调用规则

许多框架会把 `require`、全局注册或类定义包装成自定义函数。通过
`requireLikeFunction`、`globalDefineRules`、`classDefineRules` 和
`environmentModulePattern`，可以让分析器理解这些约定，而无需修改用户代码。

- `requireLikeFunction`：视为 `require` 的函数名，例如 `import`、`kg_require`。
- `environmentModulePattern`：命中这些 glob 且**顶层没有 `return`** 的文件，
  其所有顶层全局变量都会被合成为模块导出表。这对应"在隔离环境中执行模块并
  返回该环境表"的加载器。
- `globalDefineRules`：运行时注册全局变量的函数，每条规则把位置参数映射到
  角色（`name`、`value`）。
- `classDefineRules`：运行时定义类的函数，每条规则把位置参数映射到角色
  （`name`、`super`）。

`params` 是 `{ "role": <role>, "index": <n> }` 列表，`role` 取 `name`、
`value`、`super`，`index` 为从 0 开始的参数位置。不同项目的参数顺序可能不同。

```json
{
  "runtime": {
    "requireLikeFunction": ["import", "kg_require"],
    "environmentModulePattern": ["Common/battle_core/**"],
    "globalDefineRules": [
      {
        "function": "registerGlobal",
        "params": [
          { "role": "name", "index": 0 },
          { "role": "value", "index": 1 }
        ]
      }
    ],
    "classDefineRules": [
      {
        "function": "DefineClass",
        "params": [
          { "role": "name", "index": 0 },
          { "role": "super", "index": 1 }
        ]
      }
    ]
  }
}
```

默认值：

- `globalDefineRules` 默认一条 `registerGlobal` 规则，`name = 0, value = 1`。
- `classDefineRules` 默认 `DefineClass` / `DefineComponent` 规则，`name = 0, super = 1`。

#### 实例字段类型规则

对于 `DefineClass` 合成的类，`self.field = value` 若 value 来自未标注的构造参数，
字段类型会退化为 `any`。`classFieldTypeRules` 可按**类名**（全局唯一）声明常用字段
的类型，分析器会把它们注入为字段成员并覆盖推断结果。

- `class`：类名（精确匹配）。
- `fields[].name`：字段名（精确匹配）。
- `fields[].type`：类型名，解析为对该类的引用。
- `fields[].optional`：是否可空，默认 `true`（即 `T | nil`）。

```json
{
  "runtime": {
    "classFieldTypeRules": [
      {
        "class": "BattleBt",
        "fields": [
          { "name": "battle", "type": "BattleCore" },
          { "name": "warrior", "type": "BattleWarrior", "optional": false }
        ]
      }
    ]
  }
}
```

##### 运行时导出字段类型（`.emmyrc-fieldtypes.json`）

也可以由运行时（游戏引擎）导出一份字段类型表，放在**工作区根目录**的固定文件
`.emmyrc-fieldtypes.json`，格式为 `{ "类名": { "字段名": "类型名" } }`：

```json
{
  "BattleBt": { "battle": "BattleCore", "warrior": "BattleWarrior" }
}
```

- 该文件在配置预处理（`pre_process_emmyrc`）时读取，字段类型默认**非空**。
- 与手写 `classFieldTypeRules` 冲突时，**手写规则优先**。
- 文件变化会自动触发配置重载与工作区重建（无需重启）。
- 解析失败会被忽略（仅记录日志）。

### workspace

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `ignoreDir` | `string[]` | `[]` | 忽略目录 |
| `ignoreGlobs` | `string[]` | `[]` | 按 glob 忽略文件 |
| `library` | `string[] | object[]` | `[]` | 库目录，支持字符串路径或带忽略规则的对象 |
| `packages` | `string[] | object[]` | `[]` | 包目录，父目录按 library 处理，但只导入指定子目录 |
| `workspaceRoots` | `string[]` | `[]` | 工作区源代码根目录 |
| `preloadFileSize` | `number` | `0` | 预留字段，目前未使用 |
| `encoding` | `string` | `"utf-8"` | 文件编码 |
| `moduleMap` | `object[]` | `[]` | 模块名映射规则 |
| `reindexDuration` | `number` | `5000` | 全量重建索引延迟，单位毫秒 |
| `enableReindex` | `boolean` | `false` | 文件变化后启用全量重建索引 |

`library` 和 `packages` 既可以写路径字符串，也可以写对象：

```json
{
  "workspace": {
    "library": [
      "./types",
      {
        "path": "./vendor",
        "ignoreDir": ["test"],
        "ignoreGlobs": ["**/*.spec.lua"]
      }
    ]
  }
}
```

`moduleMap` 示例：

```json
{
  "workspace": {
    "moduleMap": [
      {
        "pattern": "^lib(.*)$",
        "replace": "script$1"
      }
    ]
  }
}
```

### strict

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `requirePath` | `boolean` | `false` | 强制 require 路径必须命中配置的模块路径 |
| `typeCall` | `boolean` | `false` | 更严格地检查类型调用 |
| `arrayIndex` | `boolean` | `true` | 更严格地检查数组索引 |
| `metaOverrideFileDefine` | `boolean` | `true` | 元定义覆盖文件内定义 |
| `docBaseConstMatchBaseType` | `boolean` | `true` | 允许文档中的基础常量类型与基础类型匹配 |
| `requireExportGlobal` | `boolean` | `false` | 第三方库必须显式使用 `---@export global` 才可导入 |

### format

更多说明请参见 [外部格式化工具选项](../external_format/external_formatter_options_CN.md)。

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `externalTool` | `object | null` | `null` | 整文件格式化时使用的外部工具 |
| `externalToolRangeFormat` | `object | null` | `null` | 选区格式化时使用的外部工具 |
| `useDiff` | `boolean` | `false` | 通过 diff 合并格式化结果 |

外部工具配置对象：

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `program` | `string` | `""` | 可执行文件 |
| `args` | `string[]` | `[]` | 参数列表 |
| `timeout` | `number` | `5000` | 超时时间，单位毫秒 |

### 其他分组

| 分组 | 字段 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `codeAction` | `insertSpace` | `null` | 插入 `@diagnostic disable-next-line` 时覆盖格式化器的 `emmy_doc.space_between_tag_columns` 设置 |
| `codeLens` | `enable` | `true` | 启用 CodeLens |
| `documentColor` | `enable` | `true` | 识别颜色字符串并显示颜色预览 |
| `hint` | `enable` | `true` | 总开关 |
| `hint` | `paramHint` | `true` | 参数名与参数类型提示 |
| `hint` | `indexHint` | `true` | 数组索引命名提示 |
| `hint` | `localHint` | `true` | 局部变量类型提示 |
| `hint` | `overrideHint` | `true` | 覆写方法提示 |
| `hint` | `metaCallHint` | `true` | 元表 `__call` 提示 |
| `hint` | `enumParamHint` | `false` | 枚举字面量提示 |
| `hover` | `enable` | `true` | 启用悬浮说明 |
| `hover` | `customDetail` | `null` | 自定义悬浮细节等级，通常为 `1` 到 `255` |
| `inlineValues` | `enable` | `true` | 调试时显示内联值 |
| `references` | `enable` | `true` | 启用引用查找 |
| `references` | `fuzzySearch` | `true` | 常规查找失败后尝试模糊查找 |
| `references` | `shortStringSearch` | `false` | 同时在短字符串中查找引用 |
| `resource` | `paths` | `[]` | 资源根目录，用于路径补全与跳转 |
| `semanticTokens` | `enable` | `true` | 启用语义高亮 |
| `semanticTokens` | `renderDocumentationMarkup` | `true` | 渲染文档标记，需要配合 `doc.syntax` |
| `signature` | `detailSignatureHelper` | `true` | 显示详细签名帮助 |

## 建议

- 新项目优先使用 `.emmyrc.json`
- 有现成 LuaLS 项目时，可先沿用 `.luarc.json`
- `workspace.library` 与 `workspace.workspaceRoots` 建议尽早配置，否则跳转、补全和诊断的结果会比较分散
- 若项目依赖第三方库可见性约束，可考虑启用 `strict.requireExportGlobal`

[返回顶部](#emmylua-配置指南)
