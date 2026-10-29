//! The window: the pastel layout of the original tool.

use std::path::{Path, PathBuf};

use egui::{
    text::{CCursor, CCursorRange},
    Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, RichText, Stroke, Ui,
};

use crate::rename::{self, Done, Op, Plan, Row, RowKind};

pub const APP_TITLE: &str = "Renamer Portable - MASTER";

const BG_MAIN: Color32 = Color32::from_rgb(0xF0, 0xE6, 0xF6);
const BG_FRAME: Color32 = Color32::from_rgb(0xE8, 0xF4, 0xF8);
const ACCENT: Color32 = Color32::from_rgb(0xB4, 0xA7, 0xD6);
const ACCENT_HOVER: Color32 = Color32::from_rgb(0x9A, 0x8A, 0xAF);
const BUTTON: Color32 = Color32::from_rgb(0x9E, 0xC5, 0xAB);
const BUTTON_HOVER: Color32 = Color32::from_rgb(0x88, 0xB8, 0x9D);
const PINK: Color32 = Color32::from_rgb(0xFF, 0xB6, 0xC1);
const PINK_HOVER: Color32 = Color32::from_rgb(0xFF, 0x9E, 0xAA);
const PINK_BG: Color32 = Color32::from_rgb(0xFF, 0xE5, 0xE5);
const LIST_BG: Color32 = Color32::from_rgb(0xFF, 0xF8, 0xDC);
const LIST_FIELD: Color32 = Color32::from_rgb(0xFF, 0xFA, 0xCD);
const LIST_LINE: Color32 = Color32::from_rgb(0xF0, 0xE6, 0x8C);
const LIST_LABEL: Color32 = Color32::from_rgb(0x8B, 0x73, 0x55);
const SELECTED: Color32 = Color32::from_rgb(0xE6, 0xDC, 0xF5);
const FIND_BG: Color32 = Color32::from_rgb(0xFF, 0xF0, 0xE6);
const FIND_LINE: Color32 = Color32::from_rgb(0xFF, 0x8C, 0x42);
const FIND_LABEL: Color32 = Color32::from_rgb(0xCC, 0x55, 0x00);
const FIND_BUTTON: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x35);
const FIND_BUTTON_HOVER: Color32 = Color32::from_rgb(0xE5, 0x5A, 0x2B);
const REPL_BG: Color32 = Color32::from_rgb(0xE6, 0xF0, 0xFF);
const REPL_LINE: Color32 = Color32::from_rgb(0x4A, 0x90, 0xD9);
const REPL_LABEL: Color32 = Color32::from_rgb(0x2C, 0x6F, 0xBB);
const REPL_BUTTON_HOVER: Color32 = Color32::from_rgb(0x3A, 0x7B, 0xC8);
const TITLE: Color32 = Color32::from_rgb(0x7B, 0x5E, 0xA7);
const SUBTITLE: Color32 = ACCENT_HOVER;
const TEXT: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x33);
const CHANGED: Color32 = Color32::from_rgb(0x2E, 0x7D, 0x32);
const INVALID: Color32 = Color32::from_rgb(0xC6, 0x28, 0x28);
const INVALID_BG: Color32 = Color32::from_rgb(0xFF, 0xEB, 0xEE);

const ROW_H: f32 = 22.0;

enum Dialog {
    Info { title: String, text: String },
    Confirm { title: String, ops: Vec<Op> },
    /// Summary plus the details of what went wrong.
    Result { title: String, count: usize, errors: Vec<String> },
}

pub struct App {
    folder: String,
    find: String,
    replace: String,
    files: Vec<String>,
    rows: Vec<Row>,
    /// Inputs the rows were computed for.
    rows_key: (String, String, usize),
    selected: Option<String>,
    undo: Vec<Done>,
    dialog: Option<Dialog>,
    focus_find: bool,
    reload: bool,
}

fn valid_folder(folder: &str) -> Option<PathBuf> {
    let f = folder.trim();
    if f.is_empty() {
        return None;
    }
    let p = Path::new(f);
    p.is_dir().then(|| p.to_path_buf())
}

