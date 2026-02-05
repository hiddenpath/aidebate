use ai_lib_rust::Message;

use crate::types::{DebatePhase, Position};

pub fn build_side_prompt(
    side: Position,
    phase: DebatePhase,
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> Vec<Message> {
    let stance = match side {
        Position::Pro => "你是正方，支持该议题。",
        Position::Con => "你是反方，反对该议题。",
        Position::Judge => "",
    };
    let mut history = String::new();
    for (pos, ph, content, provider) in transcript {
        history.push_str(&format!(
            "[{} - {} - {}]\n{}\n\n",
            pos.label(),
            ph.title(),
            provider,
            content
        ));
    }

    let phase_goal = match phase {
        DebatePhase::Opening => "开篇陈词：阐述立场与核心论点。",
        DebatePhase::Rebuttal => "反驳：针对对方论点逐条反驳，并补充论据。",
        DebatePhase::Defense => "防守：回应对方反驳，巩固自身论据。",
        DebatePhase::Closing => "总结陈词：总结关键论点，强调结论。",
        DebatePhase::Judgement => "",
    };

    let system = format!(
        "{stance}\n议题：{topic}\n当前阶段：{phase_goal}\n要求：\n- 用 Markdown 输出。\n- 必须包含 `## Reasoning`（推理过程，精简列点）和 `## Final Position`（本轮结论）。\n- 语言简洁有力，避免重复。\n- 字数建议 120-220 中文字。\n"
    );

    let mut messages = vec![Message::system(system)];
    if !history.is_empty() {
        messages.push(Message::user(format!("已进行的辩论记录：\n{}", history)));
    }
    messages.push(Message::user(format!(
        "请完成本轮 `{}` 发言。",
        phase.title()
    )));
    messages
}

pub fn build_judge_prompt(
    topic: &str,
    transcript: &[(Position, DebatePhase, String, String)],
) -> Vec<Message> {
    let mut history = String::new();
    for (pos, ph, content, provider) in transcript {
        history.push_str(&format!(
            "[{} - {} - {}]\n{}\n\n",
            pos.label(),
            ph.title(),
            provider,
            content
        ));
    }
    let system = format!(
        "你是中立裁判，请根据完整辩论记录做出裁决。\n议题：{topic}\n要求：\n- 用 Markdown 输出。\n- 必须包含 `## Reasoning`（裁判推理过程，条理清晰）和 `## Verdict`（结论）。\n- 在结论中用 `Winner: Pro` 或 `Winner: Con` 指明胜方。\n- 简洁客观，避免复读。\n"
    );
    vec![
        Message::system(system),
        Message::user(format!("完整辩论记录：\n{}", history)),
    ]
}
