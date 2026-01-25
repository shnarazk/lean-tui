//! Tactic position finder using line/semicolon heuristics.
//!
//! Finds previous/next tactic positions. Tactics are separated by:
//! - Newlines (each non-trivial line is a tactic)
//! - Semicolons (`;` separates tactics on the same line)

use crate::tui_ipc::Position;

/// Find the position of the previous tactic before the current position.
pub fn find_previous_tactic(content: &str, current: Position) -> Option<Position> {
    let lines: Vec<&str> = content.lines().collect();
    let current_line = current.line as usize;
    let current_char = current.character as usize;

    // First check: previous tactic on same line (before semicolon)
    if let Some(line) = lines.get(current_line) {
        if let Some(pos) = find_prev_tactic_on_line(line, current_char) {
            return Some(Position {
                line: current.line,
                character: pos as u32,
            });
        }
    }

    // Second: scan backward through previous lines
    for line_idx in (0..current_line).rev() {
        let line = lines.get(line_idx).copied().unwrap_or("");
        if let Some(pos) = find_last_tactic_on_line(line) {
            return Some(Position {
                line: line_idx as u32,
                character: pos as u32,
            });
        }
    }

    None
}

/// Find the position of the next tactic after the current position.
pub fn find_next_tactic(content: &str, current: Position) -> Option<Position> {
    let lines: Vec<&str> = content.lines().collect();
    let current_line = current.line as usize;
    let current_char = current.character as usize;

    // First check: next tactic on same line (after semicolon)
    if let Some(line) = lines.get(current_line) {
        if let Some(pos) = find_next_tactic_on_line(line, current_char) {
            return Some(Position {
                line: current.line,
                character: pos as u32,
            });
        }
    }

    // Second: scan forward through following lines
    for line_idx in (current_line + 1)..lines.len() {
        let line = lines.get(line_idx).copied().unwrap_or("");
        if let Some(pos) = find_first_tactic_on_line(line) {
            return Some(Position {
                line: line_idx as u32,
                character: pos as u32,
            });
        }
    }

    None
}

/// Find the previous tactic on the same line (before a semicolon).
fn find_prev_tactic_on_line(line: &str, current_char: usize) -> Option<usize> {
    let before = &line[..current_char.min(line.len())];

    // Find the last semicolon before cursor
    let last_semi = before.rfind(';')?;

    // The previous tactic starts at the beginning of line or after the semicolon before that
    let segment_start = before[..last_semi].rfind(';').map_or(0, |i| i + 1);

    // Skip leading whitespace
    let segment = &before[segment_start..last_semi];
    let trimmed_start = segment.len() - segment.trim_start().len();

    if segment.trim().is_empty() || segment.trim().starts_with("--") {
        None
    } else {
        Some(segment_start + trimmed_start)
    }
}

/// Find the next tactic on the same line (after a semicolon).
fn find_next_tactic_on_line(line: &str, current_char: usize) -> Option<usize> {
    let after_start = current_char.min(line.len());
    let after = &line[after_start..];

    // Find the next semicolon after cursor
    let semi_offset = after.find(';')?;
    let next_start = after_start + semi_offset + 1;

    if next_start >= line.len() {
        return None;
    }

    // Skip whitespace after semicolon
    let remaining = &line[next_start..];
    let ws_len = remaining.len() - remaining.trim_start().len();
    let tactic_start = next_start + ws_len;

    let tactic = remaining.trim();
    if tactic.is_empty() || tactic.starts_with("--") {
        None
    } else {
        Some(tactic_start)
    }
}

/// Find the first tactic position on a line.
fn find_first_tactic_on_line(line: &str) -> Option<usize> {
    let trimmed = line.trim();

    // Skip empty, comment, or block comment lines
    if trimmed.is_empty() || trimmed.starts_with("--") || trimmed.starts_with("/-") {
        return None;
    }

    // Return position after leading whitespace
    Some(line.len() - line.trim_start().len())
}

