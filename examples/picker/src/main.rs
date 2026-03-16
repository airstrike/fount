use iced::widget::{column, combo_box, container, text};
use iced::{Element, Fill, Font, Task};

const PREVIEW: &str = "The quick brown fox jumps over the lazy dog.";

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Font Picker")
        .run()
}

struct App {
    fount: fount::Fount,
    font_list: combo_box::State<String>,
    selected: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    CatalogLoaded(Result<fount::Catalog, fount::Error>),
    FontSelected(String),
    FontLoaded(String, Result<(), String>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let app = Self {
            fount: fount::Fount::new(),
            font_list: combo_box::State::new(vec![]),
            selected: None,
            error: None,
        };

        let task = Task::future(fount::google::catalog(
            fount::google::DEFAULT_CATALOG_MAX_AGE,
        ))
        .map(Message::CatalogLoaded);

        (app, task)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CatalogLoaded(Ok(catalog)) => {
                self.fount.set_google_catalog(catalog);
                self.font_list =
                    combo_box::State::new(self.fount.google_catalog().unwrap().top(200));
                Task::none()
            }
            Message::CatalogLoaded(Err(e)) => {
                self.error = Some(format!("Catalog: {e}"));
                Task::none()
            }
            Message::FontSelected(name) => {
                self.selected = Some(name.clone());
                self.error = None;
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
            Message::FontLoaded(_name, Ok(())) => Task::none(),
            Message::FontLoaded(name, Err(e)) => {
                self.error = Some(format!("{name}: {e}"));
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let picker = combo_box(
            &self.font_list,
            "Search fonts...",
            None,
            Message::FontSelected,
        );

        let preview: Element<'_, Message> = if let Some(ref name) = self.selected {
            text(PREVIEW)
                .font(Font::with_family(name.as_str()))
                .size(24)
                .into()
        } else {
            text("Select a font above to preview it.").size(14).into()
        };

        let status: Element<'_, Message> = if let Some(ref e) = self.error {
            text(e).size(12).color(iced::color!(0xcc3333)).into()
        } else if let Some(ref name) = self.selected {
            text(name).size(12).into()
        } else {
            text("Loading catalog...").size(12).into()
        };

        container(column![picker, preview, status].spacing(16).max_width(600))
            .center(Fill)
            .padding(40)
            .into()
    }
}
