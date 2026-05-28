# Variable Scope Contract (P0)

Defines deterministic variable precedence and runtime template resolution across query execution and connection resolution.

## Scope precedence

For non-builtin tokens (`{{name}}`), resolve in this exact order:

1. `session`
2. `query`
3. `collection`
4. `environment` (only when active environment != `No Environment`)
5. `workspace`
6. `connection` (legacy compatibility fallback)

Builtin tokens use `{{$...}}` and are evaluated directly:

- `{{$randomUUID}}`
- `{{$timestamp}}`
- `{{$isoTimestamp}}`
- `{{$randomInt(min,max)}}`

## No Environment semantics

- App startup default is `No Environment`.
- In `No Environment`, environment scope is skipped completely.
- Missing env variables in `No Environment` must not produce warnings unless the token is unresolved after all other scopes.

## Runtime resolution contract

### Query run path

1. Read editor SQL.
2. Apply legacy `$VAR` substitutions (compat path).
3. Resolve `{{...}}` with scope precedence.
4. If unresolved tokens remain -> block run with inline error list.
5. Execute resolved SQL.
6. Persist history with run status.

### Connection template resolution

1. Read selected connection template fields.
2. Resolve each string field with same scope precedence.
3. Validate resolved values (e.g., parse port).
4. Connect or show categorized validation error.

## Error behavior

- Unresolved variable -> `Missing variable: {{name}}`
- Invalid random int args -> `Invalid {{$randomInt}}: ...`
- Resolved connection port parse failure -> actionable `invalid port after variable resolution`

## Acceptance criteria

- [ ] **VS-AC1:** Session scope overrides all lower scopes.
- [ ] **VS-AC2:** Query scope overrides collection/environment/workspace.
- [ ] **VS-AC3:** Environment scope is ignored in `No Environment`.
- [ ] **VS-AC4:** Builtins resolve every run and remain deterministic per run invocation.
- [ ] **VS-AC5:** Missing variables block execution before DB call.
- [ ] **VS-AC6:** Connection template field resolution and query resolution follow identical precedence.

## Current implementation anchor

- Precedence and parsing implemented in `crates/based-query/src/resolve.rs`.
- Runtime connection-template resolver implemented in `crates/based-workspace/src/resolve.rs`.
