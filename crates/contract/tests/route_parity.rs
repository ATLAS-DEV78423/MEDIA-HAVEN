//! ADR-0005 drift test: the command list, the registered POST routes and the
//! generated TypeScript union must all describe the same named-command surface.
//!
//! There is no codegen yet, so this test extracts each list from source text
//! and asserts the three sets are identical. A divergence in any of the three
//! files fails the build. `/healthz` and `/readyz` are GET probes, not renderer
//! commands, and are excluded by construction (POST-only) and by name.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Every `"..."` literal in a snippet, in order. The three source blocks we
/// parse contain nothing but command names inside quotes, so this is sufficient
/// without pulling in a regex dependency.
fn quoted_strings(src: &str) -> Vec<String> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            if j < bytes.len() {
                out.push(src[start..j].to_string());
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// COMMANDS from `crates/contract/src/lib.rs`.
fn commands_from_contract(src: &str) -> BTreeSet<String> {
    let anchor = src
        .find("pub const COMMANDS")
        .expect("contract: pub const COMMANDS not found");
    let open = src[anchor..]
        .find("= &[")
        .expect("contract: COMMANDS array literal not found");
    let start = anchor + open + "= &[".len();
    let end = src[start..]
        .find(']')
        .expect("contract: COMMANDS array not closed");
    quoted_strings(&src[start..start + end])
        .into_iter()
        .collect()
}

/// Registered POST routes from `crates/core/src/routes.rs`.
///
/// Whitespace-tolerant on purpose: rustfmt may reflow a long registration onto
/// several lines, so neither the opening quote nor `post(` may be assumed to
/// sit immediately after `.route(`.
fn routes_from_core(src: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut cursor = 0;
    while let Some(idx) = src[cursor..].find(".route(") {
        let after_call = cursor + idx + ".route(".len();
        let rest = &src[after_call..];

        // Skip whitespace between `.route(` and the path's opening quote.
        let Some(path_open) = rest.find('"') else {
            break;
        };
        if !rest[..path_open].chars().all(char::is_whitespace) {
            cursor = after_call;
            continue;
        }

        let path_body = &rest[path_open + 1..];
        let Some(path_close) = path_body.find('"') else {
            break;
        };
        let name = &path_body[..path_close];

        // After the path: whitespace, comma, whitespace, then `post(`.
        let tail = path_body[path_close + 1..].trim_start();
        if let Some(tail) = tail.strip_prefix(',') {
            // Only POST routes are renderer commands; GET probes are excluded.
            if tail.trim_start().starts_with("post(") && name != "healthz" && name != "readyz" {
                names.insert(name.trim_start_matches('/').to_string());
            }
        }
        cursor = after_call + path_open + path_close + 2;
    }
    names
}

/// Independent POST count — deliberately does not use the route parser, so a
/// registration the parser misses still trips the count assertions below.
fn post_registration_count(src: &str) -> usize {
    src.matches("post(").count()
}

/// CommandName union from `apps/desktop/src/services/generated.ts`.
fn commands_from_generated_ts(src: &str) -> BTreeSet<String> {
    let anchor = src
        .find("export type CommandName")
        .expect("generated.ts: export type CommandName not found");
    let eq = src[anchor..]
        .find('=')
        .expect("generated.ts: union '=' not found");
    let start = anchor + eq + 1;
    let end = src[start..]
        .find(';')
        .expect("generated.ts: CommandName union not closed");
    quoted_strings(&src[start..start + end])
        .into_iter()
        .collect()
}

#[test]
fn command_lists_derive_from_one_source() {
    // CARGO_MANIFEST_DIR is <workspace>/crates/contract for this test.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |rel: &str| {
        fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
    };

    let contract = commands_from_contract(&read("crates/contract/src/lib.rs"));
    let routes_src = read("crates/core/src/routes.rs");
    let routes = routes_from_core(&routes_src);
    let generated = commands_from_generated_ts(&read("apps/desktop/src/services/generated.ts"));

    // The independent `post(` count cannot be fooled by a route the parser
    // skips (e.g. a reflowed multi-line registration). Three-way agreement is
    // required: source registrations, parsed routes and the command list.
    let post_calls = post_registration_count(&routes_src);
    assert_eq!(
        post_calls, 24,
        "routes.rs must register exactly 24 POST renderer commands"
    );
    assert_eq!(
        post_calls,
        routes.len(),
        "POST registrations != parsed POST routes — a .route( registration was missed"
    );
    assert_eq!(
        post_calls,
        contract.len(),
        "POST registrations != COMMANDS — a registered command is missing from the contract"
    );

    assert_eq!(
        contract, routes,
        "COMMANDS and registered POST routes diverged"
    );
    assert_eq!(
        contract, generated,
        "COMMANDS and generated CommandName diverged"
    );
    assert_eq!(contract.len(), 24, "expected exactly 24 renderer commands");

    // The text-extracted list must also match the compiled constant, so a
    // parser that silently found the wrong block cannot make this test pass.
    let compiled: BTreeSet<String> = mediahaven_contract::COMMANDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        contract, compiled,
        "extracted COMMANDS diverged from the compiled constant"
    );
}

#[cfg(test)]
mod parser_self_check {
    use super::*;

    #[test]
    fn route_parser_ignores_get_probes_and_takes_post_routes() {
        let sample = r#"
            Router::new()
                .route("/healthz", get(healthz))
                .route("/readyz", get(readyz))
                .route("/library_home", post(library_home))
                .route("/queue_add", post(queue_add));
        "#;
        let got = routes_from_core(sample);
        assert_eq!(
            got,
            BTreeSet::from(["library_home".to_string(), "queue_add".to_string()])
        );
    }

    #[test]
    fn route_parser_handles_multiline_registrations() {
        // Would FAIL on the old parser: it searched the literal `.route("` and
        // required `, post(` on one line, so it found none of these reflowed
        // registrations. The independent count must see them too.
        let sample = r#"
            Router::new()
                .route(
                    "/healthz",
                    get(healthz),
                )
                .route(
                    "/new_cmd",
                    post(new_cmd),
                )
                .route(
                    "/readyz",
                    get(readyz),
                )
                .route("/one_line", post(one_line));
        "#;
        assert_eq!(post_registration_count(sample), 2);
        assert_eq!(
            routes_from_core(sample),
            BTreeSet::from(["new_cmd".to_string(), "one_line".to_string()]),
            "multi-line .route( registrations must be parsed; GET probes excluded"
        );
    }

    #[test]
    fn drift_in_generated_union_is_detected() {
        // Proves the equality assertion fails on a divergence (the H2 red
        // case) without modifying any checked-in source file.
        let good = "export type CommandName = | \"library_home\" | \"library_movies\";";
        let drifted =
            "export type CommandName = | \"library_home\" | \"library_movies\" | \"bogus\";";
        assert_ne!(
            commands_from_generated_ts(good),
            commands_from_generated_ts(drifted)
        );
    }
}
