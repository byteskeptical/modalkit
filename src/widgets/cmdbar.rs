//! # Command bar
//!
//! ## Overview
//!
//! These components allow creating a bar for entering searches and commands.

//! Typically, this widget is used indirectly by consumers through [Screen], which places this at
//! the bottom of the terminal window.
//!
//! [Screen]: super::screen::Screen
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use tui::{buffer::Buffer, layout::Rect, text::Span, widgets::StatefulWidget};

use crate::editing::{
    action::{
        Action,
        CommandAction,
        CommandBarAction,
        EditAction,
        EditResult,
        PromptAction,
        Promptable,
    },
    application::ApplicationInfo,
    base::{CommandType, Count, EditTarget, MoveDir1D, MoveDirMod, SearchType},
    context::EditContext,
    history::ScrollbackState,
    rope::EditRope,
    store::Store,
};

use super::{
    textbox::{TextBox, TextBoxState},
    PromptActions,
};

/// Persistent state for rendering [CommandBar].
pub struct CommandBarState<I: ApplicationInfo> {
    scrollback: ScrollbackState,
    cmdtype: CommandType,
    tbox: TextBoxState<I>,
}

impl<I> CommandBarState<I>
where
    I: ApplicationInfo,
{
    /// Create state for a [CommandBar] widget.
    pub fn new(id: I::ContentId, store: &mut Store<I>) -> Self {
        let buffer = store.load_buffer(id);

        CommandBarState {
            scrollback: ScrollbackState::Pending,
            cmdtype: CommandType::Command,
            tbox: TextBoxState::new(buffer),
        }
    }

    /// Set the type of command that the bar is being used for.
    pub fn set_type(&mut self, ct: CommandType) {
        self.cmdtype = ct;
    }

    /// Reset the contents of the bar, and return the contents as an [EditRope].
    pub fn reset(&mut self) -> EditRope {
        self.scrollback = ScrollbackState::Pending;

        self.tbox.reset()
    }

    /// Reset the contents of the bar, and return the contents as a [String].
    pub fn reset_text(&mut self) -> String {
        self.reset().to_string()
    }
}

impl<I> Deref for CommandBarState<I>
where
    I: ApplicationInfo,
{
    type Target = TextBoxState<I>;

    fn deref(&self) -> &Self::Target {
        &self.tbox
    }
}

impl<I> DerefMut for CommandBarState<I>
where
    I: ApplicationInfo,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tbox
    }
}

impl<C, I> PromptActions<C, Store<I>, I> for CommandBarState<I>
where
    C: Default + EditContext,
    I: ApplicationInfo,
{
    fn submit(&mut self, ctx: &C, store: &mut Store<I>) -> EditResult<Vec<(Action<I>, C)>, I> {
        let unfocus = CommandBarAction::Unfocus.into();

        let action = match self.cmdtype {
            CommandType::Command => {
                let rope = self.reset();
                let text = rope.to_string();

                store.set_last_cmd(rope);

                CommandAction::Execute(text).into()
            },
            CommandType::Search(_, _) => {
                let text = self.reset().trim();

                store.set_last_search(text);

                let target =
                    EditTarget::Search(SearchType::Regex, MoveDirMod::Same, Count::Contextual);

                Action::Edit(EditAction::Motion.into(), target)
            },
        };

        Ok(vec![(unfocus, ctx.clone()), (action, ctx.clone())])
    }

    fn abort(
        &mut self,
        _empty: bool,
        ctx: &C,
        store: &mut Store<I>,
    ) -> EditResult<Vec<(Action<I>, C)>, I> {
        // We always unfocus currently, regardless of whether _empty=true.
        let act = Action::CommandBar(CommandBarAction::Unfocus).into();

        let text = self.reset().trim();

        match self.cmdtype {
            CommandType::Search(_, _) => {
                store.set_aborted_search(text);
            },
            CommandType::Command => {
                store.set_aborted_cmd(text);
            },
        }

        Ok(vec![(act, ctx.clone())])
    }

    fn recall(
        &mut self,
        dir: &MoveDir1D,
        count: &Count,
        ctx: &C,
        store: &mut Store<I>,
    ) -> EditResult<Vec<(Action<I>, C)>, I> {
        let count = ctx.resolve(count);
        let rope = self.tbox.get();

        let text = match self.cmdtype {
            CommandType::Search(_, _) => {
                store.searches.recall(&rope, &mut self.scrollback, *dir, count)
            },
            CommandType::Command => store.commands.recall(&rope, &mut self.scrollback, *dir, count),
        };

        if let Some(text) = text {
            self.set_text(text);
        }

        Ok(vec![])
    }
}

impl<'a, C, I> Promptable<C, Store<I>, I> for CommandBarState<I>
where
    C: Default + EditContext,
    I: ApplicationInfo,
{
    fn prompt(
        &mut self,
        act: &PromptAction,
        ctx: &C,
        store: &mut Store<I>,
    ) -> EditResult<Vec<(Action<I>, C)>, I> {
        match act {
            PromptAction::Abort(empty) => self.abort(*empty, ctx, store),
            PromptAction::Recall(dir, count) => self.recall(dir, count, ctx, store),
            PromptAction::Submit => self.submit(ctx, store),
        }
    }
}

/// Widget for rendering a command bar.
pub struct CommandBar<'a, I: ApplicationInfo> {
    focused: bool,
    message: Option<Span<'a>>,

    _pc: PhantomData<I>,
}

impl<'a, I> CommandBar<'a, I>
where
    I: ApplicationInfo,
{
    /// Create a new widget.
    pub fn new() -> Self {
        CommandBar { focused: false, message: None, _pc: PhantomData }
    }

    /// Indicate whether the widget is currently focused.
    pub fn focus(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set a status string that will be displayed instead of the contents when the widget is not
    /// currently focused.
    pub fn status(mut self, msg: Option<Span<'a>>) -> Self {
        self.message = msg;
        self
    }
}

impl<'a, I> StatefulWidget for CommandBar<'a, I>
where
    I: ApplicationInfo,
{
    type State = CommandBarState<I>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if self.focused {
            let prompt = match state.cmdtype {
                CommandType::Command => ":",
                CommandType::Search(MoveDir1D::Next, _) => "/",
                CommandType::Search(MoveDir1D::Previous, _) => "?",
            };

            let tbox = TextBox::new().prompt(prompt);

            tbox.render(area, buf, &mut state.tbox);
        } else if let Some(span) = self.message {
            buf.set_span(area.left(), area.top(), &span, area.width);
        }
    }
}

impl<'a, I> Default for CommandBar<'a, I>
where
    I: ApplicationInfo,
{
    fn default() -> Self {
        CommandBar::new()
    }
}
