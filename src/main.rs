use std::{fs::File, io::Read};

use chrono::DateTime;

use iced::{
    futures::TryFutureExt,
    theme,
    widget::{button, horizontal_space, Column, Row, Space, Text},
    Application, Command, Font, Length,
};
use iced_aw::Split;
use serde::Deserialize;

fn main() -> iced::Result {
    let config: Config;

    {
        let mut str = String::new();
        let mut file = File::open("config.toml").expect("configuration file config.toml not found");
        file.read_to_string(&mut str).unwrap();
        config = toml::from_str(&str).unwrap();
    }

    App::run(iced::Settings {
        window: iced::window::Settings {
            size: (500, 800),
            ..Default::default()
        },
        default_font: Font::with_name(config.font.to_owned().leak()),
        flags: config,
        default_text_size: 11.5,
        ..Default::default()
    })
}

/// Configuration file abstraction.
#[derive(Deserialize, Debug, Default)]
struct Config {
    host_url: String,

    /// `@RequestMapping("xxx")`.
    global_mapping: String,
    /// `@GetMapping("xxx")`.
    paper_need_process_mapping: String,
    /// `@PostMapping("xxx")`.
    process_paper_mapping: String,

    font: String,
}

#[derive(Debug)]
struct BuiltHost {
    paper_need_process: String,
    process_paper: String,
}
#[derive(Debug)]
struct StaticIns {
    config: Config,
    host: BuiltHost,
    client: reqwest::Client,
}

#[derive(Debug)]
struct App {
    /// Loaded papers.
    papers: Vec<Paper>,

    static_ins: &'static StaticIns,

    split_0_pos: Option<u16>,
}

impl Application for App {
    type Executor = iced::executor::Default;

    type Message = Msg;

    type Theme = iced::Theme;

    type Flags = Config;

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            Self {
                papers: vec![],

                static_ins: Box::leak(Box::new(StaticIns {
                    host: BuiltHost {
                        paper_need_process: format!(
                            "{}{}/{}",
                            flags.host_url, flags.global_mapping, flags.paper_need_process_mapping
                        ),
                        process_paper: format!(
                            "{}{}/{}",
                            flags.host_url, flags.global_mapping, flags.process_paper_mapping
                        ),
                    },

                    config: flags,
                    client: reqwest::Client::new(),
                })),

                split_0_pos: None,
            },
            iced::font::load(include_bytes!("../fonts/SymbolsNerdFontMono-Regular.ttf").as_slice())
                .map(Msg::FontLoaded),
        )
    }

    fn title(&self) -> String {
        "SubBoard GUI".to_string()
    }

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Msg::Split0Resized(s) => self.split_0_pos = Some(s),
            Msg::Refresh => {
                return Command::perform(
                    async {
                        let span = tracing::span!(tracing::Level::INFO, "refresh papers");
                        let _ = span.enter();

                        Msg::RefreshDone(
                            self.static_ins
                                .client
                                .get(&self.static_ins.host.paper_need_process)
                                .send()
                                .and_then(|res| res.json())
                                .unwrap_or_else(|err| {
                                    tracing::event!(tracing::Level::ERROR, "{err}");
                                    vec![]
                                })
                                .await,
                        )
                    },
                    std::convert::identity,
                )
            }
            _ => (),
        }

        iced::Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        let mut left = Column::new();

        {
            let mut bar = Row::new().height(13);

            bar = bar.push(
                button(Text::new("󰑐").width(15).height(15))
                    .style(theme::Button::Text)
                    .on_press(Msg::Refresh),
            );
            bar = bar.push(horizontal_space(Length::Fill));

            left = left.push(bar);
        }

        let right = Space::new(Length::Fill, Length::Fill);

        Split::new(
            left,
            right,
            self.split_0_pos,
            iced_aw::split::Axis::Horizontal,
            Msg::Split0Resized,
        )
        .into()
    }
}

#[derive(Debug, Clone)]
enum Msg {
    FontLoaded(Result<(), iced::font::Error>),
    Split0Resized(u16),
    Refresh,
    RefreshDone(Vec<Paper>),
}

#[derive(Debug, Deserialize, Clone)]
struct Paper {
    pid: i32,
    info: String,
    time: DateTime<chrono::Local>,
    name: String,
    email: String,

    #[serde(default)]
    processed: bool,
}
