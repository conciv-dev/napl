//! Stage1 adapter over the NAPL-generated `incremental` crate.

pub use gen_incremental::{
    diff_body_lines, incremental_unlock_list, select_intersecting_entries, BodyLineDiff,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_tracks_changed_lines_and_unified() {
        let diff = diff_body_lines(
            "Greet a person by name.\n",
            "Greet a person by name, loudly.\n",
        );
        assert!(diff.unified.contains("-Greet a person by name."));
        assert!(diff.unified.contains("+Greet a person by name, loudly."));
        assert_eq!(diff.changed_old_lines, vec![1]);
        assert_eq!(diff.changed_new_lines, vec![1]);
    }

    #[test]
    fn unlock_list_is_sorted_and_deduped() {
        let list = incremental_unlock_list(
            &[".napl/src/typescript/greet.ts".to_string()],
            &[],
            ".napl/src/typescript",
        );
        assert_eq!(list, vec![".napl/src/typescript/greet.ts".to_string()]);
    }
}
