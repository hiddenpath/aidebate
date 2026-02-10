# AI Debate v0.3.0

**基于 [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) 和 [ai-protocol](https://github.com/hiddenpath/ai-protocol) 构建的多模型 AI 辩论竞技场。**

[English](README.md)

三个 AI 模型参与结构化辩论：正方和反方在四轮辩论中展示论点，然后由裁判做出裁决。辩手可以选择性地搜索网络以获取支持论点的证据。

## 功能特性

- **四轮辩论流程**：一辩开篇 → 二辩反驳 → 三辩防守 → 总结陈词 → 裁判裁决
- **网络搜索工具调用**：辩手可通过 Tavily API 搜索网络获取证据（可选）
- **动态模型选择**：通过 UI 为每个角色选择任意可用模型
- **自动供应商检测**：自动检测已配置的 API Key 并显示可用模型
- **多供应商支持**：DeepSeek、智谱 GLM、Groq、Mistral、OpenAI、Anthropic、MiniMax
- **自动故障转移**：主模型失败时自动切换到备用模型
- **实时流式传输**：所有轮次（包括裁判）均使用真正的 SSE 流式传输
- **Token 用量追踪**：每轮显示 Token 消耗量
- **推理过程展示**：支持折叠显示模型的思考/推理过程
- **辩论历史记录**：SQLite 数据库持久化存储辩论记录
- **现代化 UI**：暗色主题、响应式布局、Markdown 实时渲染

## 架构

### 后端
- **框架**：Axum（异步 Web 框架）
- **AI 集成**：[ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) v0.6.6
- **协议**：[ai-protocol](https://github.com/hiddenpath/ai-protocol)
- **数据库**：SQLx + SQLite
- **流式传输**：Server-Sent Events (SSE)
- **工具调用**：通过 Tavily API 实现函数调用与网络搜索

### 前端
- **Markdown**：[Marked.js](https://marked.js.org/)（CDN）
- **样式**：现代暗色主题，响应式布局
- **实时更新**：SSE 客户端，流式更新
- **搜索展示**：可视化搜索卡片，展示查询内容和来源

## 快速开始

### 1. 配置 API Key

```bash
cp .env.example .env
# 编辑 .env 文件并添加你的 API Key（至少需要一个供应商）
# 可选：添加 TAVILY_API_KEY 以启用网络搜索工具调用
```

### 2. 构建并运行

```bash
cargo run
```

### 3. 在浏览器中打开

访问 `http://127.0.0.1:3000`

## API Key 配置说明

API Key 通过 `.env` 文件加载（使用 `dotenv` crate）。启动时，系统会扫描所有已知的供应商密钥，并自动在 UI 中提供相应的模型。

| 环境变量 | 供应商 | 角色 | 说明 |
|---------|--------|------|------|
| `DEEPSEEK_API_KEY` | DeepSeek | 默认正方模型 | `sk-your-key` |
| `ZHIPU_API_KEY` | 智谱 / GLM | 默认反方模型 | `your-key` |
| `GROQ_API_KEY` | Groq | 默认裁判模型 | `gsk_your-key`，有慷慨的免费额度 |
| `MISTRAL_API_KEY` | Mistral | 通用备用模型 | `your-key`，推荐配置 |
| `OPENAI_API_KEY` | OpenAI | 可选 | `sk-your-key` |
| `ANTHROPIC_API_KEY` | Anthropic | 可选 | `sk-ant-your-key` |
| `MINIMAX_API_KEY` | MiniMax | 可选 | `your-key` |
| `TAVILY_API_KEY` | Tavily | 网络搜索（可选） | `tvly-your-key`，在 [tavily.com](https://tavily.com) 获取 |

**工作原理：**
1. 将 `.env.example` 复制为 `.env` 并填入你的密钥
2. 至少需要**一个** AI 供应商密钥；推荐配置 `MISTRAL_API_KEY` 作为可靠的备用方案
3. 系统自动检测哪些密钥存在，并仅在 `/api/models` 接口和 UI 下拉菜单中暴露这些供应商
4. 如果主模型失败（如认证错误），系统自动回退到 `mistral/mistral-small-latest`
5. 用户可通过 UI 或环境变量覆盖各角色的模型：
   - `PRO_MODEL_ID` — 例如 `deepseek/deepseek-chat`
   - `CON_MODEL_ID` — 例如 `zhipu/glm-4-plus`
   - `JUDGE_MODEL_ID` — 例如 `groq/llama-3.3-70b-versatile`

## 数据库

项目使用 **SQLite** 持久化存储辩论历史。无需外部数据库服务器。

- **默认路径**：`debate.db`（在工作目录中创建）
- **自定义路径**：在 `.env` 中设置 `DATABASE_URL`（例如 `DATABASE_URL=sqlite://path/to/debate.db`）
- **自动创建**：数据库文件和表在首次运行时自动创建
- **无需迁移**：启动时使用 `CREATE TABLE IF NOT EXISTS`

**数据库表结构**（`debate_messages` 表）：

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER | 自增主键 |
| `user_id` | TEXT | 用户标识 |
| `session_id` | TEXT | 辩论会话标识 |
| `role` | TEXT | `pro`、`con` 或 `judge` |
| `phase` | TEXT | `opening`、`rebuttal`、`defense`、`closing` 或 `judgement` |
| `provider` | TEXT | 使用的模型 ID（例如 `deepseek/deepseek-chat`） |
| `content` | TEXT | 辩论消息内容 |
| `created_at` | TIMESTAMP | 自动生成的时间戳 |

## 环境配置

完整选项请参见 [.env.example](.env.example)：

```bash
# 必须：至少一个 AI 供应商的 API Key
DEEPSEEK_API_KEY=sk-your-key
ZHIPU_API_KEY=your-key
GROQ_API_KEY=gsk_your-key
MISTRAL_API_KEY=your-key        # 推荐作为备用

# 可选：其他供应商
OPENAI_API_KEY=sk-your-key
ANTHROPIC_API_KEY=sk-ant-your-key
MINIMAX_API_KEY=your-key

# 可选：网络搜索，获取有证据支持的辩论
TAVILY_API_KEY=tvly-your-key

# 可选：覆盖默认模型
PRO_MODEL_ID=deepseek/deepseek-chat
CON_MODEL_ID=zhipu/glm-4-plus
JUDGE_MODEL_ID=groq/llama-3.3-70b-versatile

# 可选：数据库路径（默认：sqlite://debate.db）
DATABASE_URL=sqlite://debate.db

# 可选：网络代理
AI_PROXY_URL=http://127.0.0.1:7890

# 可选：本地 ai-protocol 目录（未设置时自动检测）
AI_PROTOCOL_DIR=../ai-protocol

# 可选：启用弹性功能（熔断器、并发限制）
AI_DEBATE_RESILIENCE=true
```

## 工具调用（网络搜索）

设置 `TAVILY_API_KEY` 后，辩手（正方和反方）可以调用 `web_search` 工具查找证据：

1. 模型接收辩论上下文和 `web_search` 工具定义
2. 如果模型认为证据有助于论证，它会使用查询词调用 `web_search`
3. 系统通过 Tavily API 执行搜索并将结果返回给模型
4. 模型结合搜索结果生成论述
5. 搜索活动在 UI 中以查询内容和来源的形式展示

**注意**：裁判不使用工具——它仅基于辩论记录进行客观评判。

## 配置说明（新增）

- **每轮最大 token 配额**：可以通过环境变量调整：
   - `PRO_MAX_TOKENS`：正方单轮最大 tokens，默认 `2048`。
   - `CON_MAX_TOKENS`：反方单轮最大 tokens，默认 `2048`。
   - `JUDGE_MAX_TOKENS`：裁判单轮最大 tokens，默认 `3072`（建议增大以容纳完整裁决上下文）。
- **历史上下文截断**：为避免上下文过长，系统会在构建 prompt 时根据 token 预算自动截断历史记录。保留的最近条数由 token 用量决定。
- **保留 tokens**：为系统消息和回复内容保留 token 预算（默认 512）。可通过以下环境变量调整：
   - `PRO_RESERVED_TOKENS`
   - `CON_RESERVED_TOKENS`
   - `JUDGE_RESERVED_TOKENS`
   - `TRANSCRIPT_RESERVED_TOKENS`（通用默认值）

如果未设置 `TAVILY_API_KEY`，系统将照常工作（无工具调用，无行为变化）。

## 默认模型配置

| 角色 | 默认模型 | 备用模型 |
|------|---------|---------|
| 正方 | `deepseek/deepseek-chat` | `mistral/mistral-small-latest` |
| 反方 | `zhipu/glm-4-plus` | `mistral/mistral-small-latest` |
| 裁判 | `groq/llama-3.3-70b-versatile` | `mistral/mistral-small-latest` |

用户可以在开始辩论前通过 UI 覆盖这些选择。

## API 接口

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/` | 主页面 |
| GET | `/health` | 健康检查，返回模型配置信息 |
| GET | `/api/models` | 可用的供应商、模型和功能标志 |
| POST | `/debate/stream` | 发起辩论，返回 SSE 流 |
| GET | `/history` | 获取辩论历史记录 |

## SSE 事件类型

| 类型 | 说明 |
|------|------|
| `phase` | 辩论初始化，包含模型信息 |
| `phase_start` | 辩论轮次开始 |
| `delta` | 流式内容片段 |
| `thinking` | 模型推理/思考内容 |
| `usage` | Token 用量元数据 |
| `search` | 执行了网络搜索（查询 + 结果） |
| `phase_done` | 辩论轮次完成 |
| `error` | 发生错误 |
| `done` | 辩论结束 |

## 辩论流程

1. **用户输入辩题**，可选择为各角色指定模型
2. **系统初始化 AI 客户端**（正方、反方、裁判），支持故障转移
3. **四轮辩论**（每轮：正方发言 → 反方发言）：
   - 一辩开篇
   - 二辩反驳
   - 三辩防守
   - 总结陈词
   - *（如果启用了网络搜索，模型可在任何轮次中搜索证据）*
4. **裁判做出裁决**，基于完整的辩论记录

## 项目结构

```
aidebate/
├── src/
│   ├── main.rs              # 入口点，服务器初始化
│   ├── config.rs            # 供应商检测和 AI 客户端管理
│   ├── handlers.rs          # HTTP 路由处理器（Axum）
│   ├── storage.rs           # SQLite 数据库操作
│   ├── types.rs             # 核心数据结构和枚举
│   ├── prompts.rs           # 辩论角色的提示词模板
│   ├── tools.rs             # 网络搜索工具（Tavily API）
│   ├── app_metrics.rs       # 指标和计时基础设施
│   └── debate/
│       ├── mod.rs           # 辩论模块导出
│       └── engine.rs        # 辩论执行引擎，支持流式传输
├── static/
│   ├── index.html           # 单页 Web UI
│   └── js/
│       └── marked.min.js    # Markdown 渲染库
├── .env.example             # 环境变量模板
├── Cargo.toml               # Rust 项目清单
└── README.md
```

## ai-lib-rust 使用的功能

- **统一客户端接口**：`AiClient::new("provider/model")`
- **自动故障转移**：`AiClientBuilder::with_fallbacks()`
- **流式传输**：`execute_stream()` 返回 `StreamingEvent` 流
- **工具调用**：`tools(Vec<ToolDefinition>)` + `execute()` 实现函数调用
- **Token 用量**：`StreamingEvent::Metadata { usage }` 追踪 Token 消耗
- **错误分类**：认证错误触发自动故障转移
- **协议驱动**：所有行为由 ai-protocol 清单定义

## 相关项目

- [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) - AI-Protocol 的 Rust 运行时
- [ai-lib-python](https://github.com/hiddenpath/ai-lib-python) - AI-Protocol 的 Python 运行时
- [ai-protocol](https://github.com/hiddenpath/ai-protocol) - 与供应商无关的 AI 规范
- [aidebate-python](https://github.com/hiddenpath/aidebate-python) - 本项目的 Python 版本

## 许可证

本项目采用以下任一许可证：

- Apache 许可证 2.0 版（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT 许可证（[LICENSE-MIT](LICENSE-MIT)）

由你选择。
