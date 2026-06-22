//! A live editing session: the buffer plus the things the REPL needs around it —
//! an undo stack, the source path, and a dirty flag. Mutations are transactional
//! (applied to a clone, swapped in on success) so a mid-program error never leaves
//! a half-edited buffer, and the pre-image becomes the undo snapshot. Nothing reaches
//! disk until an explicit `save` (the deliberate-save trust requirement, rule 9).

use crate::ast::{Command, Statement};
use crate::errors::{Result, XledError};
use crate::exec::{self, Outcome};
use crate::io;
use crate::model::Buffer;

pub struct Session {
    pub buf: Buffer,
    undo_stack: Vec<Buffer>,
    pub source: Option<String>,
    pub dirty: bool,
}

impl Session {
    pub fn new(buf: Buffer, source: Option<String>) -> Self {
        Session {
            buf,
            undo_stack: Vec::new(),
            source,
            dirty: false,
        }
    }

    /// Apply a program. A mutating program is run on a clone and swapped in only on success,
    /// pushing the pre-image onto the undo stack. Inspect-only programs touch nothing.
    pub fn run(&mut self, program: &[Statement]) -> Result<Outcome> {
        if !program.iter().any(is_mutating) {
            return exec::run(&mut self.buf, program);
        }
        let mut working = self.buf.clone();
        let out = exec::run(&mut working, program)?; // on error, self.buf is untouched
        self.undo_stack.push(std::mem::replace(&mut self.buf, working));
        self.dirty = true;
        Ok(out)
    }

    /// Run a program against a throwaway clone and return what it would show (or the
    /// resulting table) — never commits. Any notices ride above the result as a banner:
    /// this is where the user sees a cast-skip warning *before* deciding to commit.
    pub fn preview(&self, program: &[Statement]) -> Result<String> {
        let mut working = self.buf.clone();
        let out = exec::run(&mut working, program)?;
        let body = if out.output.is_empty() {
            io::serialize(&working)?
        } else {
            out.output.join("\n")
        };
        if out.notices.is_empty() {
            Ok(body)
        } else {
            Ok(format!("{}\n{body}", out.notices.join("\n")))
        }
    }

    /// Revert the last mutation. Returns false if there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(prev) => {
                self.buf = prev;
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Write the buffer to `path` (or the source file). Sets the source on first write to a
    /// new path, and clears `dirty` when writing to the session's own source.
    pub fn save(&mut self, path: Option<&str>) -> Result<String> {
        let target = path
            .map(|s| s.to_string())
            .or_else(|| self.source.clone())
            .ok_or_else(|| {
                XledError::Correction("no file to write to — give a path: write <path>".into())
            })?;
        std::fs::write(&target, io::serialize(&self.buf)?)?;
        if self.source.is_none() {
            self.source = Some(target.clone());
        }
        if self.source.as_deref() == Some(target.as_str()) {
            self.dirty = false;
        }
        Ok(target)
    }
}

/// A statement mutates the buffer unless its command is inspect-only (`show`/`describe`/none).
fn is_mutating(st: &Statement) -> bool {
    matches!(
        st.command,
        Some(
            Command::Subst { .. }
                | Command::Assign(_)
                | Command::Del
                | Command::Crop
                | Command::Header
                | Command::Rename(_)
                | Command::Fill
                | Command::DropBlanks(_)
        )
    )
}
