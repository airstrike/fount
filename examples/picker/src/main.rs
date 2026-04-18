use std::time::Duration;

use iced::widget::{column, combo_box, container, row, text};
use iced::{Center, Element, Fill, Font, Subscription, Task, padding};

const PREVIEW: &str = "The quick brown fox jumps over the lazy dog.";

/// Braille-dot spinner frames. Each frame is default-rendered (not in the
/// selected font) so it remains visible even while the target family is
/// still being registered with cosmic-text.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPINNER_INTERVAL: Duration = Duration::from_millis(80);

/// Width of the left-column slot that holds the spinner (and the mirrored
/// right-column for symmetry). Every row reserves the same gutter so the
/// preview text, font-name label, and combo_box all start at the same x.
const GUTTER: u32 = 16;
/// Gap between gutter and main content in each row.
const ROW_GAP: u32 = 8;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("Font Picker")
        .run()
}

enum App {
    Loading(Loading),
    Loaded(Picker),
}

#[derive(Default)]
struct Loading {
    /// Outer Option = "result has arrived"; inner Option = Some(catalog)
    /// on success or None if the fetch failed.
    catalog: Option<Option<fount::Catalog>>,
    system_fonts: Option<Vec<fount::system::Font>>,
    error: Option<String>,
    spinner_frame: usize,
}

impl Loading {
    /// Still waiting on at least one source to finish fetching.
    fn is_fetching(&self) -> bool {
        self.catalog.is_none() || self.system_fonts.is_none()
    }
}

struct Picker {
    fount: fount::Fount,
    system_fonts: Vec<fount::system::Font>,
    font_list: combo_box::State<String>,
    /// The family the user asked to see next. Set synchronously on
    /// selection, *before* any load task completes.
    selected: Option<String>,
    /// The last family that finished loading (success or error). The
    /// picker is "loading" exactly when `selected != loaded` — once the
    /// first load task for the current selection reports back, cosmic-text
    /// can render the family, and we stop the spinner.
    loaded: Option<String>,
    spinner_frame: usize,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    CatalogLoaded(Result<fount::Catalog, fount::Error>),
    SystemDiscovered(Vec<fount::system::Font>),
    FontSelected(String),
    FontLoaded(String, Result<(), String>),
    Tick,
}

impl Picker {
    fn is_loading(&self) -> bool {
        self.selected.is_some() && self.selected != self.loaded
    }
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let catalog_task = Task::future(fount::google::catalog(
            fount::google::DEFAULT_CATALOG_MAX_AGE,
        ))
        .map(Message::CatalogLoaded);

        // Discover system fonts (and Office-bundled fonts via the `office`
        // feature) on a blocking thread so we don't stall the runtime.
        let system_task = Task::future(async {
            tokio::task::spawn_blocking(|| {
                fount::system::discover(&fount::system::Config::default())
            })
            .await
            .unwrap_or_default()
        })
        .map(Message::SystemDiscovered);

