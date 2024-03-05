use lsp_types::Position;
use tree_sitter::Range;

pub trait PositionInRange {
    fn includes(&self, position: &Position) -> bool;
}

impl PositionInRange for Range {
    fn includes(&self, position: &Position) -> bool {
        self.start_point.row == position.line as usize
            && self.start_point.column <= position.character as usize
            && self.end_point.row == position.line as usize
            && self.end_point.column >= position.character as usize
    }
}