impl App {
    pub fn new(ctx: &egui::Context, folder: String) -> Self {
        apply_theme(ctx);
        let mut app = Self {
            folder,
            find: String::new(),
            replace: String::new(),
            files: Vec::new(),
            rows: Vec::new(),
            rows_key: (String::new(), String::new(), 0),
            selected: None,
            undo: Vec::new(),
            dialog: None,
            focus_find: false,
            reload: true,
        };
        app.load_files();
        app
    }

    fn info(&mut self, title: &str, text: impl Into<String>) {
        self.dialog = Some(Dialog::Info { title: title.to_string(), text: text.into() });
    }

    /// Reads the folder again and rebuilds the preview.
    pub fn load_files(&mut self) {
        self.reload = false;
        self.files = match valid_folder(&self.folder) {
            Some(p) => match rename::list_files(&p) {
                Ok(f) => f,
                Err(e) => {
                    self.info("Errore", format!("Impossibile leggere la cartella:\n{e}"));
                    Vec::new()
                }
            },
            None => Vec::new(),
        };
        self.rows_key.2 = usize::MAX; // force
        self.refresh_rows();
    }

    fn refresh_rows(&mut self) {
        let key = (self.find.clone(), self.replace.clone(), self.files.len());
        if key == self.rows_key {
            return;
        }
        self.rows = rename::preview(&self.files, &self.find, &self.replace);
        self.rows_key = key;
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn set_texts(&mut self, find: &str, replace: &str) {
        self.find = find.to_string();
        self.replace = replace.to_string();
        self.refresh_rows();
    }

    fn get_valid_folder(&mut self) -> Option<PathBuf> {
        if self.folder.trim().is_empty() {
            self.info("Attenzione", "Seleziona prima una cartella.");
            return None;
        }
        let p = valid_folder(&self.folder);
        if p.is_none() {
            self.info("Attenzione", "Cartella non valida.");
        }
        p
    }

    // ---- actions --------------------------------------------------------------------------

    /// Asks for confirmation before renaming (or says there is nothing to do).
    pub fn queue_ops(&mut self, title: &str, ops: Vec<Op>) {
        if ops.is_empty() {
            self.info(title, "Nessuna modifica — Nessun file soddisfa i criteri di rinomina.");
        } else {
            self.dialog = Some(Dialog::Confirm { title: title.to_string(), ops });
        }
    }

    pub fn apply_removal(&mut self) {
        if self.find.is_empty() {
            self.info("Attenzione", "Inserisci il testo da rimuovere.");
            return;
        }
        let Some(folder) = self.get_valid_folder() else { return };
        let files = rename::list_files(&folder).unwrap_or_default();
        let blockers = rename::removal_blockers(&files, &self.find);
        if !blockers.is_empty() {
            let shown = blockers.iter().take(10).cloned().collect::<Vec<_>>().join("\n");
            let extra = if blockers.len() > 10 {
                format!("\n... e altri {}", blockers.len() - 10)
            } else {
                String::new()
            };
            self.info(
                "Operazione bloccata",
                format!("La rimozione cancellerebbe COMPLETAMENTE il nome di:\n\n{shown}{extra}\n\nUsa un testo più specifico."),
            );
            return;
        }
        self.replace.clear();
        self.run_replace("Rimozione");
    }

    pub fn apply_replacement(&mut self) {
        if self.find.is_empty() {
            self.info("Attenzione", "Inserisci il testo da cercare.");
            return;
        }
        self.run_replace("Sostituzione");
    }

    fn run_replace(&mut self, title: &str) {
        let Some(folder) = self.get_valid_folder() else { return };
        let files = rename::list_files(&folder).unwrap_or_default();
        let plan = rename::plan_replace(&folder, &files, &self.find, &self.replace);
        self.finish_plan(title, plan, true);
    }

    pub fn remove_dots(&mut self) {
        let Some(folder) = self.get_valid_folder() else { return };
        let files = rename::list_files(&folder).unwrap_or_default();
        let plan = rename::plan_remove_dots(&folder, &files);
        self.finish_plan("Rimozione Punti", plan, false);
    }

    pub fn convert_sxxe(&mut self) {
        let Some(folder) = self.get_valid_folder() else { return };
        let files = rename::list_files(&folder).unwrap_or_default();
        let plan = rename::plan_sxxe(&folder, &files);
        self.finish_plan("Conversione S01E01", plan, false);
    }

    fn finish_plan(&mut self, title: &str, plan: Plan, show_errors: bool) {
        if show_errors && !plan.errors.is_empty() && plan.ops.is_empty() {
            self.dialog =
                Some(Dialog::Result { title: title.to_string(), count: 0, errors: plan.errors });
            return;
        }
        self.queue_ops(&format!("Completato - {title}"), plan.ops);
    }

    fn execute(&mut self, title: &str, ops: &[Op]) {
        let (done, errors) = rename::execute(ops);
        let count = done.len();
        if !done.is_empty() {
            self.undo.push(done);
        }
        self.load_files();
        if errors.is_empty() {
            self.info(title, format!("✅ Modificati {count} file con successo."));
        } else {
            self.dialog = Some(Dialog::Result { title: title.to_string(), count, errors });
        }
    }

    pub fn undo_last(&mut self) {
        let Some(last) = self.undo.pop() else {
            self.info("Undo", "Nulla da annullare.");
            return;
        };
        let (count, errors) = rename::undo(&last);
        self.load_files();
        let mut msg = format!("Ripristinati {count} file.");
        if !errors.is_empty() {
            msg.push_str(&format!(
                "\nErrore nel ripristino di {} file:\n{}",
                errors.len(),
                errors.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
            ));
        }
        self.info("Undo", msg);
    }

    fn browse(&mut self) {
        let mut dlg = rfd::FileDialog::new();
        if let Some(p) = valid_folder(&self.folder) {
            dlg = dlg.set_directory(p);
        }
        if let Some(p) = dlg.pick_folder() {
            self.folder = p.to_string_lossy().into_owned();
            self.load_files();
        }
    }

    fn on_select(&mut self, original: &str) {
        self.selected = Some(original.to_string());
        if self.find.trim().is_empty() {
            self.find = rename::split_ext(original).0.to_string();
            self.focus_find = true;
            self.refresh_rows();
        }
    }
}

// ---- drawing ------------------------------------------------------------------------------

fn apply_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Light);
    let mut v = egui::Visuals::light();
    v.panel_fill = BG_MAIN;
    v.window_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::WHITE;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke.color = Color32::WHITE;
    v.window_corner_radius = CornerRadius::same(14);
    v.window_stroke = Stroke::new(2.0, ACCENT);
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.set_visuals_of(theme, v.clone());
    }
}

