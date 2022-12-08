use std::collections::HashMap;

use bitflags::bitflags;

use crate::{
    editing::base::TargetShape::{BlockWise, CharWise, LineWise},
    editing::base::{Register, TargetShape},
    editing::rope::EditRope,
};

bitflags! {
    /// Flags that control the behaviour of [RegisterStore::put].
    pub struct RegisterPutFlags: u32 {
        /// No flags set.
        const NONE = 0b00000000;

        /// Append contents to register.
        const APPEND = 0b00000001;

        /// The value being put came from deleting text.
        const DELETE = 0b00000010;

        /// This is not a normal text register update.
        ///
        /// This will skip setting [Register::Unnamed] to have the same value as the updated
        /// register.
        const NOTEXT = 0b00000100;
    }
}

/// The current values mapped to by a [Register].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterCell {
    /// The shape of the text within the [Register].
    pub shape: TargetShape,

    /// The actual stored text.
    pub value: EditRope,
}

/// Storage for [Register] values.
///
/// Registers are used to save different types of values during editing:
///
/// - Recently cut and copied text (see [EditAction::Delete] and [EditAction::Yank])
/// - Last used commands, searches and substitution patterns
/// - Recorded macros (see [MacroAction::ToggleRecording])
///
/// [EditAction::Delete]: crate::editing::action::EditAction::Delete
/// [EditAction::Yank]: crate::editing::action::EditAction::Yank
/// [MacroAction::ToggleRecording]: crate::editing::action::MacroAction::ToggleRecording
pub struct RegisterStore {
    altbufname: RegisterCell,
    curbufname: RegisterCell,

    last_command: RegisterCell,
    last_inserted: RegisterCell,
    last_search: RegisterCell,
    last_yanked: RegisterCell,
    last_deleted: Vec<RegisterCell>,
    last_macro: Option<Register>,

    small_delete: RegisterCell,

    unnamed: RegisterCell,
    unnamed_macro: RegisterCell,
    named: HashMap<char, RegisterCell>,
}

impl RegisterCell {
    /// Create a new cell.
    pub fn new(shape: TargetShape, value: EditRope) -> Self {
        RegisterCell { shape, value }
    }

    /// Merge the contents of two register cells, respecting their shapes.
    pub fn merge(&self, other: &RegisterCell) -> RegisterCell {
        match (self.shape, other.shape) {
            (CharWise, CharWise) | (LineWise, LineWise) | (CharWise, BlockWise) => {
                /*
                 * These combinations keep the existing cell's shape, and don't
                 * need any additional characters when merged:
                 *
                 * Char + Char = Char
                 * Char + Block = Char
                 * Line + Line = Line
                 */
                let text = self.value.clone() + other.value.clone();
                RegisterCell::new(self.shape, text)
            },
            (BlockWise, BlockWise | CharWise) => {
                /*
                 * This combination keeps the existing cell's shape, and needs
                 * a newline added in between the contents:
                 *
                 * Block + Block = Block
                 * Block + Char = Block
                 */
                let text = self.value.clone() + EditRope::from("\n") + other.value.clone();
                RegisterCell::new(self.shape, text)
            },
            (BlockWise | CharWise, LineWise) => {
                /*
                 * These combinations take on the appended cell's shape, and need
                 * a newline added in between the contents:
                 *
                 * Block + Line = Line
                 * Char + Line = Line
                 */
                let text = self.value.clone() + EditRope::from("\n") + other.value.clone();
                RegisterCell::new(other.shape, text)
            },
            (LineWise, BlockWise | CharWise) => {
                /*
                 * These combinations keep the existing cell's shape, and need
                 * a newline appended after the contents:
                 *
                 * Line + Block = Line
                 * Line + Char = Line
                 */
                let text = self.value.clone() + other.value.clone() + EditRope::from("\n");
                RegisterCell::new(self.shape, text)
            },
        }
    }
}

impl Default for RegisterCell {
    fn default() -> RegisterCell {
        RegisterCell::new(TargetShape::CharWise, EditRope::from(""))
    }
}

impl From<&str> for RegisterCell {
    fn from(s: &str) -> RegisterCell {
        RegisterCell::new(TargetShape::CharWise, EditRope::from(s))
    }
}

impl From<EditRope> for RegisterCell {
    fn from(s: EditRope) -> RegisterCell {
        RegisterCell::new(TargetShape::CharWise, s)
    }
}

impl From<(TargetShape, &str)> for RegisterCell {
    fn from(c: (TargetShape, &str)) -> RegisterCell {
        RegisterCell::new(c.0, EditRope::from(c.1))
    }
}

impl RegisterStore {
    fn new() -> Self {
        RegisterStore {
            altbufname: RegisterCell::default(),
            curbufname: RegisterCell::default(),

            last_command: RegisterCell::default(),
            last_inserted: RegisterCell::default(),
            last_search: RegisterCell::default(),
            last_yanked: RegisterCell::default(),
            last_deleted: vec![RegisterCell::default(); 9],
            last_macro: None,

            small_delete: RegisterCell::default(),

            unnamed: RegisterCell::default(),
            unnamed_macro: RegisterCell::default(),
            named: HashMap::new(),
        }
    }

    fn _push_deleted(&mut self, cell: RegisterCell) {
        if cell.value.get_lines() < 1 {
            self.small_delete = cell.clone();
        } else {
            self.last_deleted.insert(0, cell);
            self.last_deleted.truncate(9);
        }
    }

