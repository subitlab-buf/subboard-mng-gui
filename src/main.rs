use std::{collections::HashMap, fs::File, io::Read};

use chrono::DateTime;

use iced::{
    futures::TryFutureExt,
    theme,
    widget::{button, container, vertical_space, Column, Row, Scrollable, Text},
    Application, Color, Command, Font, Length,
};
use iced_aw::Split;
use serde::{Deserialize, Serialize};

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
}

impl Application for App {
    type Executor = iced::executor::Default;

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

                        #[derive(Debug, Serialize)]
                        struct Req {
                            pid: i32,
                        }

                        if let Err(err) = si
                            .client
                            .post(&si.host.process_paper)
                            .json(&Req { pid: paper })
                            .send()
                            .await
                        {
                            tracing::event!(tracing::Level::ERROR, "{err}");
                        }
                    },
                    |_| Msg::Refresh,
                );
            }
            Msg::FontLoaded(Ok(_)) => self.nerd_font = Font::with_name("Symbols Nerd Font Mono"),
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

            bar = bar.push(
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
                            let mut row = Row::new().push(
                                Text::new(&paper.name)
                                    .width(Length::Fill)
                                    .horizontal_alignment(iced::alignment::Horizontal::Left)
                                    .height(13.5)
                                    .vertical_alignment(iced::alignment::Vertical::Center),
                            );

                            row = match paper.processed {
                                Some(true) => row.push(
                                    Text::new("󰄬")
                                        .size(10)
                                        .width(15)
                                        .font(self.nerd_font)
                                        .style(Color::new(
                                            0.411_764_7,
                                            0.694_117_67,
                                            0.325_490_2,
                                            0.003_921_569,
                                        )),
                                ),
                                Some(false) => row.push(
                                    Text::new("󰅖")
                                        .size(10)
                                        .width(15)
                                        .font(self.nerd_font)
                                        .style(Color::new(
                                            0.772_549_03,
                                            0.352_941_2,
                                            0.396_078_44,
                                            0.003_921_569,
                                        )),
                                ),
                                None => row,
                            };

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
            right = right
                .push(vertical_space(15))
                .push(container(Text::new(&paper.info)).width(Length::Fill))
                .push(vertical_space(15));

            right = right
                .push(Text::new(format!("Time: {}", paper.time.to_rfc3339())))
                .push(Text::new(format!("Name: {}", paper.name)))
                .push(Text::new(format!("Email: {}", paper.email)))
                .push(vertical_space(Length::Fill));

            if paper.processed.is_some() {
                right = right.push(
                    button(
                        Text::new("Accept")
                            .horizontal_alignment(iced::alignment::Horizontal::Center),
                    )
                    .width(Length::Fill)
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
}

#[derive(Debug, Clone)]
enum Msg {
    FontLoaded(Result<(), iced::font::Error>),
    Split0Resized(u16),
    Refresh,
    RefreshDone(Vec<Paper>),
    OpenPaper(i32),
    Accept(i32),
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
