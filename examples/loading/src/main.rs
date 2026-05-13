//! `fount` loading example.
//!
//! Demonstrates the typical "real app" pattern: one bundled local font set as
//! the iced default, plus one Google font downloaded at runtime. A small
//! progress bar runs during the load, and the loading screen renders only in
//! the local font so there's no swap mid-load.

use std::time::Duration;

use iced::widget::{center, column, progress_bar, text};
use iced::{Background, Center, Color, Element, Font, Subscription, Task, border, color};

const INTER_REGULAR: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

const LOCAL_FAMILY: &str = "Inter";
const REMOTE_FAMILY: &str = "Playfair Display";
const PREVIEW: &str = "The quick brown fox jumps over the lazy dog.";

const TICK_INTERVAL: Duration = Duration::from_millis(16);
/// Per-tick easing factor for the displayed progress value. ~0.15 lerps to
/// within 1% of the target in ~25 frames (~0.4s), which feels lively without
/// looking instantaneous.
const EASE: f32 = 0.15;

/// Progress targets per stage. The bar caps below 100% until *all* downloaded
/// variants have been registered with iced — only then does it finish.
const STAGE_CATALOG: f32 = 0.33;
const STAGE_DOWNLOADED: f32 = 0.80;
const STAGE_REGISTERED: f32 = 1.0;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("fount – loading example")
        .default_font(Font::new(LOCAL_FAMILY))
        .font(INTER_REGULAR)
        .run()
}

enum App {
    Loading(Loading),
    Loaded(Loaded),
}

#[derive(Default)]
struct Loading {
    catalog: Option<fount::Catalog>,
    /// Number of `iced::font::load` tasks still in flight.
    pending_registers: Option<usize>,
    error: Option<String>,
    /// Smoothly-animated progress in `0.0..=1.0`. Eases toward the current
    /// stage target each tick.
    displayed: f32,
}

impl Loading {
    fn stage_target(&self) -> f32 {
        if self.error.is_some() {
            return self.displayed;
        }
        match (self.catalog.is_some(), self.pending_registers) {
            (false, _) => STAGE_CATALOG,
            (true, None) => STAGE_DOWNLOADED,
            (true, Some(n)) if n > 0 => STAGE_DOWNLOADED,
            (true, Some(_)) => STAGE_REGISTERED,
        }
    }

    fn stage_label(&self) -> &'static str {
        match (self.catalog.is_some(), self.pending_registers) {
            (false, _) => "Fetching Google Fonts catalog…",
            (true, None) => "Downloading Playfair Display…",
            (true, Some(n)) if n > 0 => "Registering font…",
            (true, Some(_)) => "Ready",
        }
    }
}

struct Loaded;

#[derive(Debug, Clone)]
enum Message {
    CatalogLoaded(Result<fount::Catalog, String>),
    FontDownloaded(Result<Vec<Vec<u8>>, String>),
    FontRegistered(Result<(), String>),
    Tick,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let task = Task::future(fount::google::catalog(
            fount::google::DEFAULT_CATALOG_MAX_AGE,
        ))
        .map(|r| Message::CatalogLoaded(r.map_err(|e| e.to_string())));
        (App::Loading(Loading::default()), task)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match self {
            App::Loading(loading) => loading.update(message).unwrap_or_else(|| {
                *self = App::Loaded(Loaded);
                Task::none()
            }),
            App::Loaded(_) => Task::none(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self {
            App::Loading(_) => iced::time::every(TICK_INTERVAL).map(|_| Message::Tick),
            App::Loaded(_) => Subscription::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            App::Loading(loading) => loading.view(),
            App::Loaded(loaded) => loaded.view(),
        }
    }
}

impl Loading {
    /// Process a message. Returns `Some(task)` while still loading, or `None`
    /// to signal the caller to transition to [`Loaded`].
    fn update(&mut self, message: Message) -> Option<Task<Message>> {
        match message {
            Message::CatalogLoaded(Ok(catalog)) => {
                let owned = catalog.clone();
                let task =
                    Task::future(
                        async move { fount::google::load(REMOTE_FAMILY, Some(&owned)).await },
                    )
                    .map(|r| Message::FontDownloaded(r.map_err(|e| e.to_string())));
                self.catalog = Some(catalog);
                Some(task)
            }
            Message::CatalogLoaded(Err(e)) => {
                self.error = Some(format!("Catalog: {e}"));
                Some(Task::none())
            }
            Message::FontDownloaded(Ok(bytes)) => {
                self.pending_registers = Some(bytes.len());
                let tasks = bytes.into_iter().map(|b| {
                    iced::font::load(b)
                        .map(|r| Message::FontRegistered(r.map_err(|e| format!("{e:?}"))))
                });
                Some(Task::batch(tasks))
            }
            Message::FontDownloaded(Err(e)) => {
                self.error = Some(format!("Download: {e}"));
                Some(Task::none())
            }
            Message::FontRegistered(result) => {
                if let Some(remaining) = self.pending_registers.as_mut() {
                    *remaining = remaining.saturating_sub(1);
                }
                if let Err(e) = result {
                    self.error = Some(format!("Register: {e}"));
                }
                Some(Task::none())
            }
            Message::Tick => {
                let target = self.stage_target();
                self.displayed += (target - self.displayed) * EASE;
                if self.error.is_none()
                    && matches!(self.pending_registers, Some(0))
                    && self.displayed >= STAGE_REGISTERED - 0.005
                {
                    return None;
                }
                Some(Task::none())
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let title = text("Loading").size(32);

        let status: Element<'_, Message> = if let Some(ref e) = self.error {
            text(e.clone()).size(13).color(color!(0xcc3333)).into()
        } else {
            text(self.stage_label()).size(13).color(MUTED).into()
        };

        let bar = progress_bar(0.0..=1.0, self.displayed)
            .length(280)
            .girth(6)
            .style(bar_style);

        let body = if self.error.is_some() {
            column![title, status]
        } else {
            column![title, bar, status]
        }
        .spacing(16)
        .align_x(Center);

        center(body).into()
    }
}

impl Loaded {
    fn view(&self) -> Element<'_, Message> {
        let block = |label: &'static str, sample: Element<'static, Message>| {
            column![text(label).size(12).color(MUTED), sample]
                .spacing(8)
                .align_x(Center)
        };

        let inter = block("Inter  ·  bundled locally", text(PREVIEW).size(30).into());

        let playfair = block(
            "Playfair Display  ·  loaded from Google Fonts",
            text(PREVIEW).font(REMOTE_FAMILY).size(30).into(),
        );

        center(column![inter, playfair].spacing(40).align_x(Center)).into()
    }
}

const MUTED: Color = Color::from_rgb(0.55, 0.55, 0.58);

fn bar_style(_theme: &iced::Theme) -> progress_bar::Style {
    progress_bar::Style {
        background: Background::Color(Color::from_rgb(
            0xe5 as f32 / 255.0,
            0xe5 as f32 / 255.0,
            0xea as f32 / 255.0,
        )),
        bar: Background::Color(Color::from_rgb(
            0x5e as f32 / 255.0,
            0x8a as f32 / 255.0,
            0xff as f32 / 255.0,
        )),
        border: border::rounded(3.0),
    }
}
