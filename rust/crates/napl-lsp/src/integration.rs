//! In-process analysis tests: build a real on-disk `.napl` workspace and drive
//! the hover, navigation, and diagnostics entry points the LSP handlers call.

use tower_lsp_server::ls_types::{DiagnosticSeverity, GotoDefinitionResponse};

use crate::convert::position;
use crate::testkit::{greeting, greeting_with, GEN_CONTENT};
use crate::{diagnostics, hover, navigation};

const BODY_LINE_3_DOC: u32 = 6;

#[test]
fn diagnostics_surface_machine_layer_ambiguity_on_clean_prompt() {
    let fixture = greeting();
    let diags = diagnostics::compute(&fixture.prompt_path(), crate::testkit::PROMPT_RAW);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(diags[0].source.as_deref(), Some("napl-mapl"));
    assert_eq!(diags[0].message, "name may be empty");
    assert_eq!(diags[0].range.start.line, BODY_LINE_3_DOC);
}

#[test]
fn diagnostics_report_drift_when_generated_file_edited() {
    let fixture = greeting();
    std::fs::write(fixture.generated_path(), "export const greet = 1;\n").unwrap();
    let diags = diagnostics::compute(&fixture.prompt_path(), crate::testkit::PROMPT_RAW);
    let drift = diags
        .iter()
        .find(|d| d.severity == Some(DiagnosticSeverity::ERROR) && d.message.starts_with("DRIFT"));
    assert!(drift.is_some(), "expected a DRIFT diagnostic, got {diags:?}");
}

#[test]
fn diagnostics_flag_missing_frontmatter() {
    let fixture = greeting();
    let diags = diagnostics::compute(&fixture.prompt_path(), "no frontmatter here\n");
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("missing YAML frontmatter"));
}

#[test]
fn generated_files_carry_no_diagnostics() {
    let fixture = greeting();
    let diags = diagnostics::compute(&fixture.generated_path(), GEN_CONTENT);
    assert!(diags.is_empty());
}

#[test]
fn hover_on_prompt_body_shows_attribution_and_machine_layer() {
    let fixture = greeting();
    let result = hover::hover(
        &fixture.prompt_path(),
        crate::testkit::PROMPT_RAW,
        position(BODY_LINE_3_DOC as usize, 4),
    );
    let markdown = hover_markdown(result);
    assert!(markdown.contains("greet function signature"));
    assert!(markdown.contains("machine says"));
    assert!(markdown.contains("name may be empty"));
}

#[test]
fn hover_on_generated_line_points_back_at_prompt() {
    let fixture = greeting();
    let result = hover::hover(&fixture.generated_path(), GEN_CONTENT, position(0, 0));
    let markdown = hover_markdown(result);
    assert!(markdown.contains("⇠ greeting.napl:7"));
    assert!(markdown.contains("greet function signature"));
}

#[test]
fn definition_from_prompt_body_resolves_to_generated_source() {
    let fixture = greeting();
    let response = navigation::definition(
        &fixture.prompt_path(),
        crate::testkit::PROMPT_RAW,
        position(BODY_LINE_3_DOC as usize, 4),
    );
    let locations = definition_locations(response);
    assert_eq!(locations.len(), 1);
    assert!(locations[0].uri.as_str().ends_with("greeting.ts"));
    assert_eq!(locations[0].range.start.line, 0);
}

#[test]
fn definition_from_generated_line_resolves_back_to_prompt() {
    let fixture = greeting();
    let response = navigation::definition(&fixture.generated_path(), GEN_CONTENT, position(0, 0));
    let locations = definition_locations(response);
    assert_eq!(locations.len(), 1);
    assert!(locations[0].uri.as_str().ends_with("greeting.napl"));
    assert_eq!(locations[0].range.start.line, BODY_LINE_3_DOC);
}

#[test]
fn references_from_generated_line_lists_prompt_locations() {
    let fixture = greeting();
    let locations = navigation::references(&fixture.generated_path(), position(0, 0)).unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].range.start.line, BODY_LINE_3_DOC);
}

#[test]
fn code_lens_annotates_generated_file_with_prompt_backlink() {
    let fixture = greeting();
    let lenses = navigation::code_lens(&fixture.generated_path(), GEN_CONTENT);
    assert_eq!(lenses.len(), 1);
    let command = lenses[0].command.as_ref().unwrap();
    assert_eq!(command.command, "napl.revealLocation");
    assert!(command.title.contains("greeting.napl"));
    assert!(!command.title.contains("DRIFT"));
}

#[test]
fn code_lens_prefixes_drift_banner_when_generated_file_edited() {
    let fixture = greeting();
    let edited = "export const greet = 1;\n";
    std::fs::write(fixture.generated_path(), edited).unwrap();
    let lenses = navigation::code_lens(&fixture.generated_path(), edited);
    assert!(lenses[0].command.as_ref().unwrap().title.contains("DRIFT"));
}

#[test]
fn emoji_alias_prompt_and_mapl_behave_identically() {
    let fixture = greeting_with("examples/greeting.\u{1F9D1}", ".napl/mapl/greeting.\u{1F916}");
    let prompt = fixture.root.join("examples/greeting.\u{1F9D1}");
    let diags = diagnostics::compute(&prompt, crate::testkit::PROMPT_RAW);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].message, "name may be empty");
    let markdown = hover_markdown(hover::hover(
        &prompt,
        crate::testkit::PROMPT_RAW,
        position(BODY_LINE_3_DOC as usize, 4),
    ));
    assert!(markdown.contains("machine says"));
}

fn hover_markdown(hover: Option<tower_lsp_server::ls_types::Hover>) -> String {
    match hover.expect("expected a hover").contents {
        tower_lsp_server::ls_types::HoverContents::Markup(markup) => markup.value,
        other => panic!("unexpected hover contents: {other:?}"),
    }
}

fn definition_locations(
    response: Option<GotoDefinitionResponse>,
) -> Vec<tower_lsp_server::ls_types::Location> {
    match response.expect("expected a definition response") {
        GotoDefinitionResponse::Array(locations) => locations,
        GotoDefinitionResponse::Scalar(location) => vec![location],
        GotoDefinitionResponse::Link(_) => panic!("unexpected link response"),
    }
}
