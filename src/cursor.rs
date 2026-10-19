//! A Turbo Pascal-style block cursor for iced's text editor.
//!
//! iced draws a fixed 1-pixel caret. This wrapper reads the caret rectangle
//! the editor reports for input methods (IME) on each redraw, then paints a
//! block over it: the cell filled with the text color and the character
//! under the cursor redrawn in the background color.
//!
//! The same wrapper can also reserve a column on the left for line numbers.
//! iced's editor does not say how far it has scrolled, but the caret position
//! it reports (window coordinates) together with the cursor's line gives it:
//! the numbers are laid out from the caret up and down, counting the rows each
//! line takes when it wraps.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad, Renderer as _};
use iced::advanced::text::{self, LineHeight, Paragraph, Renderer as _};
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
    gutter: Option<Gutter>,
}

/// Colors of the line-number column.
#[derive(Debug, Clone, Copy)]
pub struct Gutter {
    pub background: Color,
    pub number: Color,
    pub current: Color,
}

/// The editor's own padding (iced's default).
const PADDING: f32 = 5.0;

impl<'a, Message> BlockCursor<'a, Message> {
    pub fn new(
        editor: impl Into<Element<'a, Message>>,
        content: &'a text_editor::Content,
        color: Color,
        background: Color,
        size: f32,
    ) -> Self {
        Self { editor: editor.into(), content, color, background, size, gutter: None }
    }

    /// Shows line numbers in a column left of the editor.
    pub fn line_numbers(mut self, gutter: Option<Gutter>) -> Self {
        self.gutter = gutter;
        self
    }

    /// The width of the number column: room for the largest line number.
    fn gutter_width(&self, renderer: &Renderer) -> f32 {
        if self.gutter.is_none() {
            return 0.0;
        }
        let digits = self.content.line_count().to_string().len().max(3);
        measure_text(renderer, "0", self.size, f32::INFINITY).width * digits as f32 + 16.0
    }

    /// Paints the numbers of the lines in view. `bounds` is the whole widget,
    /// `editor` the editor's own rectangle; `caret` anchors the layout.
    fn draw_gutter(
        &self,
        gutter: Gutter,
        caret: Option<Rectangle>,
        renderer: &mut Renderer,
        bounds: Rectangle,
        editor: Rectangle,
    ) {
        let width = editor.x - bounds.x;
        let column = Rectangle::new(Point::new(bounds.x, editor.y), Size::new(width, editor.height));
        renderer.fill_quad(Quad { bounds: column, ..Quad::default() }, gutter.background);

        let size = self.size;
        let line_height = LineHeight::default().to_absolute(Pixels(size)).0;
        let text_top = editor.y + PADDING;
        let text_width = editor.width - 2.0 * PADDING;
        let digit = measure_text(renderer, "0", size, f32::INFINITY).width;
        let rows = |renderer: &Renderer, text: &str| -> usize {
            // Short lines cannot wrap: skip measuring them.
            if (text.chars().count() as f32) * size * 1.3 < text_width {
                return 1;
            }
            ((measure_text(renderer, text, size, text_width).height / line_height).round() as usize).max(1)
        };
        let line_text = |line: usize| self.content.line(line).map(|l| l.text.into_owned()).unwrap_or_default();

        // Where the cursor's line starts on screen; without a caret, line 1 is at the top.
        let cursor = self.content.cursor();
        let start = match cursor.selection {
            Some(s) if (s.line, s.column) < (cursor.position.line, cursor.position.column) => s,
            _ => cursor.position,
        };
        let (anchor, anchor_y) = match caret {
            Some(caret) => {
                let text = line_text(start.line);
                let before = text.get(..start.column).unwrap_or(&text);
                let within = if before.is_empty() { 0 } else { rows(renderer, before) - 1 };
                (start.line, caret.y - within as f32 * line_height)
            }
            None => (0, text_top),
        };

        let total = self.content.line_count();
        let bottom = editor.y + editor.height;
        let mut numbers: Vec<(usize, f32)> = Vec::new();
        let mut y = anchor_y;
        for line in anchor..total {
            if y >= bottom {
                break;
            }
            numbers.push((line, y));
            y += rows(renderer, &line_text(line)) as f32 * line_height;
        }
        let mut y = anchor_y;
        for line in (0..anchor).rev() {
            y -= rows(renderer, &line_text(line)) as f32 * line_height;
            if y + line_height <= editor.y {
                break;
            }
            numbers.push((line, y));
        }

        let font = renderer.default_font();
        for (line, y) in numbers {
            let label = (line + 1).to_string();
            let x = bounds.x + width - 8.0 - digit * label.len() as f32;
            let color = if line == cursor.position.line { gutter.current } else { gutter.number };
            renderer.fill_text(
                text::Text {
                    content: label,
                    bounds: Size::new(width, line_height),
                    size: Pixels(size),
                    line_height: LineHeight::Absolute(Pixels(line_height)),
                    font,
                    align_x: text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                },
                Point::new(x, y),
                color,
                column,
            );
        }
    }
}

