use std::{collections::HashMap, fs::File, io::Read};

use chrono::DateTime;

use hex_color::HexColor;
use iced::{
    color,
    futures::TryFutureExt,
    theme,
    widget::{button, container, vertical_space, Column, Row, Scrollable, Text},
    Application, Color, Command, Font, Length,
};
use iced_aw::Split;
use serde::Deserialize;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let config: Config;

    {
        let mut str = String::new();
        let mut file = File::open("config.toml").expect("configuration file config.toml not found");
        file.read_to_string(&mut str).unwrap();
        config = toml::from_str(&str).unwrap();
    }

    App::run(iced::Settings {
        window: iced::window::Settings {
            size: (1200, 800),
            ..Default::default()
        },
        default_font: Font::with_name(config.font.to_owned().leak()),
        flags: config,
        default_text_size: 15.0,
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
    host: BuiltHost,
    client: reqwest::Client,
}

#[derive(Debug)]
struct App {
    /// Loaded papers.
    papers: HashMap<i32, Paper>,
    static_ins: &'static StaticIns,

    split_0_pos: Option<u16>,
    selected_paper: Option<i32>,
    nerd_font: Font,
    dark_mode: bool,
}

impl Application for App {
    type Executor = iced_futures::backend::native::tokio::Executor;

    type Message = Msg;

    type Theme = iced::Theme;

    type Flags = Config;

    fn new(flags: Self::Flags) -> (Self, iced::Command<Self::Message>) {
        (
            Self {
                papers: HashMap::new(),

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

                    client: reqwest::Client::new(),
                })),

                split_0_pos: Some(200),
                selected_paper: None,
                nerd_font: Font::MONOSPACE,
                dark_mode: false,
            },
            iced::font::load(include_bytes!("../fonts/SymbolsNerdFontMono-Regular.ttf").as_slice())
                .map(Msg::FontLoaded),
        )
    }

    #[inline]
    fn title(&self) -> String {
        format!(
            "{}SubBoard GUI",
            if let Some(value) = self.selected_paper.and_then(|v| self.papers.get(&v)) {
                format!("{} - ", value.info)
            } else {
                Default::default()
            }
        )
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
            Msg::RefreshDone(papers) => {
                for paper in papers {
                    self.papers.insert(paper.pid, paper);
                }
            }
            Msg::OpenPaper(paper) => self.selected_paper = Some(paper),
            Msg::Accept(paper) => {
                let si = self.static_ins;
                return Command::perform(
                    async move {
                        let span = tracing::span!(tracing::Level::INFO, "accept paper {paper}");
                        let _ = span.enter();

                        if let Err(err) = si
                            .client
                            .post(&si.host.process_paper)
                            .query(&[("pid", paper)])
                            .send()
                            .await
                        {
                            tracing::event!(tracing::Level::ERROR, "{err}");
                            false
                        } else {
                            true
                        }
                    },
                    move |p| Msg::Accepted(paper, p),
                );
            }
            Msg::FontLoaded(Ok(_)) => self.nerd_font = Font::with_name("Symbols Nerd Font Mono"),
            Msg::Accepted(paper, p) => {
                if let Some(value) = self.papers.get_mut(&paper) {
                    value.processed = Some(p)
                }
                return Command::perform(async {}, |_| Msg::Refresh);
            }
            Msg::ToggleDarkMode => {
                self.dark_mode = !self.dark_mode;
            }
            _ => (),
        }

        iced::Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        let mut left = Column::new();

        {
            let mut bar = Row::new().height(30).width(Length::Fill);

            bar = bar.push(
                Text::new("   PAPERS")
                    .height(30)
                    .width(Length::Fill)
                    .horizontal_alignment(iced::alignment::Horizontal::Left)
                    .vertical_alignment(iced::alignment::Vertical::Center)
                    .style(Color::new(0.5, 0.5, 0.5, 1.0)),
            );

            bar = bar
                .push(
                    button(
                        Text::new("󰃝")
                            .width(30)
                            .height(30)
                            .size(13.5)
                            .horizontal_alignment(iced::alignment::Horizontal::Center)
                            .style(Color::new(0.5, 0.5, 0.5, 1.0))
                            .font(self.nerd_font),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::ToggleDarkMode),
                )
                .push(
                    button(
                        Text::new("󰑐")
                            .width(30)
                            .height(30)
                            .size(13.5)
                            .horizontal_alignment(iced::alignment::Horizontal::Center)
                            .style(Color::new(0.5, 0.5, 0.5, 1.0))
                            .font(self.nerd_font),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::Refresh),
                );

            left = left.push(bar);
        }

        {
            let mut down = Column::new().width(Length::Fill);

            let mut papers: Vec<&Paper> = self.papers.values().collect();
            papers.sort_unstable_by_key(|paper| &paper.time);

            for paper in papers {
                down = down.push(
                    button(
                        container({
                            let mut row = Row::new().height(18.5).push(
                                Text::new(format!(" {}", paper.name))
                                    .width(Length::Fill)
                                    .horizontal_alignment(iced::alignment::Horizontal::Left)
                                    .height(18.5)
                                    .vertical_alignment(iced::alignment::Vertical::Center),
                            );

                            if let Some(p) = paper.processed {
                                row = row.push(
                                    Text::new("󰧞")
                                        .size(10)
                                        .width(15)
                                        .height(18.5)
                                        .vertical_alignment(iced::alignment::Vertical::Center)
                                        .font(self.nerd_font)
                                        .style(if p {
                                            self.theme().palette().success
                                        } else {
                                            self.theme().palette().danger
                                        }),
                                );
                            }

                            row
                        })
                        .style(
                            if self.selected_paper.map_or(false, |e| paper.pid == e) {
                                theme::Container::Box
                            } else {
                                theme::Container::Transparent
                            },
                        ),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::OpenPaper(paper.pid)),
                )
            }

            left = left.push(Scrollable::new(down).height(Length::Fill));
        }

        let mut right = Column::new().height(Length::Fill).width(Length::Fill);
        if let Some(paper) = self
            .selected_paper
            .and_then(|value| self.papers.get(&value))
        {
            let hex_color = HexColor::parse_rgb(&paper.color).unwrap_or_default();

            right = right
                .push(vertical_space(15))
                .push(
                    container(Text::new(format!(" {} ", paper.info)).size(16.5))
                        .style(theme::Container::Custom(Box::new(move |_: &_| {
                            iced::widget::container::Appearance {
                                text_color: Some(color!(000000)),
                                background: Some(iced::Background::Color(Color::from_rgb8(
                                    hex_color.r,
                                    hex_color.g,
                                    hex_color.b,
                                ))),
                                border_radius: Default::default(),
                                border_width: 0.,
                                border_color: Default::default(),
                            }
                        })))
                        .width(Length::Fill),
                )
                .push(vertical_space(15));

            right = right
                .push(Text::new(format!("Time: {}", paper.time.to_rfc2822())))
                .push(Text::new(format!("Name: {}", paper.name)))
                .push(Text::new(format!("Email: {}", paper.email)))
                .push(vertical_space(Length::Fill));

            if paper.processed.is_none() {
                right = right.push(
                    button(
                        Text::new("Accept")
                            .horizontal_alignment(iced::alignment::Horizontal::Center),
                    )
                    .width(Length::Fill)
                    .style(theme::Button::Positive)
                    .on_press(Msg::Accept(paper.pid)),
                );
            }
        }

        Split::new(
            left,
            right,
            self.split_0_pos,
            iced_aw::split::Axis::Vertical,
            Msg::Split0Resized,
        )
        .into()
    }

    #[inline]
    fn theme(&self) -> Self::Theme {
        if self.dark_mode {
            iced::Theme::Dark
        } else {
            iced::Theme::Light
        }
    }
}

#[derive(Debug, Clone)]
enum Msg {
    FontLoaded(Result<(), iced::font::Error>),
    Split0Resized(u16),
    Refresh,
    RefreshDone(Vec<Paper>),
    OpenPaper(i32),
    Accept(i32),
    Accepted(i32, bool),
    ToggleDarkMode,
}

#[derive(Debug, Deserialize, Clone)]
struct Paper {
    pid: i32,
    info: String,
    time: DateTime<chrono::Local>,
    name: String,
    email: String,
    color: String,

    #[serde(default)]
    processed: Option<bool>,
}
