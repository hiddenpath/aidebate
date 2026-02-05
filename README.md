# AI Debate v0.1.0

**基于 [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) 构建的 AI 辩论系统**

三方 AI 大模型进行结构化辩论：正方、反方各陈述观点，最后由裁判进行裁决。

## 功能特性

- **4 轮辩论流程**：开篇陈词 → 反驳 → 防守 → 总结陈词
- **多 Provider 支持**：DeepSeek (正方) / 智谱 GLM (反方) / Groq (裁判)
- **自动 Fallback**：主模型失败时自动切换到 Mistral 备用模型
- **实时流式响应**：支持推理过程的实时展示
- **结构化输出**：Markdown 格式，包含 Reasoning 和 Final Position
- **历史记录**：SQLite 数据库存储辩论历史
- **现代化 UI**：响应式前端，支持中断和重新开始

## 技术架构

### 后端
- **框架**: Axum (异步 web 框架)
- **AI 集成**: [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) v0.6.5
- **协议规范**: [ai-protocol](https://github.com/hiddenpath/ai-protocol)
- **数据库**: SQLx + SQLite
- **流式处理**: Server-Sent Events (SSE)

### 前端
- **渲染**: Marked.js (Markdown)
- **样式**: 现代化暗色主题
- **实时更新**: SSE 客户端

## 模型配置

| 角色 | 主模型 | 备用模型 |
|------|--------|----------|
| 正方 | `deepseek/deepseek-chat` | `mistral/mistral-small-latest` |
| 反方 | `zhipu/glm-4-plus` | `mistral/mistral-small-latest` |
| 裁判 | `groq/llama-3.3-70b-versatile` | `mistral/mistral-small-latest` |

## 环境配置

```bash
# 必需的 API Keys
export DEEPSEEK_API_KEY="your_deepseek_key"
export ZHIPU_API_KEY="your_zhipu_key"
export GROQ_API_KEY="your_groq_key"
export MISTRAL_API_KEY="your_mistral_key"  # fallback

# 可选配置
export AI_PROTOCOL_DIR="/path/to/ai-protocol"  # 本地协议目录
export AI_PROXY_URL="http://127.0.0.1:7890"    # 代理设置

# 自定义模型 (可选，覆盖默认配置)
export PRO_MODEL_ID="deepseek/deepseek-chat"
export CON_MODEL_ID="zhipu/glm-4-plus"
export JUDGE_MODEL_ID="groq/llama-3.3-70b-versatile"
```

## 运行方式

```bash
cargo run
```

访问 `http://127.0.0.1:3000`

## API 接口

| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/` | 主页面 |
| GET | `/health` | 健康检查，返回模型配置 |
| POST | `/debate/stream` | 发起辩论，返回 SSE 流 |
| GET | `/history` | 获取辩论历史 |

## 辩论流程

1. **用户输入议题** (自然语言描述)
2. **系统初始化 3 个 AI Client** (正方、反方、裁判)
3. **4 轮辩论**:
   - 正方开篇陈词 → 反方开篇陈词
   - 正方反驳 → 反方反驳
   - 正方防守 → 反方防守
   - 正方总结 → 反方总结
4. **裁判裁决**: 基于完整辩论记录进行推理和裁决

## ai-lib-rust 特性利用

- **统一 Client 接口**: `AiClient::new("provider/model")` 
- **自动 Fallback**: `AiClientBuilder::with_fallbacks()`
- **流式响应**: `execute_stream()` 返回 `StreamingEvent`
- **错误分类**: 401 认证错误自动触发 fallback
- **协议驱动**: 所有行为由 ai-protocol manifest 定义

## 依赖

```toml
[dependencies]
ai-lib-rust = { git = "https://github.com/hiddenpath/ai-lib-rust", tag = "v0.6.5" }
```

## 相关项目

- [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) - Protocol Runtime for AI-Protocol
- [ai-protocol](https://github.com/hiddenpath/ai-protocol) - Provider-agnostic AI specification

## 许可证

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
