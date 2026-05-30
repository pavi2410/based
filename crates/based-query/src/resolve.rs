//! Postman-style `{{$fn}}` and `{{name}}` variable resolution (sandboxed).

use anyhow::Result;
use rand::Rng;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::variables::Variables;

#[derive(Debug, Clone, Default)]
pub struct VariableContext {
    pub session: Variables,
    pub query: Variables,
    pub collection: Variables,
    /// Active environment variables. `None` means "No Environment".
    pub environment: Option<Variables>,
    pub workspace: Variables,
    /// Backward-compatibility scope for existing callers.
    pub connection: Variables,
}

#[derive(Debug, Clone)]
pub enum ResolveError {
    MissingVariable(String),
    InvalidRandomInt(String),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingVariable(name) => write!(f, "Missing variable: {{{{{name}}}}}"),
            Self::InvalidRandomInt(msg) => write!(f, "Invalid {{$randomInt}}: {msg}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolve all `{{…}}` tokens in `query`. Does not mutate the source string.
pub fn resolve_query(query: &str, ctx: &VariableContext) -> Result<String, ResolveError> {
    let mut out = String::with_capacity(query.len());
    let mut rest = query;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            return Ok(out);
        };
        let token = after[..end].trim();
        let value = eval_token(token, ctx)?;
        out.push_str(&value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn eval_token(token: &str, ctx: &VariableContext) -> Result<String, ResolveError> {
    if let Some(builtin) = token.strip_prefix('$') {
        return eval_builtin(builtin);
    }
    if let Some(v) = ctx.session.get(token) {
        return Ok(v.clone());
    }
    if let Some(v) = ctx.query.get(token) {
        return Ok(v.clone());
    }
    if let Some(v) = ctx.collection.get(token) {
        return Ok(v.clone());
    }
    if let Some(v) = ctx.environment.as_ref().and_then(|vars| vars.get(token)) {
        return Ok(v.clone());
    }
    if let Some(v) = ctx.workspace.get(token) {
        return Ok(v.clone());
    }
    if let Some(v) = ctx.connection.get(token) {
        return Ok(v.clone());
    }
    Err(ResolveError::MissingVariable(token.to_string()))
}

fn eval_builtin(name: &str) -> Result<String, ResolveError> {
    if name == "randomUUID" {
        return Ok(Uuid::new_v4().to_string());
    }
    if name == "timestamp" {
        return Ok(OffsetDateTime::now_utc().unix_timestamp().to_string());
    }
    if name == "isoTimestamp" {
        return OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|e| ResolveError::InvalidRandomInt(e.to_string()));
    }
    if let Some(args) = name
        .strip_prefix("randomInt(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let (min_s, max_s) = args
            .split_once(',')
            .ok_or_else(|| ResolveError::InvalidRandomInt("expected min,max".into()))?;
        let min: i64 = min_s
            .trim()
            .parse()
            .map_err(|_| ResolveError::InvalidRandomInt("min not integer".into()))?;
        let max: i64 = max_s
            .trim()
            .parse()
            .map_err(|_| ResolveError::InvalidRandomInt("max not integer".into()))?;
        if min > max {
            bail_resolve(ResolveError::InvalidRandomInt("min > max".into()))?;
        }
        let v = rand::rng().random_range(min..=max);
        return Ok(v.to_string());
    }
    Err(ResolveError::MissingVariable(format!("${name}")))
}

fn bail_resolve(e: ResolveError) -> Result<String, ResolveError> {
    Err(e)
}

/// List unresolved `{{name}}` tokens (non-builtin) for preview UI.
pub fn find_missing_variables(query: &str, ctx: &VariableContext) -> Vec<String> {
    let mut missing = Vec::new();
    let mut rest = query;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        let token = after[..end].trim();
        if !token.starts_with('$')
            && !ctx.session.contains_key(token)
            && !ctx.query.contains_key(token)
            && !ctx.collection.contains_key(token)
            && !ctx
                .environment
                .as_ref()
                .is_some_and(|vars| vars.contains_key(token))
            && !ctx.workspace.contains_key(token)
            && !ctx.connection.contains_key(token)
        {
            missing.push(token.to_string());
        }
        rest = &after[end + 2..];
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_uuid_differs() {
        let ctx = VariableContext::default();
        let a = resolve_query("{{$randomUUID}}", &ctx).unwrap();
        let b = resolve_query("{{$randomUUID}}", &ctx).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn user_var_from_session() {
        let mut ctx = VariableContext::default();
        ctx.session.insert("userId".into(), "42".into());
        let out = resolve_query("SELECT * FROM u WHERE id = {{userId}}", &ctx).unwrap();
        assert_eq!(out, "SELECT * FROM u WHERE id = 42");
    }

    #[test]
    fn precedence_session_over_query_collection_env_workspace() {
        let mut ctx = VariableContext::default();
        ctx.workspace.insert("k".into(), "workspace".into());
        ctx.environment = Some({
            let mut v = Variables::new();
            v.insert("k".into(), "environment".into());
            v
        });
        ctx.collection.insert("k".into(), "collection".into());
        ctx.query.insert("k".into(), "query".into());
        ctx.session.insert("k".into(), "session".into());
        let out = resolve_query("{{k}}", &ctx).unwrap();
        assert_eq!(out, "session");
    }

    #[test]
    fn environment_is_skipped_when_none() {
        let mut ctx = VariableContext::default();
        ctx.workspace.insert("k".into(), "workspace".into());
        let out = resolve_query("{{k}}", &ctx).unwrap();
        assert_eq!(out, "workspace");
    }

    #[test]
    fn missing_blocks() {
        let ctx = VariableContext::default();
        let err = resolve_query("{{missing}}", &ctx).unwrap_err();
        assert!(matches!(err, ResolveError::MissingVariable(_)));
    }
}