    /// Get the current value of a [Register].
    ///
    /// If none is specified, this returns the value of [Register::Unnamed].
    pub fn get(&self, reg: &Register) -> RegisterCell {
        match reg {
            Register::Unnamed => self.unnamed.clone(),
            Register::UnnamedMacro => self.unnamed_macro.clone(),
            Register::UnnamedCursorGroup => RegisterCell::default(),
            Register::RecentlyDeleted(off) => {
                self.last_deleted.get(*off).cloned().unwrap_or_default()
            },
            Register::SmallDelete => self.small_delete.clone(),
            Register::Named(name) => self.named.get(&name).cloned().unwrap_or_default(),
            Register::AltBufName => self.altbufname.clone(),
            Register::LastSearch => self.last_search.clone(),
            Register::LastYanked => self.last_yanked.clone(),

            /*
             * Operating system clipboards.
             */
            Register::SelectionPrimary => {
                // XXX: implement
                RegisterCell::default()
            },
            Register::SelectionClipboard => {
                // XXX: implement
                RegisterCell::default()
            },

            /*
             * Read-only registers.
             */
            Register::CurBufName => self.curbufname.clone(),
            Register::LastCommand => self.last_command.clone(),
            Register::LastInserted => self.last_inserted.clone(),

            /*
             * Blackhole register.
             */
            Register::Blackhole => RegisterCell::default(),
        }
    }

    /// Update the current value of a [Register] with `cell`. If none is specified, this updates
    /// the value of [Register::Unnamed].
    ///
    /// The `append` flag controls whether this should wholly replace or append to the current
    /// value.
    ///
    /// The `del` flag indicates whether this register update is being done as part of a text
    /// deletion in a document.
    pub fn put(&mut self, reg: &Register, mut cell: RegisterCell, flags: RegisterPutFlags) {
        if flags.contains(RegisterPutFlags::APPEND) {
            cell = self.get(reg).merge(&cell)
        }

        /*
         * For each register, take care of storing the value with the correct behaviour. If we're
         * storing the blackhole register, we don't do anything. For all other registers, we update
         * the unnamed ("") register with the exact same value.
         */
        let unnamed = match reg {
            Register::Blackhole => return,
            Register::UnnamedCursorGroup => return,

            Register::Unnamed => {
                if flags.contains(RegisterPutFlags::DELETE) {
                    self._push_deleted(cell.clone());
                } else {
                    self.last_yanked = cell.clone();
                }
                cell
            },
            Register::UnnamedMacro => {
                self.unnamed_macro = cell.clone();
                cell
            },
            Register::Named(name) => {
                self.named.insert(*name, cell.clone());
                cell
            },

            Register::RecentlyDeleted(off) => {
                if let Some(elem) = self.last_deleted.get_mut(*off) {
                    *elem = cell.clone();
                }

                cell
            },
            Register::SmallDelete => {
                self.small_delete = cell.clone();
                cell
            },

            Register::AltBufName => {
                self.altbufname = cell.clone();
                cell
            },
            Register::LastYanked => {
                self.last_yanked = cell.clone();
                cell
            },

            /*
             * Operating system clipboards.
             */
            Register::SelectionPrimary => {
                // XXX: implement
                cell
            },
            Register::SelectionClipboard => {
                // XXX: implement
                cell
            },

            /*
             * Read-only registers don't write anywhere.
             */
            Register::CurBufName => cell,
            Register::LastCommand => cell,
            Register::LastInserted => cell,
            Register::LastSearch => cell,
        };

        if !flags.contains(RegisterPutFlags::NOTEXT) {
            self.unnamed = unnamed;
        }
    }

    /// Return the contents of a register for macro execution.
    pub fn get_macro(&mut self, reg: Register) -> EditRope {
        self.last_macro = Some(reg);

        return self.get(&reg).value;
    }

    /// Return the same contents as the last call to [RegisterStore::get_macro].
    pub fn get_last_macro(&self) -> Option<EditRope> {
        if let Some(ref reg) = self.last_macro {
            return Some(self.get(reg).value);
        } else {
            return None;
        }
    }

    pub(super) fn set_last_cmd<T: Into<EditRope>>(&mut self, rope: T) {
        self.last_command = RegisterCell::from(rope.into());
    }

    pub(super) fn set_last_search<T: Into<EditRope>>(&mut self, rope: T) {
        self.last_search = RegisterCell::from(rope.into());
    }
}

impl Default for RegisterStore {
    fn default() -> RegisterStore {
        RegisterStore::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_merge() {
        let a = RegisterCell::new(CharWise, EditRope::from("a"));
        let b = RegisterCell::new(BlockWise, EditRope::from("1\n2"));
        let c = RegisterCell::new(LineWise, EditRope::from("q\nr\ns\n"));

        // Test appending to a CharWise cell.
        assert_eq!(a.merge(&a), RegisterCell::new(CharWise, EditRope::from("aa")));
        assert_eq!(a.merge(&b), RegisterCell::new(CharWise, EditRope::from("a1\n2")));
        assert_eq!(a.merge(&c), RegisterCell::new(LineWise, EditRope::from("a\nq\nr\ns\n")));

        // Test appending to a BlockWise cell.
        assert_eq!(b.merge(&a), RegisterCell::new(BlockWise, EditRope::from("1\n2\na")));
        assert_eq!(b.merge(&b), RegisterCell::new(BlockWise, EditRope::from("1\n2\n1\n2")));
        assert_eq!(b.merge(&c), RegisterCell::new(LineWise, EditRope::from("1\n2\nq\nr\ns\n")));

        // Test appending to a LineWise cell.
        assert_eq!(c.merge(&a), RegisterCell::new(LineWise, EditRope::from("q\nr\ns\na\n")));
        assert_eq!(c.merge(&b), RegisterCell::new(LineWise, EditRope::from("q\nr\ns\n1\n2\n")));
        assert_eq!(c.merge(&c), RegisterCell::new(LineWise, EditRope::from("q\nr\ns\nq\nr\ns\n")));
    }
}
