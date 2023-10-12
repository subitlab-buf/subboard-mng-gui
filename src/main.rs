use std::{collections::HashMap, fs::File, io::Read, sync::Arc, time::Duration};

use chrono::DateTime;

use hex_color::HexColor;
use iced::{
    color,
    futures::TryFutureExt,
    keyboard::KeyCode,
    theme,
    widget::{button, container, horizontal_space, vertical_space, Column, Row, Scrollable, Text},
    Application, Color, Command, Font, Length,
};
use iced_aw::Split;
use serde::Deserialize;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
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
    related_papers: (Option<i32>, Option<i32>),
    nerd_font: Font,
    dark_mode: bool,
    split_axis: iced_aw::split::Axis,
    display_bg: bool,

    refresh_count: Arc<()>,
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
                split_0_pos: Some(250),
                selected_paper: None,
                related_papers: (None, None),
                nerd_font: Font::MONOSPACE,
                dark_mode: false,
                split_axis: iced_aw::split::Axis::Vertical,
                display_bg: true,
                refresh_count: Arc::new(()),
            },
            Command::batch([
                Command::perform(async {}, |_| Msg::RefreshLoop(Duration::ZERO)),
                iced::font::load(
                    include_bytes!("../fonts/SymbolsNerdFontMono-Regular.ttf").as_slice(),
                )
                .map(Msg::FontLoaded),
            ]),
        )
    }

    #[inline]
    fn title(&self) -> String {
        format!(
            "SubBoard{}",
            if let Some(value) = self.selected_paper.and_then(|v| self.papers.get(&v)) {
                format!(" - Paper from {}", value.name)
            } else {
                Default::default()
            }
        )
    }

    fn update(&mut self, message: Self::Message) -> iced::Command<Self::Message> {
        match message {
            Msg::Split0Resized(s) => self.split_0_pos = Some(s),
            Msg::Refresh => {
                let arc = self.refresh_count.clone();
                return Command::perform(
                    async {
                        let _: Arc<_> = arc;
                        let span = tracing::span!(tracing::Level::INFO, "refresh papers");
                        tracing::event!(tracing::Level::INFO, "refreshing papers");
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
                );
            }
            Msg::RefreshLoop(duration) => {
                let weak = Arc::downgrade(&self.refresh_count);
                return Command::perform(
                    async move {
                        tokio::time::sleep(duration).await;
                        weak.strong_count() == 1
                    },
                    |p| {
                        if p {
                            Msg::Multi(vec![
                                Msg::Refresh,
                                Msg::RefreshLoop(Duration::from_secs(45)),
                            ])
                        } else {
                            Msg::RefreshLoop(Duration::from_secs(30))
                        }
                    },
                );
            }
            Msg::RefreshDone(papers) => {
                for paper in papers {
                    self.papers.insert(paper.pid, paper);
                }
            }
            Msg::OpenPaper {
                before,
                target,
                after,
            } => {
                self.selected_paper = Some(target);
                self.related_papers = (before, after);
                self.display_bg = true
            }
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
            Msg::SwitchSplitAxis => {
                self.split_axis = match self.split_axis {
                    iced_aw::split::Axis::Horizontal => iced_aw::split::Axis::Vertical,
                    iced_aw::split::Axis::Vertical => iced_aw::split::Axis::Horizontal,
                }
            }
            Msg::ToggleBg => self.display_bg = !self.display_bg,
            Msg::CleanAccepted => self.papers.retain(|_, v| v.processed.is_none()),
            Msg::Multi(vec) => {
                let mut commands = Vec::with_capacity(vec.len());
                for msg in vec {
                    commands.push(self.update(msg));
                }
                return Command::batch(commands);
            }
            Msg::Event(iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key_code,
                ..
            })) => match key_code {
                KeyCode::Up | KeyCode::K => {
                    if let Some((v1, v2)) = self.selected_paper.zip(self.related_papers.0) {
                        let mut papers: Vec<&Paper> = self.papers.values().collect();
                        papers.sort_unstable_by_key(|paper| &paper.time);
                        papers.reverse();
                        return self.update(Msg::OpenPaper {
                            before: papers
                                .iter()
                                .position(|e| e.pid == v2)
                                .and_then(|pos| if pos == 0 { None } else { papers.get(pos - 1) })
                                .map(|e| e.pid),
                            target: v2,
                            after: Some(v1),
                        });
                    }
                }
                KeyCode::Down | KeyCode::J => {
                    if let Some((v1, v2)) = self.selected_paper.zip(self.related_papers.1) {
                        let mut papers: Vec<&Paper> = self.papers.values().collect();
                        papers.sort_unstable_by_key(|paper| &paper.time);
                        papers.reverse();
                        return self.update(Msg::OpenPaper {
                            after: papers
                                .iter()
                                .position(|e| e.pid == v2)
                                .and_then(|pos| papers.get(pos + 1))
                                .map(|e| e.pid),
                            target: v2,
                            before: Some(v1),
                        });
                    }
                }
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    if let Some(value) = self.selected_paper {
                        return self.update(Msg::Accept(value));
                    }
                }
                _ => (),
            },
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
                        Text::new(match self.split_axis {
                            iced_aw::split::Axis::Vertical => "",
                            iced_aw::split::Axis::Horizontal => "",
                        })
                        .width(23.5)
                        .height(30)
                        .size(13.5)
                        .horizontal_alignment(iced::alignment::Horizontal::Center)
                        .style(Color::new(0.5, 0.5, 0.5, 1.0))
                        .font(self.nerd_font),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::SwitchSplitAxis),
                )
                .push(
                    button(
                        Text::new("")
                            .width(23.5)
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
                        Text::new("")
                            .width(23.5)
                            .height(30)
                            .size(13.5)
                            .horizontal_alignment(iced::alignment::Horizontal::Center)
                            .style(Color::new(0.5, 0.5, 0.5, 1.0))
                            .font(self.nerd_font),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::CleanAccepted),
                );

            if Arc::strong_count(&self.refresh_count) == 1 {
                bar = bar.push(
                    button(
                        Text::new("")
                            .width(23.5)
                            .height(30)
                            .size(13.5)
                            .horizontal_alignment(iced::alignment::Horizontal::Center)
                            .style(Color::new(0.5, 0.5, 0.5, 1.0))
                            .font(self.nerd_font),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::Refresh),
                );
            }

            left = left.push(bar);
        }

        {
            let mut down = Column::new().width(Length::Fill);

            let mut papers: Vec<&Paper> = self.papers.values().collect();
            papers.sort_unstable_by_key(|paper| &paper.time);
            papers.reverse();

            let mut before = None;
            let mut after;

            for paper in papers.iter().copied().enumerate() {
                after = papers.get(paper.0 + 1).copied().map(|e| e.pid);

                down = down.push(
                    button(
                        container({
                            let mut row = Row::new().height(18.5).push(
                                Text::new(format!(" {}: {}", paper.1.name, paper.1.info))
                                    .width(Length::Fill)
                                    .horizontal_alignment(iced::alignment::Horizontal::Left)
                                    .vertical_alignment(iced::alignment::Vertical::Center),
                            );

                            if let Some(p) = paper.1.processed {
                                row = row.push(
                                    Text::new("")
                                        .size(10)
                                        .width(18.5)
                                        .height(18.5)
                                        .horizontal_alignment(iced::alignment::Horizontal::Center)
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
                            if self.selected_paper.map_or(false, |e| paper.1.pid == e) {
                                theme::Container::Box
                            } else {
                                theme::Container::Transparent
                            },
                        ),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::OpenPaper {
                        before,
                        target: paper.1.pid,
                        after,
                    }),
                );

                before = Some(paper.1.pid);
            }

            left = left.push(Scrollable::new(down).height(Length::Fill));
        }

        let mut right = Column::new().height(Length::Fill).width(Length::Fill);
        if let Some(paper) = self
            .selected_paper
            .and_then(|value| self.papers.get(&value))
        {
            let hex_color = HexColor::parse_rgb(&paper.color).unwrap_or_default();

            right = right.push(
                Scrollable::new({
                    let mut col = Column::new()
                        .push(vertical_space(15))
                        .push(
                            Row::new().push(
                                container(Text::new(format!("  {}  ", paper.info)).size(18.5))
                                    .style(if self.display_bg {
                                        theme::Container::Custom(Box::new(move |_: &_| {
                                            iced::widget::container::Appearance {
                                                text_color: Some(color!(000000)),
                                                background: Some(iced::Background::Color(
                                                    Color::from_rgb8(
                                                        hex_color.r,
                                                        hex_color.g,
                                                        hex_color.b,
                                                    ),
                                                )),
                                                border_radius: Default::default(),
                                                border_width: 0.,
                                                border_color: Default::default(),
                                            }
                                        }))
                                    } else {
                                        theme::Container::Transparent
                                    })
                                    .width(Length::Fill),
                            ),
                        )
                        .push(vertical_space(15))
                        .push(
                            Row::new()
                                .push(Text::new("").font(self.nerd_font))
                                .push(horizontal_space(3.5))
                                .push(Text::new(&paper.name)),
                        );

                    if let Some(email) = paper.email.as_deref() {
                        col = col.push(
                            Row::new()
                                .push(Text::new("").font(self.nerd_font))
                                .push(horizontal_space(3.5))
                                .push(Text::new(email)),
                        );
                    }

                    col.push(
                        Text::new(paper.time.to_rfc2822()).style(Color::new(0.5, 0.5, 0.5, 1.)),
                    )
                })
                .height(Length::Fill),
            );

            if paper.processed.is_none() {
                let mut row = Row::new().height(35).push(
                    button(
                        Text::new("Accept")
                            .horizontal_alignment(iced::alignment::Horizontal::Center),
                    )
                    .width(Length::Fill)
                    .style(theme::Button::Positive)
                    .on_press(Msg::Accept(paper.pid)),
                );

                row = row.push(
                    button(
                        Text::new("")
                            .size(16.5)
                            .height(35)
                            .width(35)
                            .horizontal_alignment(iced::alignment::Horizontal::Center)
                            .vertical_alignment(iced::alignment::Vertical::Center)
                            .style(Color::new(0.5, 0.5, 0.5, 1.))
                            .font(self.nerd_font),
                    )
                    .style(theme::Button::Text)
                    .on_press(Msg::ToggleBg),
                );

                right = right.push(row).push(vertical_space(15));
            }
        }

        Split::new(
            left,
            Row::new()
                .push(horizontal_space(15))
                .push(right)
                .push(horizontal_space(15)),
            self.split_0_pos,
            self.split_axis,
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

    fn subscription(&self) -> iced_futures::Subscription<Self::Message> {
        iced::subscription::events().map(Msg::Event)
    }
}

#[derive(Debug, Clone)]
enum Msg {
    FontLoaded(Result<(), iced::font::Error>),
    Split0Resized(u16),
    RefreshLoop(Duration),
    Refresh,
    RefreshDone(Vec<Paper>),
    OpenPaper {
        before: Option<i32>,
        target: i32,
        after: Option<i32>,
    },
    Accept(i32),
    Accepted(i32, bool),
    ToggleDarkMode,
    SwitchSplitAxis,
    ToggleBg,
    CleanAccepted,
    Multi(Vec<Self>),
    Event(iced::Event),
}

#[derive(Debug, Deserialize, Clone)]
struct Paper {
    pid: i32,
    info: String,
    time: DateTime<chrono::Local>,
    name: String,
    email: Option<String>,
    color: String,

    #[serde(default)]
    processed: Option<bool>,
}
