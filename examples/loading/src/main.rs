//! `fount` loading example.
//!
//! Demonstrates the typical "real app" pattern: one bundled local font set as
//! the iced default, plus one Google font downloaded at runtime. A small
//! progress bar runs during the load, and the loading screen renders only in
//! the local font so there's no swap mid-load.

use std::time::Duration;

use iced::time::Instant;
use iced::widget::{Widget, center, column, progress_bar, text};
use iced::{Background, Center, Element, Font, Subscription, Task, Theme, border, window};

const INTER_REGULAR: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

const LOCAL_FAMILY: &str = "Inter";
const REMOTE_FAMILY: &str = "Playfair Display";
const PREVIEW: &str = "The quick brown fox jumps over the lazy dog.";

/// The displayed progress value closes `EASE` of its remaining distance to the
/// target every `EASE_STEP` of elapsed time, independent of the frame rate.
/// That reaches within 1% of the target in ~0.4s, which feels lively without
/// looking instantaneous.
const EASE: f32 = 0.15;
const EASE_STEP: Duration = Duration::from_millis(16);

/// Progress targets per stage. The bar caps below 100% until *all* downloaded
/// variants have been registered with iced — only then does it finish.
const STAGE_CATALOG: f32 = 0.33;
const STAGE_DOWNLOADED: f32 = 0.80;
const STAGE_REGISTERED: f32 = 1.0;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("fount – loading example")
        .font(Font::new(LOCAL_FAMILY))
        .fonts([INTER_REGULAR])
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
    /// stage target on each frame.
    displayed: f32,
    /// When the previous frame was drawn, to measure elapsed time.
    last_frame: Option<Instant>,
}

impl Loading {
    fn is_done(&self) -> bool {
        self.error.is_none()
            && matches!(self.pending_registers, Some(0))
            && self.displayed >= STAGE_REGISTERED - 0.005
    }

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
    Frame(Instant),
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
            App::Loading(loading) => {
                let task = loading.update(message);
                if loading.is_done() {
                    *self = App::Loaded(Loaded);
                }
                task
            }
            App::Loaded(_) => Task::none(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self {
            App::Loading(_) => window::frames().map(Message::Frame),
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
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CatalogLoaded(Ok(catalog)) => {
                let owned = catalog.clone();
                let task =
                    Task::future(
                        async move { fount::google::load(REMOTE_FAMILY, Some(&owned)).await },
                    )
                    .map(|r| Message::FontDownloaded(r.map_err(|e| e.to_string())));
                self.catalog = Some(catalog);
                task
            }
            Message::CatalogLoaded(Err(e)) => {
                self.error = Some(format!("Catalog: {e}"));
                Task::none()
            }
            Message::FontDownloaded(Ok(bytes)) => {
                self.pending_registers = Some(bytes.len());
                let tasks = bytes.into_iter().map(|b| {
                    iced::font::load(b)
                        .map(|r| Message::FontRegistered(r.map_err(|e| format!("{e:?}"))))
                });
                Task::batch(tasks)
            }
            Message::FontDownloaded(Err(e)) => {
                self.error = Some(format!("Download: {e}"));
                Task::none()
            }
            Message::FontRegistered(result) => {
                if let Some(remaining) = self.pending_registers.as_mut() {
                    *remaining = remaining.saturating_sub(1);
                }
                if let Err(e) = result {
                    self.error = Some(format!("Register: {e}"));
                }
                Task::none()
            }
            Message::Frame(now) => {
                let elapsed = self
                    .last_frame
                    .map_or(Duration::ZERO, |last| now.saturating_duration_since(last));
                self.last_frame = Some(now);
                let steps = elapsed.as_secs_f32() / EASE_STEP.as_secs_f32();
                let ease = 1.0 - (1.0 - EASE).powf(steps);
                let target = self.stage_target();
                self.displayed += (target - self.displayed) * ease;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let title = text("Loading").size(32);

        let status: Element<'_, Message> = if let Some(ref e) = self.error {
            text(e.clone()).size(13).style(text::danger).boxed()
        } else {
            text(self.stage_label())
                .size(13)
                .style(text::secondary)
                .boxed()
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

        center(body).boxed()
    }
}

impl Loaded {
    fn view(&self) -> Element<'_, Message> {
        let block = |label: &'static str, sample: Element<'static, Message>| {
            column![text(label).size(12).style(text::secondary), sample]
                .spacing(8)
                .align_x(Center)
        };

        let inter = block("Inter  ·  bundled locally", text(PREVIEW).size(30).boxed());

        let playfair = block(
            "Playfair Display  ·  loaded from Google Fonts",
            text(PREVIEW).font(REMOTE_FAMILY).size(30).boxed(),
        );

        center(column![inter, playfair].spacing(40).align_x(Center)).boxed()
    }
}

fn bar_style(theme: &Theme) -> progress_bar::Style {
    let palette = theme.palette();
    progress_bar::Style {
        background: Background::Color(palette.background.weak.color),
        bar: Background::Color(palette.primary.base.color),
        border: border::rounded(3.0),
    }
}
