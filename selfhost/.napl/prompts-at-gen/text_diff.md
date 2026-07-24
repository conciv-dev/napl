# Line-oriented text diffing

This module produces and parses a **unified diff** between two texts, line by
line, using a longest-common-subsequence alignment. It is pure: no I/O, no
dependencies on other project modules.

## Where this code lives

The working directory already contains a generated `body_lines` crate at its
root — leave it completely untouched. Create this module as a **new, separate
Rust library crate in a subdirectory named `text_diff/`**: `text_diff/Cargo.toml`
(package name `text_diff`) and `text_diff/src/lib.rs`. Do not add a workspace
manifest and do not modify anything outside `text_diff/`. Ensure `cargo test`
passes from inside `text_diff/`.

## Splitting text into lines — `to_lines(text)`

Return the lines of `text` as owned strings. Empty text has **no** lines (an
empty vector). Otherwise, a single trailing line terminator (`\n`, or `\r\n`) is
removed first, then the text is split on each `\n`; a `\r` immediately before a
split `\n` is consumed with it, but a lone `\r` not followed by `\n` stays inside
the line's content. So `a\nb\n` and `a\nb` both give `["a", "b"]`, `a\r\nb\r\n`
also gives `["a", "b"]`, and `a\rb\n` gives the single line `a\rb`.

## Producing a unified diff

`unified_diff(before, after)` returns a unified diff string with **3** lines of
surrounding context. `unified_diff_with_context(before, after, context)` is the
same with a caller-chosen context width; `unified_diff` is the `context = 3` case.

Split both inputs with `to_lines`, then align them with a longest-common-
subsequence: matching lines become **context** entries, lines only in `before`
become **deletions**, lines only in `after` become **insertions**. Each aligned
entry knows its 1-based line number within `before` (for context and deletions)
and within `after` (for context and insertions).

Emit only the neighborhoods of change: an entry is included when it lies within
`context` positions of some deletion or insertion. Consecutive included entries
form one **hunk**. For each hunk emit a header line

    @@ -<oldStart>,<oldCount> +<newStart>,<newCount> @@

where `oldStart` is the `before` line number of the hunk's first non-insertion
entry (or `0` if the hunk has none) and `oldCount` is how many of its entries came
from `before` (context + deletions); `newStart` and `newCount` are the mirror over
`after` (context + insertions). After the header, emit each entry on its own line
prefixed by a single marker character: a space for context, `-` for a deletion,
`+` for an insertion, followed by the line text. Join every emitted line with
`\n`. When there is no change at all, the diff is the empty string.

Consequences to preserve exactly: diffing `""` into `a\nb\n` yields
`@@ -0,0 +1,2 @@` then `+a` then `+b`; diffing `a\nb\n` into `""` yields
`@@ -1,2 +0,0 @@` then `-a` then `-b`; identical inputs yield `""`.

## Parsing a unified diff — `parse_hunks(diff)`

Return the hunks contained in a unified-diff string. A diff that is empty or only
whitespace yields no hunks. Split the diff into lines the same CRLF-aware way used
elsewhere. A line matching the header shape `@@ -A,B +C,D @@` (A, B, C, D being
non-negative integers) starts a new hunk carrying those four numbers and, so far,
no lines. Any other line, when a hunk is open, is one of the hunk's lines: its
first character selects the kind — a space means **context**, `-` means a
**deletion**, `+` means an **insertion**, and any other first character is ignored
— and the remainder of the line after that first character is the line's text.

Model the result as: a `Hunk` with public fields `old_start`, `old_count`,
`new_start`, `new_count` (unsigned integers) and `lines` (a vector of
`HunkLine`); a `HunkLine` with public fields `kind` and `text`; and a `HunkKind`
with the three variants `Context`, `Del`, and `Ins`. So parsing
`@@ -1,3 +1,3 @@` followed by ` a`, `-b`, `+B`, ` c` gives one hunk with
`old_start = 1`, `old_count = 3`, `new_start = 1`, `new_count = 3`, and line
kinds context, deletion, insertion, context, carrying texts `a`, `b`, `B`, `c`.

## Re-applying hunks — `apply_hunks(before, hunks)`

Reconstruct the diff's target from the original text and parsed hunks. Take the
`before` lines (via `to_lines`); walk the hunks in order, first copying any
original lines before each hunk's start, then for each hunk line: a context line
copies the corresponding original line (falling back to the hunk line's own text
if the original is exhausted) and advances the original position; a deletion
advances the original position without emitting; an insertion emits its text
without advancing. After the last hunk, copy any remaining original lines. Join
the result with `\n`.

This must round-trip: for any `before`/`after`, parsing `unified_diff(before,
after)` and applying the hunks back onto `before` reproduces `after`'s lines
joined by `\n` (i.e. `to_lines(after)` joined with `\n`).
</content>
