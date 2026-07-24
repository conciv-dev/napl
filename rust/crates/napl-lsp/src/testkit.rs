//! Test-only fixture builder: a minimal but fully-attributed `.napl` workspace
//! on disk (prompt, generated source, map, attribution, machine layer) whose
//! hashes are computed at build time so the prompt classifies as clean.

use std::path::{Path, PathBuf};

use napl_core::hash::content_hash;
use tempfile::TempDir;

pub const PROMPT_REL: &str = "examples/greeting.napl";
pub const GEN_REL: &str = ".napl/src/typescript/src/greeting.ts";

pub const PROMPT_RAW: &str =
    "---\nmodule: greeting\ntargets: [typescript]\n---\nGreet a user.\n\nThe greet function takes a name.\n";
pub const GEN_CONTENT: &str =
    "export function greet(name: string): string {\n  return `Hello, ${name}!`;\n}\n";

const ATTRIBUTION: &str = "module: greeting\ntarget: typescript\nentries:\n  - promptLines:\n      - 3\n      - 3\n    file: src/greeting.ts\n    lines:\n      - 1\n      - 1\n    note: greet function signature\n";
const MAPL: &str = "module: greeting\ntarget: typescript\nentries:\n  - promptLines:\n      - 3\n      - 3\n    kind: ambiguity\n    message: name may be empty\n    reasoning: unclear\n";

/// A live temp workspace; drops (and deletes) with the returned value.
pub struct Fixture {
    pub _dir: TempDir,
    pub root: PathBuf,
}

impl Fixture {
    pub fn prompt_path(&self) -> PathBuf {
        self.root.join(PROMPT_REL)
    }

    pub fn generated_path(&self) -> PathBuf {
        self.root.join(GEN_REL)
    }
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// Build the greeting fixture with the given prompt/mapl spellings.
pub fn greeting_with(prompt_rel: &str, mapl_rel: &str) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    write(&root, prompt_rel, PROMPT_RAW);
    write(&root, GEN_REL, GEN_CONTENT);
    write(&root, ".napl/attribution/greeting.yaml", ATTRIBUTION);
    write(&root, mapl_rel, MAPL);
    let prompt_hash = content_hash(PROMPT_RAW);
    let file_hash = content_hash(GEN_CONTENT);
    let map = format!(
        "{{\n  \"version\": 2,\n  \"prompts\": {{\n    \"{prompt_rel}\": {{\n      \"module\": \"greeting\",\n      \"promptHash\": \"{prompt_hash}\",\n      \"declaredTargets\": [\"typescript\"],\n      \"targets\": {{\n        \"typescript\": {{\n          \"promptHashAtGen\": \"{prompt_hash}\",\n          \"files\": [\"{GEN_REL}\"]\n        }}\n      }}\n    }}\n  }},\n  \"files\": {{\n    \"{GEN_REL}\": {{\n      \"target\": \"typescript\",\n      \"hash\": \"{file_hash}\",\n      \"prompts\": [\"{prompt_rel}\"]\n    }}\n  }}\n}}\n"
    );
    write(&root, ".napl/map.json", &map);
    Fixture { _dir: dir, root }
}

/// The canonical-spelling greeting fixture.
pub fn greeting() -> Fixture {
    greeting_with(PROMPT_REL, ".napl/mapl/greeting.mapl")
}
