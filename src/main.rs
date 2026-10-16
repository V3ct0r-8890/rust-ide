//! rust-ide: a Turbo Pascal–style IDE for Rust and Python (iced GUI front end).

use iced::widget::{column, container, row, text, text_editor};
use iced::{Background, Color, Element, Font, Length, Theme};

const TURBO_BLUE: Color = Color::from_rgb(0.0, 0.0, 0.66);
const TURBO_GRAY: Color = Color::from_rgb(0.75, 0.75, 0.75);
const TURBO_YELLOW: Color = Color::from_rgb(1.0, 1.0, 0.33);

fn main() -> iced::Result {
    iced::application(Ide::default, Ide::update, Ide::view)
        .title("Rust IDE")
        .theme(|_: &Ide| Theme::Dark)
        .default_font(Font::MONOSPACE)
        .run()
}

#[derive(Default)]
struct Ide {
    content: text_editor::Content,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
}

impl Ide {
    fn update(&mut self, message: Message) {
        match message {
            Message::Edit(action) => self.content.perform(action),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let menu_bar = container(
            row(["File", "Edit", "Search", "Run", "Compile", "Debug", "Options", "Window", "Help"]
                .into_iter()
                .map(|item| text(item).color(Color::BLACK).into()))
            .spacing(20),
        )
        .padding([2, 10])
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_GRAY));

        let editor = text_editor(&self.content)
            .on_action(Message::Edit)
            .height(Length::Fill)
            .style(|theme, status| text_editor::Style {
                background: Background::Color(TURBO_BLUE),
                value: TURBO_YELLOW,
                ..text_editor::default(theme, status)
            });

        let (line, col) = {
            let cursor = self.content.cursor();
            (cursor.position.line + 1, cursor.position.column + 1)
        };
        let status_bar = container(
            text(format!(
                " F1 Help  F2 Save  F3 Open  F5 Run  Alt+X Exit        {line}:{col}"
            ))
            .color(Color::BLACK),
        )
        .padding([2, 10])
        .width(Length::Fill)
        .style(|_| container::Style::default().background(TURBO_GRAY));

        column![menu_bar, editor, status_bar].into()
    }
}