fn panel(fill: Color32, line: Color32, width: f32) -> Frame {
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(width, line))
        .corner_radius(CornerRadius::same(15))
        .inner_margin(Margin::same(15))
}

fn button(ui: &mut Ui, text: &str, size: f32, fill: Color32, hover: Color32, dims: [f32; 2]) -> bool {
    let resp = ui.add_sized(
        dims,
        egui::Button::new(RichText::new(text).size(size + 1.5).strong().color(Color32::WHITE))
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(8)),
    );
    if resp.hovered() {
        ui.painter().rect_filled(resp.rect, CornerRadius::same(8), hover);
        ui.painter().text(
            resp.rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(size + 1.5),
            Color32::WHITE,
        );
    }
    resp.clicked()
}

fn entry(ui: &mut Ui, text: &mut String, id: &str, hint: &str, line: Color32, h: f32, size: f32) -> egui::Response {
    let width = ui.available_width();
    Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(2.0, line))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 0))
        .show(ui, |ui| {
            ui.add_sized(
                [width - 20.0, h - 4.0],
                egui::TextEdit::singleline(text)
                    .id(egui::Id::new(id))
                    .hint_text(hint)
                    .font(FontId::proportional(size))
                    .vertical_align(Align::Center)
                    .frame(egui::Frame::NONE),
            )
        })
        .inner
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.reload {
            self.load_files();
        }
        let undo_key = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z));
        if undo_key && self.dialog.is_none() {
            self.undo_last();
        }
        let modal = self.dialog.is_some();
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG_MAIN).inner_margin(Margin::same(20)))
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| self.main_ui(ui));
            });
        self.dialog_ui(&ctx);
    }
}

