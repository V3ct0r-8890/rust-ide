//! A Turbo Pascal-style block cursor for iced's text editor.
//!
//! iced draws a fixed 1-pixel caret. This wrapper reads the caret rectangle
//! the editor reports for input methods (IME) on each redraw, then paints a
//! block over it: the cell filled with the text color and the character
//! under the cursor redrawn in the background color.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad, Renderer as _};
use iced::advanced::text::{self, Paragraph, Renderer as _};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, InputMethod, Shell, Widget, overlay};
use iced::widget::text_editor;
use iced::{Color, Element, Event, Length, Pixels, Point, Rectangle, Renderer, Size, Theme, Vector, mouse, window};

use crate::highlight;

pub struct BlockCursor<'a, Message> {
    editor: Element<'a, Message>,
    content: &'a text_editor::Content,
    color: Color,
    background: Color,
    size: f32,
}

impl<'a, Message> BlockCursor<'a, Message> {
    pub fn new(
        editor: impl Into<Element<'a, Message>>,
        content: &'a text_editor::Content,
        color: Color,
        background: Color,
        size: f32,
    ) -> Self {
        Self { editor: editor.into(), content, color, background, size }
    }
}

#[derive(Default)]
struct State {
    /// The editor's caret, in window coordinates, when it is focused.
    caret: Option<Rectangle>,
}

impl<Message> Widget<Message, Theme, Renderer> for BlockCursor<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.editor)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.editor));
    }

    fn size(&self) -> Size<Length> {
        self.editor.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.editor.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        // Keeps focus-by-id working for the editor inside.
        self.editor.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let redraw = matches!(event, Event::Window(window::Event::RedrawRequested(_)));
        // The editor reports its caret only on redraws, through the shell's
        // input-method slot. Empty the slot so we see exactly its report.
        let previous = redraw.then(|| std::mem::replace(shell.input_method_mut(), InputMethod::Disabled));

        self.editor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if let Some(previous) = previous {
            let caret = match shell.input_method() {
                InputMethod::Enabled { cursor, .. } => Some(*cursor),
                InputMethod::Disabled => None,
            };
            tree.state.downcast_mut::<State>().caret = caret;
            // Put back a request made earlier in the tree (the first one wins).
            if previous.is_enabled() {
                *shell.input_method_mut() = previous;
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.editor.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);

        let focused = tree.children[0]
            .state
            .downcast_ref::<text_editor::State<highlight::Highlighter>>()
            .is_focused();
        let Some(caret) = tree.state.downcast_ref::<State>().caret else { return };
        let position = self.content.cursor();
        if !focused || position.selection.is_some_and(|s| s != position.position) {
            return; // no block while selecting: the selection shows instead
        }

        // The character under the cursor (a space at the end of a line).
        let ch = self
            .content
            .line(position.position.line)
            .and_then(|line| line.text.get(position.position.column..).and_then(|rest| rest.chars().next()))
            .filter(|c| !c.is_control())
            .unwrap_or(' ');

        let size = Pixels(self.size);
        let font = renderer.default_font();
        let measure = |content: &str| {
            <Renderer as text::Renderer>::Paragraph::with_text(text::Text {
                content,
                bounds: Size::INFINITE,
                size,
                line_height: text::LineHeight::default(),
                font,
                align_x: text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::None,
            })
            .min_bounds()
            .width
        };
        let mut width = measure(&ch.to_string());
        if width < 1.0 {
            width = measure("M"); // spaces and combining marks
        }

        let block = Rectangle::new(caret.position(), Size::new(width, caret.height));
        let Some(clipped) = layout.bounds().intersection(&block) else { return };
        renderer.fill_quad(Quad { bounds: clipped, ..Quad::default() }, self.color);
        renderer.fill_text(
            text::Text {
                content: ch.to_string(),
                bounds: Size::new(width * 2.0, caret.height),
                size,
                line_height: text::LineHeight::Absolute(Pixels(caret.height)),
                font,
                align_x: text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::None,
            },
            Point::new(caret.x, caret.y),
            self.background,
            clipped,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.editor.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.editor.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a> From<BlockCursor<'a, Message>> for Element<'a, Message> {
    fn from(widget: BlockCursor<'a, Message>) -> Self {
        Element::new(widget)
    }
}
