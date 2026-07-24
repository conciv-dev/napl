//! Serde types and validators for the NAPL on-disk schemas.
//!
//! Validation error *messages* intentionally differ from the TypeScript zod
//! text, but the **acceptance set matches**: every value the zod schemas accept
//! or reject, these types accept or reject identically.
//!
//! Stage1: each submodule is a thin adapter over its NAPL-generated crate. The
//! generated crates surface their own error enums; the adapters map them to the
//! shared [`SchemaError`] the callers expect.

mod attribution;
mod frontmatter;
mod ir;
mod journal;
mod line_range;
mod lock;
mod map;
mod ml;
mod ordered_map;

pub use attribution::{
    entries_at_body_line, parse_attribution_entries, validate_attribution, Attribution,
    AttributionEntry,
};
pub use frontmatter::{parse_frontmatter, Frontmatter, ParsedPrompt, PromptTest};
pub use ir::{validate_ir, Ir, IrFunction, IrTest, IrType};
pub use journal::{
    file_history, file_patch, next_gen_number, read_journal_str, JournalEntry, JournalFile,
    JournalMode,
};
pub use line_range::LineRange;
pub use lock::{
    default_agent_config, parse_lock, resolve_agent_config, resolve_prompt_aliases, AgentConfig,
    AgentPreset, Backend, HlLock, DEFAULT_AGENT_PRESET, DEFAULT_BACKEND, DEFAULT_MODEL,
};
pub use map::{
    declared_targets_for_module, empty_map, files_for_module, has_module, is_prompt_gen_stale,
    map_to_json, parse_map, prompts_for_module, record_attribution, record_unattributed,
    AttributionInput, FileInput, FileRecord, ModuleFile, NaplMap, PromptRecord, PromptTargetRecord,
    UnattributedInput,
};
pub use ml::{ml_entries_at_body_line, parse_ml_entries, validate_ml, Ml, MlEntry, MlKind};
pub use ordered_map::OrderedMap;

/// The single error type surfaced by schema parsing and validation.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// Structural or type mismatch during deserialization.
    #[error("deserialization failed: {0}")]
    Deserialize(String),
    /// A value parsed structurally but failed a semantic constraint.
    #[error("validation failed: {0}")]
    Validation(String),
}