/// Find the last tactic position on a line.
fn find_last_tactic_on_line(line: &str) -> Option<usize> {
    let trimmed = line.trim();

    // Skip empty, comment, or block comment lines
    if trimmed.is_empty() || trimmed.starts_with("--") || trimmed.starts_with("/-") {
        return None;
    }

    // Check if line has semicolons (multiple tactics)
    if let Some(last_semi) = line.rfind(';') {
        let after_semi = &line[last_semi + 1..];
        let ws_len = after_semi.len() - after_semi.trim_start().len();
        let tactic = after_semi.trim();

        if !tactic.is_empty() && !tactic.starts_with("--") {
            return Some(last_semi + 1 + ws_len);
        }

        // Last segment is empty/comment, find the one before
        let before_semi = &line[..last_semi];
        if let Some(prev_semi) = before_semi.rfind(';') {
            let segment = &line[prev_semi + 1..last_semi];
            let ws_len = segment.len() - segment.trim_start().len();
            if !segment.trim().is_empty() {
                return Some(prev_semi + 1 + ws_len);
            }
        } else {
            // No previous semicolon, use start of line
            let ws_len = before_semi.len() - before_semi.trim_start().len();
            if !before_semi.trim().is_empty() {
                return Some(ws_len);
            }
        }
    }

    // No semicolons, return start of content
    Some(line.len() - line.trim_start().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_lines_previous() {
        let content = "theorem foo : True := by\n  trivial\n  done";

        // From "done" (line 2), should find "trivial" (line 1)
        let result = find_previous_tactic(content, Position { line: 2, character: 2 });
        assert_eq!(result, Some(Position { line: 1, character: 2 }));

        // From "trivial" (line 1), should find "theorem" (line 0)
        let result = find_previous_tactic(content, Position { line: 1, character: 2 });
        assert_eq!(result, Some(Position { line: 0, character: 0 }));
    }

    #[test]
    fn test_simple_lines_next() {
        let content = "theorem foo : True := by\n  trivial\n  done";

        // From "theorem" (line 0), should find "trivial" (line 1)
        let result = find_next_tactic(content, Position { line: 0, character: 0 });
        assert_eq!(result, Some(Position { line: 1, character: 2 }));

        // From "trivial" (line 1), should find "done" (line 2)
        let result = find_next_tactic(content, Position { line: 1, character: 2 });
        assert_eq!(result, Some(Position { line: 2, character: 2 }));
    }

    #[test]
    fn test_no_previous_at_start() {
        let content = "trivial";
        let result = find_previous_tactic(content, Position { line: 0, character: 0 });
        assert_eq!(result, None);
    }

    #[test]
    fn test_no_next_at_end() {
        let content = "trivial";
        let result = find_next_tactic(content, Position { line: 0, character: 0 });
        assert_eq!(result, None);
    }

    #[test]
    fn test_skip_comments_and_blanks() {
        let content = "line0\n\n-- comment\nline3";

        // From line 3, should skip comment and blank, find line 0
        let result = find_previous_tactic(content, Position { line: 3, character: 0 });
        assert_eq!(result, Some(Position { line: 0, character: 0 }));

        // From line 0, should skip blank and comment, find line 3
        let result = find_next_tactic(content, Position { line: 0, character: 0 });
        assert_eq!(result, Some(Position { line: 3, character: 0 }));
    }

    #[test]
    fn test_semicolon_next() {
        let content = "  simp; ring; done";

        // From simp (char 2), next should be ring (char 8)
        let result = find_next_tactic(content, Position { line: 0, character: 2 });
        assert_eq!(result, Some(Position { line: 0, character: 8 }));

        // From ring (char 8), next should be done (char 14)
        let result = find_next_tactic(content, Position { line: 0, character: 8 });
        assert_eq!(result, Some(Position { line: 0, character: 14 }));
    }

    #[test]
    fn test_semicolon_previous() {
        let content = "  simp; ring; done";

        // From done (char 14), previous should be ring (char 8)
        let result = find_previous_tactic(content, Position { line: 0, character: 14 });
        assert_eq!(result, Some(Position { line: 0, character: 8 }));

        // From ring (char 8), previous should be simp (char 2)
        let result = find_previous_tactic(content, Position { line: 0, character: 8 });
        assert_eq!(result, Some(Position { line: 0, character: 2 }));
    }
}