#[derive(Default)]
struct State {
    /// The editor's caret, in window coordinates, when it is focused.
    caret: Option<Rectangle>,
    /// The last caret seen, for the numbers while the editor has no focus.
    last_caret: Option<Rectangle>,
}

/// The size of `content` laid out like the editor does (wrapped to `width`;
/// an infinite width means one line).
fn measure_text(renderer: &Renderer, content: &str, size: f32, width: f32) -> Size {
    <Renderer as text::Renderer>::Paragraph::with_text(text::Text {
        content,
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(size),
        line_height: LineHeight::default(),
        font: renderer.default_font(),
        align_x: text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: if width.is_finite() { text::Wrapping::default() } else { text::Wrapping::None },
    })
    .min_bounds()
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
        let gutter = self.gutter_width(renderer);
        let inner = limits.shrink(Size::new(gutter, 0.0));
        let editor = self.editor.as_widget_mut().layout(&mut tree.children[0], renderer, &inner);
        let size = Size::new(editor.size().width + gutter, editor.size().height);
        layout::Node::with_children(size, vec![editor.move_to(Point::new(gutter, 0.0))])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        // Keeps focus-by-id working for the editor inside.
        let Some(inner) = layout.children().next() else { return };
        self.editor.as_widget_mut().operate(&mut tree.children[0], inner, renderer, operation);
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
        let Some(inner) = layout.children().next() else { return };
        let redraw = matches!(event, Event::Window(window::Event::RedrawRequested(_)));
        // The editor reports its caret only on redraws, through the shell's
        // input-method slot. Empty the slot so we see exactly its report.
        let previous = redraw.then(|| std::mem::replace(shell.input_method_mut(), InputMethod::Disabled));

        self.editor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            inner,
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
            let state = tree.state.downcast_mut::<State>();
            state.caret = caret;
            if caret.is_some() {
                state.last_caret = caret;
            }
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
        let Some(inner) = layout.children().next() else { return };
        self.editor.as_widget().draw(&tree.children[0], renderer, theme, style, inner, cursor, viewport);
        let state = tree.state.downcast_ref::<State>();
        if let Some(gutter) = self.gutter {
            self.draw_gutter(gutter, state.last_caret, renderer, layout.bounds(), inner.bounds());
        }

        let focused = tree.children[0]
            .state
            .downcast_ref::<text_editor::State<highlight::Highlighter>>()
            .is_focused();
        let Some(caret) = state.caret else { return };
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
        let Some(clipped) = inner.bounds().intersection(&block) else { return };
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
        let Some(inner) = layout.children().next() else { return mouse::Interaction::None };
        self.editor.as_widget().mouse_interaction(&tree.children[0], inner, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let inner = layout.children().next()?;
        self.editor.as_widget_mut().overlay(&mut tree.children[0], inner, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a> From<BlockCursor<'a, Message>> for Element<'a, Message> {
    fn from(widget: BlockCursor<'a, Message>) -> Self {
        Element::new(widget)
    }
}
