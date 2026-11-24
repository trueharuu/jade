use std::io::{self, Write};

use jade_solve::fumen::encode_placements;

use crate::Decision;
use crate::DecisionKind;

impl Decision {
    /// Writes the decision tree in a simple indented format.
    ///
    /// Boards are shown as fumens produced by `encode_placements`.
    pub fn write_indented<W: Write>(
        &self,
        output: &mut W,
        indent: usize,
        field: jade_core::board::Board,
    ) -> io::Result<()> {
        let pad = "\t".repeat(indent);
        match &self.kind {
            DecisionKind::Branch {
                field: next,
                mv,
                children,
                ..
            } => {
                writeln!(output, "{}{}\t{}", pad, encode_placements(&field, &[*mv]), self.cost)?;
                for (index, child) in children.iter().enumerate() {
                    let Some(child) = child else {
                        continue;
                    };
                    if let Some(reveal) = jade_core::piece::Piece::from_u8(index as u8) {
                        writeln!(output, "{pad}{reveal}")?;
                    }
                    child.write_indented(output, indent + 1, *next)?;
                }
            }
            DecisionKind::Solve { steps } => {
                writeln!(output, "{}{}\t{}", pad, encode_placements(&field, steps), self.cost)?;
            }
            DecisionKind::Capped => {
                writeln!(output, "{}\t\t{}", pad, self.cost)?;
            }
        }
        Ok(())
    }

    /// Returns the indented format as a string.
    #[must_use]
    pub fn to_indented_string(&self, field: jade_core::board::Board) -> String {
        let mut output = Vec::new();
        self.write_indented(&mut output, 0, field)
            .expect("writing to Vec cannot fail");
        String::from_utf8(output).expect("indented output is valid UTF-8")
    }
}