        (
            App::Loading(Loading::default()),
            Task::batch([catalog_task, system_task]),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match self {
            App::Loading(loading) => {
                match message {
                    Message::CatalogLoaded(Ok(catalog)) => {
                        loading.catalog = Some(Some(catalog));
                    }
                    Message::CatalogLoaded(Err(e)) => {
                        loading.error = Some(format!("Catalog: {e}"));
                        // Treat catalog failure as "no catalog" so we still
                        // make progress with whatever system fonts we found.
                        loading.catalog = Some(None);
                    }
                    Message::SystemDiscovered(fonts) => {
                        loading.system_fonts = Some(fonts);
                    }
                    Message::Tick => {
                        loading.spinner_frame = loading.spinner_frame.wrapping_add(1);
                    }
                    // Ignore late selection/load messages while still
                    // loading — the picker isn't on screen yet.
                    Message::FontSelected(_) | Message::FontLoaded(_, _) => {}
                }

                // Both data sources in? Build the picker and switch states.
                if loading.catalog.is_some() && loading.system_fonts.is_some() {
                    let catalog = loading.catalog.take().unwrap();
                    let system_fonts = loading.system_fonts.take().unwrap();
                    let error = loading.error.take();
                    *self = App::Loaded(Picker::new(catalog, system_fonts, error));
                }

                Task::none()
            }
            App::Loaded(picker) => picker.update(message),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let animating = match self {
            App::Loading(loading) => loading.is_fetching(),
            App::Loaded(picker) => picker.is_loading(),
        };
        if animating {
            iced::time::every(SPINNER_INTERVAL).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            App::Loading(loading) => {
                let title = text("Font Picker").size(28).font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::DEFAULT
                });

                let spinner_frame = SPINNER_FRAMES[loading.spinner_frame % SPINNER_FRAMES.len()];

                let step =
                    |done: bool, pending_label: &str, done_label: &str| -> Element<'_, Message> {
                        let marker = if done { "✓" } else { spinner_frame };
                        let label = if done { done_label } else { pending_label };
                        row![text(marker).size(13), text(label.to_string()).size(13)]
                            .spacing(8)
                            .into()
                    };

                let steps = column![
                    step(
                        loading.catalog.is_some(),
                        "Fetching Google Fonts catalog",
                        "Google Fonts catalog",
                    ),
                    step(
                        loading.system_fonts.is_some(),
                        "Discovering system fonts",
                        "System fonts",
                    ),
                ]
                .spacing(4);

                let mut col = column![title, steps].spacing(20);
                if let Some(ref e) = loading.error {
                    col = col.push(text(e).size(12).color(iced::color!(0xcc3333)));
                }

                container(col.max_width(600))
                    .width(Fill)
                    .align_x(Center)
                    .padding(padding::all(24).top(48))
                    .into()
            }
            App::Loaded(picker) => picker.view(),
        }
    }
}

