//! Prompts: rewriting the text in a query editor, and the conversational assistant.

use based_core::EngineKind;

use crate::tools::ToolSpec;
use crate::types::{ChatRequest, Message};

fn dialect(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "PostgreSQL SQL",
        EngineKind::SQLite => "SQLite SQL",
        EngineKind::MongoDB => "a MongoDB aggregation pipeline written as a JSON array",
        EngineKind::Redis => "a single Redis command",
    }
}

fn system(engine: EngineKind, schema: &str) -> String {
    let mut s = format!(
        "You edit the text in a database query editor. The editor holds {}.\n\
         Reply with the complete new editor contents only, inside one fenced code block. \
         No explanation. Keep what the user did not ask to change. \
         Never invent tables or columns that are not in the schema.",
        dialect(engine)
    );
    if !schema.is_empty() {
        s.push_str("\n\nSchema:\n");
        s.push_str(schema);
    }
    s
}

/// Request asking the model to apply `instruction` to `current`.
pub fn edit_request(
    model: &str,
    engine: EngineKind,
    schema: &str,
    current: &str,
    instruction: &str,
) -> ChatRequest {
    let user = if current.trim().is_empty() {
        format!("The editor is empty.\n\nWrite: {instruction}")
    } else {
        format!("Current editor contents:\n```\n{current}\n```\n\nChange: {instruction}")
    };
    ChatRequest::new(model, vec![Message::user(user)]).with_system(system(engine, schema))
}

fn chat_system(engine: Option<EngineKind>, label: &str, tools: bool) -> String {
    let target = engine.map_or_else(String::new, |e| {
        format!(" The user is working with {} ({label}).", dialect(e))
    });
    let lookup = if tools {
        " Use the tools to look up tables and columns before writing a query."
    } else {
        ""
    };
    format!(
        "You are a database assistant inside the Based desktop client.{target}{lookup} \
         Be concise. Put any query in a fenced code block. \
         Never invent tables or columns; say so when you do not know the schema."
    )
}

/// Request continuing a conversation, optionally scoped to a connection and offering `tools`.
pub fn chat_request(
    model: &str,
    engine: Option<EngineKind>,
    label: &str,
    history: Vec<Message>,
    tools: Vec<ToolSpec>,
) -> ChatRequest {
    ChatRequest::new(model, history)
        .with_system(chat_system(engine, label, !tools.is_empty()))
        .with_tools(tools)
}

/// Pulls the code out of a reply: the first fenced block, or the whole text if unfenced.
/// An unterminated fence (a cut-off stream) yields what was received.
pub fn extract_code(reply: &str) -> String {
    let Some(start) = reply.find("```") else {
        return reply.trim().to_owned();
    };
    let rest = &reply[start + 3..];
    let body = rest.split_once('\n').map_or("", |(_, body)| body);
    let code = body.split("```").next().unwrap_or(body);
    code.trim_end().to_owned()
}

/// Gives `code` the same trailing-newline state as `original`, so a diff doesn't flag the
/// model's habit of dropping (or adding) the final newline as a change.
pub fn match_final_newline(original: &str, mut code: String) -> String {
    let wants = original.ends_with('\n');
    if wants && !code.ends_with('\n') {
        code.push('\n');
    } else if !wants && code.ends_with('\n') {
        code.truncate(code.trim_end_matches('\n').len());
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_system_names_the_connection() {
        let scoped = chat_system(Some(EngineKind::Postgres), "prod", false);
        assert!(scoped.contains("PostgreSQL") && scoped.contains("prod"));
        assert!(!scoped.contains("tools"));
        assert!(chat_system(Some(EngineKind::Postgres), "prod", true).contains("tools"));
        assert!(!chat_system(None, "", false).contains("working with"));
    }

    #[test]
    fn final_newline_follows_the_original() {
        assert_eq!(match_final_newline("a\n", "b".into()), "b\n");
        assert_eq!(match_final_newline("a", "b\n".into()), "b");
        assert_eq!(match_final_newline("a\n", "b\n".into()), "b\n");
        assert_eq!(match_final_newline("", "b".into()), "b");
    }

    #[test]
    fn extracts_fenced_block_ignoring_chatter() {
        let r = "Sure:\n```sql\nSELECT 1;\n```\nHope that helps";
        assert_eq!(extract_code(r), "SELECT 1;");
    }

    #[test]
    fn unfenced_reply_is_trimmed() {
        assert_eq!(extract_code("  SELECT 1  \n"), "SELECT 1");
    }

    #[test]
    fn partial_stream_yields_partial_code() {
        assert_eq!(extract_code("```sql\nSELECT a,\n  b"), "SELECT a,\n  b");
        assert_eq!(extract_code("```sql"), "");
        assert_eq!(extract_code("```"), "");
    }

    #[test]
    fn keeps_inner_indentation_and_blank_lines() {
        let r = "```\nSELECT 1\n\n  FROM t\n```";
        assert_eq!(extract_code(r), "SELECT 1\n\n  FROM t");
    }

    #[test]
    fn request_embeds_schema_and_current_text() {
        let req = edit_request(
            "m",
            EngineKind::Postgres,
            "users(id)",
            "SELECT 1",
            "add limit",
        );
        let system = req.system.unwrap();
        assert!(system.contains("PostgreSQL") && system.contains("users(id)"));
        assert!(req.messages[0].content.contains("SELECT 1"));
        assert!(req.messages[0].content.contains("add limit"));
    }

    #[test]
    fn empty_editor_asks_for_new_text_and_omits_empty_schema() {
        let req = edit_request("m", EngineKind::SQLite, "", "  ", "count users");
        assert!(!req.system.unwrap().contains("Schema:"));
        assert!(req.messages[0].content.starts_with("The editor is empty"));
    }
}
