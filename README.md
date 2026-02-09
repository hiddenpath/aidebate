# AI Debate v0.2.0

**Multi-model AI debate arena built on [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) and [ai-protocol](https://github.com/hiddenpath/ai-protocol).**

Three AI models engage in a structured debate: Pro and Con present arguments across four rounds, then a Judge delivers the verdict.

## Features

- **4-Round Debate Flow**: Opening → Rebuttal → Defense → Closing → Judgement
- **Dynamic Model Selection**: Choose any available model for each role via the UI
- **Auto Provider Detection**: Automatically detects configured API keys and shows available models
- **Multi-Provider Support**: DeepSeek, Zhipu GLM, Groq, Mistral, OpenAI, Anthropic, MiniMax
- **Automatic Fallback**: Primary model failures trigger automatic switch to backup model
- **Real-time Streaming**: All rounds (including Judge) use true SSE streaming
- **Token Usage Tracking**: Per-round token consumption display
- **Reasoning Display**: Collapsible thinking/reasoning blocks when supported by model
- **Debate History**: SQLite database for persistent debate records
- **Modern UI**: Dark theme, responsive layout, real Markdown rendering

## Architecture

### Backend
- **Framework**: Axum (async web framework)
- **AI Integration**: [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) v0.6.6
- **Protocol**: [ai-protocol](https://github.com/hiddenpath/ai-protocol)
- **Database**: SQLx + SQLite
- **Streaming**: Server-Sent Events (SSE)

### Frontend
- **Markdown**: [Marked.js](https://marked.js.org/) (CDN)
- **Style**: Modern dark theme with responsive layout
- **Real-time**: SSE client with streaming updates

## Default Model Configuration

| Role | Default Model | Fallback |
|------|---------------|----------|
| Pro | `deepseek/deepseek-chat` | `mistral/mistral-small-latest` |
| Con | `zhipu/glm-4-plus` | `mistral/mistral-small-latest` |
| Judge | `groq/llama-3.3-70b-versatile` | `mistral/mistral-small-latest` |

Users can override these selections in the UI before starting a debate.

## Quick Start

### 1. Configure API Keys

```bash
cp .env.example .env
# Edit .env and add your API keys (at least one provider required)
```

### 2. Build and Run

```bash
cargo run
```

### 3. Open in Browser

Navigate to `http://127.0.0.1:3000`

## Environment Configuration

See [.env.example](.env.example) for all available options. Key variables:

```bash
# Required: At least one AI provider API key
DEEPSEEK_API_KEY=sk-your-key
ZHIPU_API_KEY=your-key
GROQ_API_KEY=gsk_your-key
MISTRAL_API_KEY=your-key    # recommended as fallback

# Optional: Additional providers
OPENAI_API_KEY=sk-your-key
ANTHROPIC_API_KEY=sk-ant-your-key

# Optional: Override default models
PRO_MODEL_ID=deepseek/deepseek-chat
CON_MODEL_ID=zhipu/glm-4-plus
JUDGE_MODEL_ID=groq/llama-3.3-70b-versatile
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Main page |
| GET | `/health` | Health check with model configuration |
| GET | `/api/models` | Available providers and models (for UI) |
| POST | `/debate/stream` | Start a debate, returns SSE stream |
| GET | `/history` | Fetch debate history |

## Debate Flow

1. **User enters topic** and optionally selects models for each role
2. **System initializes AI clients** (Pro, Con, Judge) with fallback support
3. **4 debate rounds** (each round: Pro speaks → Con speaks):
   - Opening Statement
   - Rebuttal
   - Defense
   - Closing Statement
4. **Judge delivers verdict** based on the complete debate transcript

## ai-lib-rust Features Used

- **Unified Client Interface**: `AiClient::new("provider/model")`
- **Automatic Fallback**: `AiClientBuilder::with_fallbacks()`
- **Streaming**: `execute_stream()` returns `StreamingEvent` stream
- **Token Usage**: `StreamingEvent::Metadata { usage }` for token tracking
- **Error Classification**: Auth errors trigger automatic fallback
- **Protocol-Driven**: All behavior defined by ai-protocol manifests

## Related Projects

- [ai-lib-rust](https://github.com/hiddenpath/ai-lib-rust) - Protocol Runtime for AI-Protocol
- [ai-lib-python](https://github.com/hiddenpath/ai-lib-python) - Python Runtime for AI-Protocol
- [ai-protocol](https://github.com/hiddenpath/ai-protocol) - Provider-agnostic AI specification
- [aidebate-python](https://github.com/hiddenpath/aidebate-python) - Python port of this project

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
