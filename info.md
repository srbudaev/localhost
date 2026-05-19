# Continuous Integration

This document describes the CI setup for the `localhost` server: which
workflows exist, what each workflow runs, and which behaviours are exercised
by the automated tests.

All workflows execute on GitHub-hosted `macos-latest` runners (Apple Silicon,
macOS 15), because the server relies on `kqueue`/`kevent` for I/O
multiplexing. The stable Rust toolchain is installed via
`dtolnay/rust-toolchain@stable` and the cargo cache is restored with
`Swatinem/rust-cache@v2`.

The shared environment for every workflow is:

```yaml
env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -D warnings
```

`RUSTFLAGS: -D warnings` makes every compiler warning a hard error so the
codebase has to stay warning-free in CI.

---

## Workflows

Three workflow files live under `.github/workflows/`.

### `ci.yml` — Lint, Build, Tests

Triggered on every `push` and `pull_request` targeting `main` and on any
`ci/**` branch. Contains three jobs that fan out in parallel where possible:

1. **Lint (fmt + clippy)**
   - `cargo fmt --all -- --check` — enforces canonical formatting.
   - `cargo clippy --all-targets --locked -- -D warnings` — runs Clippy on
     the library, binaries, examples and tests; any lint becomes an error.

2. **Build (debug + release)**
   - `cargo build --locked --verbose` — debug profile.
   - `cargo build --locked --release --verbose` — release profile, catches
     issues that only appear with optimisations on (e.g. dead-code in
     `#[cfg(debug_assertions)]` branches).

3. **Tests (lib + integration)**
   - Installs `python3` (needed by integration tests that exercise the CGI
     handler).
   - `cargo test --lib --locked --verbose` — runs unit tests embedded in
     the library.
   - `cargo test --tests --locked --verbose` — runs every file under
     `tests/` as an integration test.

The `--locked` flag forces cargo to honour the checked-in `Cargo.lock`, so a
CI run uses the exact dependency versions every developer also gets locally.

### `stress.yml` — Siege availability check