impl App {
    fn main_ui(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Renamer Portable  MASTER").size(22.0).strong().color(TITLE));
            ui.label(
                RichText::new(
                    "by Lorenzo D'Apolito - Master Release (1 validazione, 1 esecuzione, UX smart)",
                )
                .size(12.0)
                .color(SUBTITLE),
            );
        });
        ui.add_space(10.0);

        // folder + quick actions
        ui.horizontal_top(|ui| {
            let quick_w = 130.0 + 130.0 + 80.0 + 3.0 * 10.0 + 2.0 * 10.0 + 8.0;
            let folder_w = (ui.available_width() - quick_w - 10.0).max(200.0);
            ui.allocate_ui_with_layout(
                egui::vec2(folder_w, 70.0),
                Layout::top_down(Align::Min),
                |ui| {
                    panel(PINK_BG, PINK, 2.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Cartella:").size(12.0).strong());
                            let w = ui.available_width() - 100.0;
                            let r = ui
                                .allocate_ui(egui::vec2(w, 35.0), |ui| {
                                    entry(ui, &mut self.folder, "folder", "", PINK, 35.0, 13.0)
                                })
                                .inner;
                            if r.lost_focus() && r.changed() || r.lost_focus() {
                                self.reload = true;
                            }
                            if button(ui, "Sfoglia...", 13.0, PINK, PINK_HOVER, [90.0, 35.0]) {
                                self.browse();
                            }
                        });
                    });
                },
            );
            panel(BG_FRAME, ACCENT, 2.0).inner_margin(Margin::same(10)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if button(ui, "Rimuovi Punti", 11.0, BUTTON, BUTTON_HOVER, [130.0, 35.0]) {
                        self.remove_dots();
                    }
                    if button(ui, "Converti S01E01", 11.0, BUTTON, BUTTON_HOVER, [130.0, 35.0]) {
                        self.convert_sxxe();
                    }
                    if button(ui, "↩ Undo", 11.0, ACCENT, ACCENT_HOVER, [80.0, 35.0]) {
                        self.undo_last();
                    }
                });
            });
        });
        ui.add_space(20.0);

        // bottom panels first, so the list takes what is left
        egui::Panel::bottom("find_replace")
            .frame(Frame::new().fill(BG_MAIN).inner_margin(Margin { top: 5, ..Default::default() }))
            .show_separator_line(false)
            .show(ui, |ui| self.find_replace_ui(ui));

        self.list_ui(ui);
    }

    fn find_replace_ui(&mut self, ui: &mut Ui) {
        ui.columns(2, |cols| {
            if let [left, right] = cols {
                left.label(RichText::new("TROVA").size(16.0).strong().color(FIND_LABEL));
                panel(FIND_BG, FIND_LINE, 3.0).show(left, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let w = ui.available_width() - 140.0;
                        let r = ui
                            .allocate_ui(egui::vec2(w, 45.0), |ui| {
                                entry(ui, &mut self.find, "find", "Testo da cercare...", FIND_LINE, 45.0, 14.0)
                            })
                            .inner;
                        if self.focus_find {
                            self.focus_find = false;
                            r.request_focus();
                            let n = self.find.chars().count();
                            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                                st.cursor.set_char_range(Some(CCursorRange::two(
                                    CCursor::new(0),
                                    CCursor::new(n),
                                )));
                                st.store(ui.ctx(), r.id);
                            }
                        }
                        if button(ui, "RIMUOVI", 13.0, FIND_BUTTON, FIND_BUTTON_HOVER, [130.0, 45.0]) {
                            self.apply_removal();
                        }
                    });
                });
                right.label(RichText::new("SOSTITUISCI CON").size(16.0).strong().color(REPL_LABEL));
                panel(REPL_BG, REPL_LINE, 3.0).show(right, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let w = ui.available_width() - 150.0;
                        ui.allocate_ui(egui::vec2(w, 45.0), |ui| {
                            entry(ui, &mut self.replace, "replace", "Nuovo testo...", REPL_LINE, 45.0, 14.0);
                        });
                        if button(ui, "SOSTITUISCI", 13.0, REPL_LINE, REPL_BUTTON_HOVER, [140.0, 45.0]) {
                            self.apply_replacement();
                        }
                    });
                });
            }
        });
        self.refresh_rows();
    }

    fn list_ui(&mut self, ui: &mut Ui) {
        let mut clicked: Option<String> = None;
        panel(LIST_BG, LIST_LINE, 2.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(
                    "LISTA FILE - Clicca per caricare nome nel campo TROVA (solo se vuoto) - Ctrl+Z undo",
                )
                .size(12.0)
                .strong()
                .color(LIST_LABEL),
            );
            ui.add_space(5.0);
            Frame::new().fill(LIST_FIELD).show(ui, |ui| {
                let half = (ui.available_width() - 20.0) / 2.0;
                let (hr, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::Sense::hover(),
                );
                let hf = FontId::proportional(13.0);
                ui.painter().text(
                    hr.left_center() + egui::vec2(6.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "Nome Originale",
                    hf.clone(),
                    TEXT,
                );
                ui.painter().text(
                    egui::pos2(hr.left() + half + 16.0, hr.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Nuovo Nome (Preview)",
                    hf,
                    TEXT,
                );
                ui.separator();
                let height = (ui.available_height() - 4.0).max(60.0);
                egui::ScrollArea::both().max_height(height).min_scrolled_height(height).auto_shrink(false).show_rows(
                    ui,
                    ROW_H,
                    self.rows.len(),
                    |ui, range| {
                        for row in self.rows.iter().skip(range.start).take(range.len()) {
                            let selected = self.selected.as_deref() == Some(row.original.as_str());
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width().max(half * 2.0), ROW_H),
                                egui::Sense::click(),
                            );
                            let bg = if selected {
                                Some(SELECTED)
                            } else if row.kind == RowKind::Invalid {
                                Some(INVALID_BG)
                            } else if resp.hovered() {
                                Some(Color32::from_rgb(0xFF, 0xF3, 0xB0))
                            } else {
                                None
                            };
                            if let Some(bg) = bg {
                                ui.painter().rect_filled(rect, 0.0, bg);
                            }
                            let colour = match row.kind {
                                RowKind::Same => TEXT,
                                RowKind::Changed => CHANGED,
                                RowKind::Invalid => INVALID,
                            };
                            let font = FontId::proportional(13.0);
                            let p = ui.painter().with_clip_rect(rect);
                            p.text(
                                rect.left_center() + egui::vec2(6.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                &row.original,
                                font.clone(),
                                TEXT,
                            );
                            p.text(
                                egui::pos2(rect.left() + half + 16.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &row.shown,
                                font,
                                colour,
                            );
                            if resp.clicked() {
                                clicked = Some(row.original.clone());
                            }
                        }
                    },
                );
            });
        });
        if let Some(c) = clicked {
            self.on_select(&c);
        }
    }

    fn dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else { return };
        // dim the window behind
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::new("dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(80));
        });
        let mut keep = true;
        let mut run: Option<(String, Vec<Op>)> = None;
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        match &dialog {
            Dialog::Info { title, text } => {
                modal(ctx, title, 420.0, |ui| {
                    ui.label(RichText::new(text).size(14.0));
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        if button(ui, "OK", 13.0, BUTTON, BUTTON_HOVER, [120.0, 35.0]) {
                            keep = false;
                        }
                    });
                });
                if esc || ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    keep = false;
                }
            }
            Dialog::Confirm { title, ops } => {
                modal(ctx, "Conferma", 420.0, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{title}:\n\nStai per rinominare {} file.\nProcedere?",
                            ops.len()
                        ))
                        .size(14.0),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if button(ui, "Sì", 13.0, BUTTON, BUTTON_HOVER, [120.0, 35.0]) {
                            run = Some((title.clone(), ops.clone()));
                            keep = false;
                        }
                        if button(ui, "No", 13.0, ACCENT, ACCENT_HOVER, [120.0, 35.0]) {
                            keep = false;
                        }
                    });
                });
                if esc {
                    keep = false;
                }
            }
            Dialog::Result { title, count, errors } => {
                modal(ctx, title, 640.0, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "✅ Modificati {count} file.\n❌ {} errori.",
                            errors.len()
                        ))
                        .size(14.0)
                        .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("Dettaglio:").size(12.0).strong());
                    egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                        for e in errors {
                            ui.label(e);
                        }
                    });
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        if button(ui, "Chiudi", 13.0, BUTTON, BUTTON_HOVER, [120.0, 35.0]) {
                            keep = false;
                        }
                    });
                });
                if esc {
                    keep = false;
                }
            }
        }
        if keep {
            self.dialog = Some(dialog);
        }
        if let Some((title, ops)) = run {
            self.execute(&title, &ops);
        }
    }
}

