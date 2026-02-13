//! Parser for the apply-patch format.
//!
//! Grammar (simplified):
//! ```text
//! Patch := "*** Begin Patch" LF { FileOp } "*** End Patch" LF?
//! FileOp := AddFile | DeleteFile | UpdateFile
//! AddFile := "*** Add File: " path LF { "+" line LF }
//! DeleteFile := "*** Delete File: " path LF
//! UpdateFile := "*** Update File: " path LF [ MoveTo ] { Hunk }
//! MoveTo := "*** Move to: " path LF
//! Hunk := ("@@" [ " " context ]) LF { HunkLine } [ "*** End of File" LF ]
//! HunkLine := (" " | "-" | "+") text LF
//! ```

use std::path::{Path, PathBuf};
use thiserror::Error;

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

#[derive(Debug, PartialEq, Error, Clone)]
pub enum ParseError {
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
    #[error("invalid hunk at line {line_number}: {message}")]
    InvalidHunk { message: String, line_number: usize },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Hunk {
    AddFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    UpdateFile {
        path: PathBuf,
        move_path: Option<PathBuf>,
        chunks: Vec<UpdateFileChunk>,
    },
}

impl Hunk {
    pub fn resolve_path(&self, cwd: &Path) -> PathBuf {
        match self {
            Hunk::AddFile { path, .. } => cwd.join(path),
            Hunk::DeleteFile { path } => cwd.join(path),
            Hunk::UpdateFile { path, .. } => cwd.join(path),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct UpdateFileChunk {
    /// A line of context used to locate the chunk position (e.g. function/class name).
    pub change_context: Option<String>,
    /// Lines that should be found and replaced.
    pub old_lines: Vec<String>,
    /// Lines that replace old_lines.
    pub new_lines: Vec<String>,
    /// If true, old_lines should occur at the end of the file.
    pub is_end_of_file: bool,
}

/// Parse a patch string into a list of hunks.
pub fn parse_patch(patch: &str) -> Result<Vec<Hunk>, ParseError> {
    let lines: Vec<&str> = patch.trim().lines().collect();

    // Check boundaries, allowing lenient heredoc wrapping
    let lines = match check_boundaries_strict(&lines) {
        Ok(()) => &lines[..],
        Err(e) => check_boundaries_lenient(&lines, e)?,
    };

    let mut hunks: Vec<Hunk> = Vec::new();
    let last_line_index = lines.len().saturating_sub(1);
    let mut remaining = &lines[1..last_line_index];
    let mut line_number = 2;

    while !remaining.is_empty() {
        let (hunk, consumed) = parse_one_hunk(remaining, line_number)?;
        hunks.push(hunk);
        line_number += consumed;
        remaining = &remaining[consumed..];
    }

    Ok(hunks)
}

fn check_boundaries_strict(lines: &[&str]) -> Result<(), ParseError> {
    let first = lines.first().map(|l| l.trim());
    let last = lines.last().map(|l| l.trim());

    match (first, last) {
        (Some(f), Some(l)) if f == BEGIN_PATCH_MARKER && l == END_PATCH_MARKER => Ok(()),
        (Some(f), _) if f != BEGIN_PATCH_MARKER => Err(ParseError::InvalidPatch(
            "first line must be '*** Begin Patch'".into(),
        )),
        _ => Err(ParseError::InvalidPatch(
            "last line must be '*** End Patch'".into(),
        )),
    }
}

fn check_boundaries_lenient<'a>(
    original: &'a [&'a str],
    original_err: ParseError,
) -> Result<&'a [&'a str], ParseError> {
    match original {
        [first, .., last] => {
            if (*first == "<<EOF" || *first == "<<'EOF'" || *first == "<<\"EOF\"")
                && last.ends_with("EOF")
                && original.len() >= 4
            {
                let inner = &original[1..original.len() - 1];
                check_boundaries_strict(inner)?;
                Ok(inner)
            } else {
                Err(original_err)
            }
        }
        _ => Err(original_err),
    }
}

fn parse_one_hunk(lines: &[&str], line_number: usize) -> Result<(Hunk, usize), ParseError> {
    let first = lines[0].trim();

    if let Some(path) = first.strip_prefix(ADD_FILE_MARKER) {
        let mut contents = String::new();
        let mut parsed = 1;
        for line in &lines[1..] {
            if let Some(content) = line.strip_prefix('+') {
                contents.push_str(content);
                contents.push('\n');
                parsed += 1;
            } else {
                break;
            }
        }
        return Ok((
            Hunk::AddFile {
                path: PathBuf::from(path),
                contents,
            },
            parsed,
        ));
    }

    if let Some(path) = first.strip_prefix(DELETE_FILE_MARKER) {
        return Ok((
            Hunk::DeleteFile {
                path: PathBuf::from(path),
            },
            1,
        ));
    }

    if let Some(path) = first.strip_prefix(UPDATE_FILE_MARKER) {
        let mut remaining = &lines[1..];
        let mut parsed = 1;

        // Optional move
        let move_path = remaining
            .first()
            .and_then(|l| l.strip_prefix(MOVE_TO_MARKER))
            .map(PathBuf::from);

        if move_path.is_some() {
            remaining = &remaining[1..];
            parsed += 1;
        }

        let mut chunks = Vec::new();
        while !remaining.is_empty() {
            // Skip blank lines between chunks
            if remaining[0].trim().is_empty() {
                parsed += 1;
                remaining = &remaining[1..];
                continue;
            }
            // Stop at next file operation marker
            if remaining[0].starts_with("***") {
                break;
            }

            let (chunk, chunk_lines) =
                parse_update_chunk(remaining, line_number + parsed, chunks.is_empty())?;
            chunks.push(chunk);
            parsed += chunk_lines;
            remaining = &remaining[chunk_lines..];
        }

        if chunks.is_empty() {
            return Err(ParseError::InvalidHunk {
                message: format!("update hunk for '{}' is empty", path),
                line_number,
            });
        }

        return Ok((
            Hunk::UpdateFile {
                path: PathBuf::from(path),
                move_path,
                chunks,
            },
            parsed,
        ));
    }

    Err(ParseError::InvalidHunk {
        message: format!(
            "'{}' is not a valid hunk header. Expected: *** Add File, *** Delete File, or *** Update File",
            first
        ),
        line_number,
    })
}

fn parse_update_chunk(
    lines: &[&str],
    line_number: usize,
    allow_missing_context: bool,
) -> Result<(UpdateFileChunk, usize), ParseError> {
    if lines.is_empty() {
        return Err(ParseError::InvalidHunk {
            message: "update chunk has no lines".into(),
            line_number,
        });
    }

    let (change_context, start_index) = if lines[0] == EMPTY_CHANGE_CONTEXT_MARKER {
        (None, 1)
    } else if let Some(ctx) = lines[0].strip_prefix(CHANGE_CONTEXT_MARKER) {
        (Some(ctx.to_string()), 1)
    } else if allow_missing_context {
        (None, 0)
    } else {
        return Err(ParseError::InvalidHunk {
            message: format!("expected @@ context marker, got: '{}'", lines[0]),
            line_number,
        });
    };

    if start_index >= lines.len() {
        return Err(ParseError::InvalidHunk {
            message: "update chunk has no diff lines".into(),
            line_number: line_number + 1,
        });
    }

    let mut chunk = UpdateFileChunk {
        change_context,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    };

    let mut parsed = 0;
    for line in &lines[start_index..] {
        match *line {
            EOF_MARKER => {
                if parsed == 0 {
                    return Err(ParseError::InvalidHunk {
                        message: "update chunk has no diff lines before End of File".into(),
                        line_number: line_number + 1,
                    });
                }
                chunk.is_end_of_file = true;
                parsed += 1;
                break;
            }
            content => {
                match content.chars().next() {
                    None => {
                        // Empty line = context
                        chunk.old_lines.push(String::new());
                        chunk.new_lines.push(String::new());
                    }
                    Some(' ') => {
                        chunk.old_lines.push(content[1..].to_string());
                        chunk.new_lines.push(content[1..].to_string());
                    }
                    Some('+') => {
                        chunk.new_lines.push(content[1..].to_string());
                    }
                    Some('-') => {
                        chunk.old_lines.push(content[1..].to_string());
                    }
                    _ => {
                        if parsed == 0 {
                            return Err(ParseError::InvalidHunk {
                                message: format!(
                                    "unexpected line in update chunk: '{}'. Lines must start with ' ', '+', or '-'",
                                    content
                                ),
                                line_number: line_number + 1,
                            });
                        }
                        // Start of next chunk or hunk
                        break;
                    }
                }
                parsed += 1;
            }
        }
    }

    Ok((chunk, parsed + start_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_patch() {
        let hunks = parse_patch("*** Begin Patch\n*** End Patch").unwrap();
        assert!(hunks.is_empty());
    }

    #[test]
    fn parse_add_file() {
        let hunks =
            parse_patch("*** Begin Patch\n*** Add File: hello.txt\n+hello world\n*** End Patch")
                .unwrap();
        assert_eq!(hunks.len(), 1);
        match &hunks[0] {
            Hunk::AddFile { path, contents } => {
                assert_eq!(path, &PathBuf::from("hello.txt"));
                assert_eq!(contents, "hello world\n");
            }
            _ => panic!("wrong hunk type"),
        }
    }

    #[test]
    fn parse_delete_file() {
        let hunks =
            parse_patch("*** Begin Patch\n*** Delete File: old.txt\n*** End Patch").unwrap();
        assert_eq!(hunks.len(), 1);
        assert!(
            matches!(&hunks[0], Hunk::DeleteFile { path } if path == &PathBuf::from("old.txt"))
        );
    }

    #[test]
    fn parse_update_with_context() {
        let hunks = parse_patch(concat!(
            "*** Begin Patch\n",
            "*** Update File: file.py\n",
            "@@ def greet():\n",
            "-    print(\"hi\")\n",
            "+    print(\"hello\")\n",
            "*** End Patch"
        ))
        .unwrap();
        assert_eq!(hunks.len(), 1);
        match &hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                assert_eq!(chunks[0].change_context.as_deref(), Some("def greet():"));
                assert_eq!(chunks[0].old_lines, vec!["    print(\"hi\")"]);
                assert_eq!(chunks[0].new_lines, vec!["    print(\"hello\")"]);
            }
            _ => panic!("wrong hunk type"),
        }
    }

    #[test]
    fn parse_update_with_move() {
        let hunks = parse_patch(concat!(
            "*** Begin Patch\n",
            "*** Update File: src/old.rs\n",
            "*** Move to: src/new.rs\n",
            "@@\n",
            "-old\n",
            "+new\n",
            "*** End Patch"
        ))
        .unwrap();
        match &hunks[0] {
            Hunk::UpdateFile {
                path, move_path, ..
            } => {
                assert_eq!(path, &PathBuf::from("src/old.rs"));
                assert_eq!(move_path.as_ref().unwrap(), &PathBuf::from("src/new.rs"));
            }
            _ => panic!("wrong hunk type"),
        }
    }

    #[test]
    fn parse_multi_chunk_update() {
        let hunks = parse_patch(concat!(
            "*** Begin Patch\n",
            "*** Update File: file.txt\n",
            "@@\n",
            " foo\n",
            "-bar\n",
            "+BAR\n",
            "@@\n",
            " baz\n",
            "-qux\n",
            "+QUX\n",
            "*** End Patch"
        ))
        .unwrap();
        match &hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                assert_eq!(chunks.len(), 2);
            }
            _ => panic!("wrong hunk type"),
        }
    }

    #[test]
    fn parse_eof_marker() {
        let hunks = parse_patch(concat!(
            "*** Begin Patch\n",
            "*** Update File: file.txt\n",
            "@@\n",
            "+new line\n",
            "*** End of File\n",
            "*** End Patch"
        ))
        .unwrap();
        match &hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                assert!(chunks[0].is_end_of_file);
            }
            _ => panic!("wrong hunk type"),
        }
    }

    #[test]
    fn parse_heredoc_wrapper() {
        let hunks = parse_patch(concat!(
            "<<'EOF'\n",
            "*** Begin Patch\n",
            "*** Add File: test.txt\n",
            "+content\n",
            "*** End Patch\n",
            "EOF\n"
        ))
        .unwrap();
        assert_eq!(hunks.len(), 1);
    }

    #[test]
    fn parse_bad_first_line() {
        let err = parse_patch("bad\n*** End Patch").unwrap_err();
        assert!(matches!(err, ParseError::InvalidPatch(_)));
    }

    #[test]
    fn parse_empty_update_hunk() {
        let err =
            parse_patch("*** Begin Patch\n*** Update File: test.py\n*** End Patch").unwrap_err();
        assert!(matches!(err, ParseError::InvalidHunk { .. }));
    }

    #[test]
    fn parse_implicit_context_first_chunk() {
        // First chunk can omit @@ marker
        let hunks = parse_patch(concat!(
            "*** Begin Patch\n",
            "*** Update File: file.py\n",
            " import foo\n",
            "+import bar\n",
            "*** End Patch"
        ))
        .unwrap();
        match &hunks[0] {
            Hunk::UpdateFile { chunks, .. } => {
                assert_eq!(chunks.len(), 1);
                assert!(chunks[0].change_context.is_none());
                assert_eq!(chunks[0].old_lines, vec!["import foo"]);
                assert_eq!(chunks[0].new_lines, vec!["import foo", "import bar"]);
            }
            _ => panic!("wrong hunk type"),
        }
    }
}