impl Picker {
    fn new(
        catalog: Option<fount::Catalog>,
        system_fonts: Vec<fount::system::Font>,
        error: Option<String>,
    ) -> Self {
        let mut fount = fount::Fount::new();
        fount.set_system_families(fount::system::family_names(&system_fonts));
        if let Some(catalog) = catalog {
            fount.set_google_catalog(catalog);
        }

        // Build the merged family list once, up front.
        let mut names: Vec<String> = fount.system_families().to_vec();
        if let Some(catalog) = fount.google_catalog() {
            names.extend(catalog.top(200));
        }
        names.sort();
        names.dedup();

        Self {
            fount,
            system_fonts,
            font_list: combo_box::State::new(names),
            selected: None,
            loaded: None,
            spinner_frame: 0,
            error,
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FontSelected(name) => {
                // Re-selecting the currently-loaded font is a no-op; avoids
                // a spinner flicker if the combo_box re-emits.
                if self.loaded.as_deref() == Some(name.as_str())
                    && self.selected.as_deref() == Some(name.as_str())
                {
                    return Task::none();
                }

                self.selected = Some(name.clone());
                self.error = None;

                // Prefer system fonts (already on disk) over Google downloads.
                // Load *all* faces for the family, not just the first one —
                // otherwise the only registered weight may be (e.g.) Black,
                // and cosmic-text can't match the default weight=400 render.
                let faces: Vec<fount::system::Font> = self
                    .system_fonts
                    .iter()
                    .filter(|f| f.family.eq_ignore_ascii_case(&name))
                    .cloned()
                    .collect();

                if !faces.is_empty() {
                    let tasks = faces.into_iter().map(|font| {
                        let n = name.clone();
                        Task::future(async move { fount::system::load(&font).await }).then(
                            move |result| {
                                let n = n.clone();
                                match result {
                                    Ok(bytes) => iced::font::load(bytes).map(move |r| {
                                        Message::FontLoaded(
                                            n.clone(),
                                            r.map_err(|e| format!("{e:?}")),
                                        )
                                    }),
                                    Err(e) => {
                                        Task::done(Message::FontLoaded(n, Err(e.to_string())))
                                    }
                                }
                            },
                        )
                    });
                    return Task::batch(tasks);
                }

                let n = name.clone();
                let catalog = self.fount.google_catalog().cloned();
                Task::future(async move { fount::google::load(&name, catalog.as_ref()).await })
                    .then(move |result| {
                        let n = n.clone();
                        match result {
                            Ok(bytes_list) => Task::batch(bytes_list.into_iter().map({
                                let n = n.clone();
                                move |bytes| {
                                    let n = n.clone();
                                    iced::font::load(bytes).map(move |r| {
                                        Message::FontLoaded(
                                            n.clone(),
                                            r.map_err(|e| format!("{e:?}")),
                                        )
                                    })
                                }
                            })),
                            Err(e) => Task::done(Message::FontLoaded(n, Err(e.to_string()))),
                        }
                    })
            }
            Message::FontLoaded(name, result) => {
                // Only pay attention to loads for the *current* selection —
                // late messages from a previous selection must not flip our
                // loading state or overwrite an error for the current one.
                if self.selected.as_deref() == Some(name.as_str()) {
                    self.loaded = Some(name.clone());
                    if let Err(e) = result {
                        self.error = Some(format!("{name}: {e}"));
                    }
                }
                Task::none()
            }
            Message::Tick => {
                self.spinner_frame = self.spinner_frame.wrapping_add(1);
                Task::none()
            }
            // Loading-phase messages can't reach a Loaded picker.
            Message::CatalogLoaded(_) | Message::SystemDiscovered(_) => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // --- preview (24pt styled text, in a fixed-height slot) ---
        let preview_text: Element<'_, Message> = if let Some(ref name) = self.selected {
            text(PREVIEW)
                .font(Font::with_family(name.as_str()))
                .size(24)
                .into()
        } else {
            text("Select a font below to preview it.").size(16).into()
        };
        let preview_content = container(preview_text)
            .height(80)
            .align_y(Center)
            .width(Fill);

        // --- status (spinner in the left gutter; stable-position name) ---
        let spinner_content: Element<'_, Message> = if self.is_loading() {
            let frame = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
            text(frame).size(14).into()
        } else {
            text("").size(14).into()
        };
        let status_text: Element<'_, Message> = if let Some(ref e) = self.error {
            text(e).size(12).color(iced::color!(0xcc3333)).into()
        } else if let Some(ref name) = self.selected {
            let label = if self.is_loading() {
                format!("{name} loading…")
            } else {
                name.clone()
            };
            text(label).size(12).into()
        } else {
            text(format!(
                "{} fonts available",
                self.font_list.options().len()
            ))
            .size(12)
            .into()
        };
        let status_content = container(status_text)
            .height(28)
            .align_y(Center)
            .width(Fill);

        // --- picker ---
        let picker_content = combo_box(
            &self.font_list,
            "Search fonts...",
            self.selected.as_ref(),
            Message::FontSelected,
        )
        .width(Fill);

        // Each row: [left gutter] [gap] [main content, fills] [gap] [right gutter].
        // The left gutter only shows the spinner on the status row — for
        // preview and picker it's empty, but reserved so all three items
        // line up at the same x. The right gutter mirrors for symmetry.
        let empty_gutter = || container(text("")).width(GUTTER);

        let preview_row = row![empty_gutter(), preview_content, empty_gutter(),]
            .spacing(ROW_GAP)
            .align_y(Center);

        let status_row = row![
            container(spinner_content)
                .width(GUTTER)
                .align_x(Center)
                .align_y(Center),
            status_content,
            empty_gutter(),
        ]
        .spacing(ROW_GAP)
        .align_y(Center);

        let picker_row = row![empty_gutter(), picker_content, empty_gutter(),]
            .spacing(ROW_GAP)
            .align_y(Center);

        container(
            column![preview_row, status_row, picker_row]
                .spacing(12)
                .max_width(600),
        )
        .width(Fill)
        .align_x(Center)
        .padding(padding::all(24).top(48))
        .into()
    }
}