Triggered manually via `workflow_dispatch` (Actions tab → "Stress" → "Run
workflow"). The job:

1. Builds the server in release mode.
2. Installs `siege` from Homebrew.
3. Starts the server in the background using the example config.
4. Waits for the listening socket to accept connections.
5. Runs `siege -b -t30S http://127.0.0.1:8080/` (benchmark mode, 30 seconds).
6. Parses the `Availability:` value from the siege report and fails the job
   if it falls below **99.5 %**.
7. Shuts the server down cleanly.

The threshold matches the audit requirement: `availability should be at
least 99.5%` under `siege -b`.

### `security.yml` — Cargo audit

Triggered on `workflow_dispatch` and on a weekly `schedule`. The job:

1. Installs `cargo-audit` (`cargo install --locked cargo-audit`, cached
   between runs).
2. Runs `cargo audit` against the committed `Cargo.lock`.

`cargo audit` cross-references every locked dependency with the
RustSec advisory database; the job fails if any dependency has a known
vulnerability.

---

## `Cargo.lock` is committed

`Cargo.lock` was removed from `.gitignore` and is now tracked. For a binary
crate (which is what `localhost` is) the recommended Rust convention is to
commit the lockfile so that every build — local, CI and reviewer — links
against identical dependency versions. The `--locked` flag used by every CI
step makes a stale or out-of-sync lockfile fail the build immediately.

---

## Unit tests added

Tests live next to the code they cover, inside `#[cfg(test)] mod tests`
blocks. The new tests focus on the four areas the project specification
calls out: protocol parsing, configuration validation, routing, and status
code generation.

### `src/http/parser.rs`

Covers request parsing end-to-end through `RequestParser::add_data` /
`parse`:

- `test_parse_simple_request` — minimal `GET` request, asserts method, path
  and version are extracted from the request line.
- `test_parse_request_with_query_string` — confirms the query string is
  split off the path and decoded into `query_params`.
- `test_parse_headers_case_insensitive_lookup` — verifies that
  `Headers::get` is case-insensitive (`content-type`, `CONTENT-TYPE`,
  `Content-Type` all resolve).
- `test_parse_invalid_method_returns_error` — unknown HTTP methods cause
  the parser to return a `ParseError` instead of panicking.
- `test_parse_invalid_version_returns_error` — only `HTTP/1.1` is accepted;
  other versions are rejected at parse time.
- `test_parse_post_with_content_length` — POST body of the declared length
  is read into `request.body`.
- `test_parse_post_incremental_body` — body bytes that arrive in multiple
  `add_data` calls are correctly stitched together.
- `test_parse_chunked_body_single_chunk` — single chunk + terminator.
- `test_parse_chunked_body_multiple_chunks` — several chunks concatenated.
- `test_parse_chunked_body_empty` — only the terminating `0\r\n\r\n`.
- `test_parse_chunked_body_with_chunk_extensions` — chunk-extension syntax
  (`5;extension=foo\r\n...`) is tolerated.
- `test_parse_chunked_body_incremental` — the parser remembers chunks that
  have already been read when the next chunk has not yet arrived.
- `test_parse_chunked_invalid_size_returns_error` — non-hex chunk size is
  rejected.
- `test_body_too_large_with_content_length_rejected` — a declared
  `Content-Length` larger than `max_body_size` is rejected up-front.
- `test_body_too_large_with_chunked_rejected` — a chunked body whose total
  size would exceed `max_body_size` is rejected during chunk parsing.
- `test_body_exactly_at_limit_accepted` — a body whose length equals the
  limit is accepted (boundary check).

### `src/application/handler/router.rs`

Covers `Router::match_route_with_path`, `is_method_allowed`,
`validate_request` and `resolve_file_path`:

- `test_route_matching` — basic exact-match lookup.
- `test_exact_match_beats_prefix_match` — `/foo` defined and `/foo/bar`
  defined: `/foo` resolves to the exact entry, not the prefix.
- `test_longest_prefix_wins` — `/a` and `/a/b/c` defined: a request to
  `/a/b/c/d` matches `/a/b/c`.
- `test_prefix_must_be_followed_by_slash_or_end` — `/foo` does not match
  `/foobar` (prefix boundary semantics).
- `test_root_route_catches_unknown_paths` — `/` acts as the catch-all fallback.
- `test_no_match_when_no_root_route` — without `/` defined, an unknown
  path returns no match.
- `test_method_allowed_when_in_list` / `test_method_rejected_when_not_in_list`
  — allowed-methods filter.
- `test_method_check_is_case_insensitive` — `"get"` in config matches
  `Method::GET` on the wire.
- `test_validate_request_returns_405_for_wrong_method` — wrong method on
  an existing route returns a `405 Method Not Allowed` response.
- `test_validate_request_errors_when_no_route` — request with no matching
  route surfaces as an error to the caller.
- `test_resolve_file_path_rejects_parent_dir` — directory-traversal
  attempts (`../`) are refused by `resolve_file_path`.

### `src/application/config/validator.rs` (covered via integration tests in `tests/config_tests.rs`)

Targets the validator's job of catching misconfigurations before the server
starts:

- Single server on a single port loads cleanly.
- Multiple servers on different ports load cleanly.
- Multiple servers on the same port with **different hostnames** load cleanly
  (host-based virtual hosting).
- Multiple servers on the same port with the **same hostname** are rejected
  as a port conflict.
- Server with port `0` is rejected.
- Server with empty `server_name` is rejected.
- Server with empty `ports` list is rejected.
- Route with empty `methods` list is rejected.
- Route with an invalid HTTP method (e.g. `"BREW"`) is rejected.
- Route that combines more than one of `filename` / `directory` / `redirect`
  is rejected.
- Error page entry with neither `filename` nor a usable target is rejected.
- Unknown error code in `[errors]` is rejected.
- CGI handler whose extension does not start with `.` is rejected.
- `client_timeout_secs = 0` or `client_max_body_size = 0` is rejected.

### `src/http/status.rs`

Targets status-code generation through the public `Response` constructors:

- `test_status_code_creation` — only codes in `100..=599` build a valid
  `StatusCode`.
- `test_status_code_categories` — `is_informational` / `is_success` /
  `is_redirect` / `is_client_error` / `is_server_error` agree with the
  numeric ranges.
- `test_status_code_display` / `test_reason_phrase` — the standard reason
  phrase is returned for each code.
- `test_all_required_error_codes_have_reason_phrases` — every code the
  audit explicitly checks for (`400`, `403`, `404`, `405`, `413`, `500`)
  has a non-empty reason phrase.
- `test_response_bad_request_is_400`, `test_response_forbidden_is_403`,
  `test_response_not_found_is_404`, `test_response_method_not_allowed_is_405`,
  `test_response_payload_too_large_can_be_constructed`,
  `test_response_internal_error_is_500` — each helper constructor on
  `Response` produces the right numeric status.
- `test_allows_body_for_no_content_and_not_modified` — `204` and `304` are
  reported as bodyless, in line with RFC 9110.

### `src/application/config/loader.rs`

- `test_load_valid_config` — a minimal config (`root = "."`) loads and
  validates successfully.
- `test_load_rejects_nonexistent_root` — a config whose `root` points at a
  non-existent directory is rejected by the validator (covers the
  "invalid paths" expectation from the project spec).

---

## Integration test infrastructure

### `tests/common.rs`

The shared helper used by every file under `tests/`:

- Spins the server up on a random free port, returns a handle the test can
  use to send requests and that shuts the server down on `Drop`.
- `send_request` injects a `Connection: close` header unless the caller
  already set one. Without this, integration tests would hang on the
  default HTTP/1.1 keep-alive.
- Reads the response with a 10-second timeout, so a misbehaving server
  fails fast instead of hanging the whole test suite.

### Integration test files under `tests/`

- `tests/integration_tests.rs` — GET / POST / DELETE happy paths, file
  uploads round-trip, static asset serving.
- `tests/error_tests.rs` — malformed requests, oversized bodies, missing
  files, disallowed methods; checks that the server responds with the
  correct status and stays up.
- `tests/cgi_tests.rs` — exercises the CGI handler (Python) for both
  chunked and unchunked request bodies.
- `tests/config_tests.rs` — black-box validation tests for everything
  listed under "validator" above; each case loads a small TOML snippet and
  asserts the resulting `Result`.
- `tests/stress_tests.rs` — short in-process load tests that verify the
  server does not crash under burst traffic (the long-form siege test
  lives in `stress.yml`).

---

## Running everything locally

The same commands that run in CI also run locally:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo build --locked
cargo build --locked --release
cargo test --lib --locked
cargo test --tests --locked
```

For the stress check you need siege installed (`brew install siege` on
macOS) and then:

```bash
cargo run --release -- config.example.toml &
sleep 1
siege -b -t30S http://127.0.0.1:8080/
```

For the security check:

```bash
cargo install --locked cargo-audit
cargo audit