fn modal(ctx: &egui::Context, title: &str, width: f32, add: impl FnOnce(&mut Ui)) {
    egui::Window::new(title)
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .fixed_size([width, 0.0])
        .show(ctx, |ui| {
            ui.set_width(width - 16.0);
            add(ui);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(dir: &Path) -> App {
        App::new(&egui::Context::default(), dir.to_string_lossy().into_owned())
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("renamer-ui-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_preview_follows_the_fields_and_the_dialogs_gate_the_rename() {
        let d = temp("flow");
        for n in ["Serie S01E02 HD.mkv", "Altro.mkv"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        let mut a = app(&d);
        assert_eq!(a.rows().len(), 2);
        a.set_texts(" HD", "");
        assert_eq!(a.rows().iter().filter(|r| r.kind == RowKind::Changed).count(), 1);
        a.apply_removal();
        assert!(matches!(a.dialog, Some(Dialog::Confirm { .. })));
        // nothing happened yet
        assert!(d.join("Serie S01E02 HD.mkv").exists());
        let Some(Dialog::Confirm { title, ops }) = a.dialog.take() else { return };
        a.execute(&title, &ops);
        assert!(d.join("Serie S01E02.mkv").exists());
        a.convert_sxxe();
        let Some(Dialog::Confirm { title, ops }) = a.dialog.take() else { return };
        a.execute(&title, &ops);
        assert!(d.join("Serie 1x02.mkv").exists());
        a.undo_last();
        a.undo_last();
        assert!(d.join("Serie S01E02 HD.mkv").exists());
        a.undo_last();
        assert!(matches!(a.dialog, Some(Dialog::Info { .. })));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_input_is_reported_not_crashed() {
        let mut a = app(Path::new("/definitely/not/here"));
        assert!(a.rows().is_empty());
        a.remove_dots();
        assert!(matches!(a.dialog, Some(Dialog::Info { .. })));
        a.dialog = None;
        a.apply_replacement();
        assert!(matches!(a.dialog, Some(Dialog::Info { .. })));
    }

    #[test]
    fn clicking_a_row_fills_an_empty_find_field_only() {
        let d = temp("click");
        std::fs::write(d.join("Film.2020.mkv"), b"x").unwrap();
        let mut a = app(&d);
        a.on_select("Film.2020.mkv");
        assert_eq!(a.find, "Film.2020");
        a.find = "x".into();
        a.on_select("Film.2020.mkv");
        assert_eq!(a.find, "x");
        let _ = std::fs::remove_dir_all(&d);
    }
}
