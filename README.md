# AI Debate - ai-lib 示例项目

基于 ai-lib 构建的 AI 辩论系统，支持两个 AI 进行结构化辩论，最后由第三方 AI 进行裁判裁决。

## 功能特性

- **4 轮辩论流程**：开篇陈词 → 反驳 → 防守 → 总结陈词
- **多 Provider 支持**：自动选择 Groq/Mistral/DeepSeek/Gemini 中的任意三家作为正方、反方、裁判
- **实时流式响应**：支持推理过程的实时展示
- **结构化输出**：要求 AI 使用 Markdown 格式输出，包含 Reasoning 和 Final Position 部分
- **历史记录**：SQLite 数据库存储辩论历史
- **现代化 UI**：响应式前端，支持中断和重新开始

## 技术架构

### 后端 (Axum + ai-lib)
- **框架**: Axum (异步 web 框架)
- **AI 集成**: ai-lib (统一的 AI provider 接口)
- **数据库**: SQLx + SQLite (辩论历史存储)
- **流式处理**: Server-Sent Events (SSE)
- **错误处理**: anyhow (简化的错误处理)

### 前端 (纯 JavaScript)
- **渲染**: Marked.js (Markdown 渲染)
- **样式**: 现代化暗色主题
- **实时更新**: SSE 客户端

## 环境配置

```bash
# 至少设置三个 API Key (任意组合)
export GROQ_API_KEY="your_groq_key"
export MISTRAL_API_KEY="your_mistral_key"
export DEEPSEEK_API_KEY="your_deepseek_key"
export GOOGLE_API_KEY="your_gemini_key"  # 可选

# 可选配置
export PROXY_URL="http://127.0.0.1:7890"  # 代理设置
export DATABASE_URL="sqlite:///path/to/aidebate.db"  # 数据库路径
```

## 运行方式

```bash
cargo run
```

访问 `http://127.0.0.1:3000`

## API 接口

- `GET /` - 主页面
- `GET /health` - 健康检查，返回活跃的 providers
- `POST /debate/stream` - 发起辩论，返回 SSE 流
- `GET/POST /history` - 获取辩论历史

## 辩论流程

1. **用户输入议题** (自然语言描述)
2. **系统自动选择 3 个 Provider** (正方、反方、裁判)
3. **4 轮辩论**:
   - 正方开篇陈词
   - 反方开篇陈词
   - 正方反驳 + 反方反驳
   - 正方防守 + 反方防守
   - 正方总结 + 反方总结
4. **裁判裁决**: 基于完整辩论记录进行推理和裁决

## AI 提示词设计

### 辩论方提示词
```
你是[正方/反方]，[支持/反对]该议题。
议题：[用户输入]
当前阶段：[开篇陈词/反驳/防守/总结陈词]

要求：
- 用 Markdown 输出
- 必须包含 ## Reasoning（推理过程）和 ## Final Position（本轮结论）
- 语言简洁有力，避免重复
- 字数建议 120-220 中文字
```

### 裁判提示词
```
你是中立裁判，请根据完整辩论记录做出裁决。
议题：[用户输入]

要求：
- 用 Markdown 输出
- 必须包含 ## Reasoning（推理过程）和 ## Verdict（结论）
- 在结论中用 Winner: Pro 或 Winner: Con 指明胜方
- 简洁客观，避免复读
```

## ai-lib 特性利用

- **统一接口**: 所有 provider 使用相同的 `ChatCompletionRequest`
- **自动重试**: 配置 failover 链路提高可靠性
- **推理模型**: 优先使用 qwen-qwq-32b 等推理模型
- **流式响应**: 实时展示 AI 推理过程
- **Metrics 集成**: 性能监控和统计

## ai-lib 库缺陷识别

1. **缺少辩论专用 API**: 没有内置的对话/辩论管理功能
2. **有限的上下文管理**: 需要手动构建消息历史
3. **缺少结构化输出解析**: 无法自动解析 AI 的结构化响应
4. **Token 使用统计不完善**: 流式响应中缺少准确的 token 计数

## 许可证

MIT License
