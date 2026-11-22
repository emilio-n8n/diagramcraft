//! DiagramCraft UI, interactions and project lifecycle.

use std::path::PathBuf;

use craftkit_io::DocumentHost;
use craftkit_jobs::JobRunner;
use craftkit_ui::cli::CliOptions;
use craftkit_ui::eframe::{self, egui};
use craftkit_ui::egui::{Align, Align2, Color32, FontId, Layout, Pos2, RichText, Stroke, Vec2};
use craftkit_ui::{AppIdentity, AppShell, Tone, dialogs, theme, widgets};

use crate::document::{self, Clipboard, Connector, Document, Point, Routing, Shape, ShapeKind};
use crate::geometry::{self, AlignMode, Axis, Handle};
use crate::{pdf, svg};

const FORMAT: &str = "diagramcraft";
const SCHEMA_VERSION: u32 = 2;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;
const GRID_STEP: f64 = 50.0;
const HANDLE_TOLERANCE: f64 = 8.0;
const CONNECTOR_TOLERANCE: f64 = 7.0;
const PASTE_OFFSET: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    New,
    Open,
    Save,
    SaveAs,
    ExportSvg,
    ExportPdf,
    Undo,
    Redo,
    Delete,
    EditLabel,
    Copy,
    Paste,
    Duplicate,
    SelectAll,
    Fit,
    ToggleGrid,
    ToggleSnap,
    Align(AlignMode),
    Distribute(Axis),
    About,
    Settings,
}

#[derive(Debug, Clone)]
enum PendingAction {
    New,
    Open(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingChoice {
    Save,
    Discard,
    Cancel,
}

/// Interaction mode selected in the left palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Connect,
    Rectangle,
    Ellipse,
    Note,
    Diamond,
}

impl Tool {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Connect => "Connect",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Note => "Note",
            Self::Diamond => "Diamond",
        }
    }

    fn shape_kind(self) -> Option<ShapeKind> {
        match self {
            Self::Rectangle => Some(ShapeKind::Rectangle),
            Self::Ellipse => Some(ShapeKind::Ellipse),
            Self::Note => Some(ShapeKind::Note),
            Self::Diamond => Some(ShapeKind::Diamond),
            Self::Select | Self::Connect => None,
        }
    }

    fn from_kind(kind: ShapeKind) -> Self {
        match kind {
            ShapeKind::Rectangle => Self::Rectangle,
            ShapeKind::Ellipse => Self::Ellipse,
            ShapeKind::Note => Self::Note,
            ShapeKind::Diamond => Self::Diamond,
        }
    }
}

/// State of an in-progress pointer drag.
#[derive(Debug, Clone)]
enum Drag {
    /// Move several shapes from their captured positions.
    Move {
        start: (f64, f64),
        origins: Vec<(u64, f64, f64)>,
    },
    /// Resize one shape from its captured rectangle.
    Resize {
        id: u64,
        handle: Handle,
        origin: (f64, f64, f64, f64),
        start: (f64, f64),
    },
    /// Move a connector waypoint.
    Waypoint { id: u64, index: usize },
}

/// World-to-screen transform for the canvas.
#[derive(Debug, Clone, Copy)]
struct View {
    origin: Pos2,
    zoom: f32,
}

impl View {
    fn to_screen(self, x: f64, y: f64) -> Pos2 {
        self.origin + Vec2::new(x as f32, y as f32) * self.zoom
    }

    fn to_world(self, pos: Pos2) -> (f64, f64) {
        let point = (pos - self.origin) / self.zoom;
        (point.x as f64, point.y as f64)
    }
}

/// An open in-place label editor.
#[derive(Debug, Clone)]
struct LabelEdit {
    id: u64,
    text: String,
    focus: bool,
}

/// The DiagramCraft application state.
pub struct DiagramApp {
    shell: AppShell,
    host: DocumentHost<Document>,
    selected: Vec<u64>,
    selected_connector: Option<u64>,
    tool: Tool,
    routing: Routing,
    connect_from: Option<u64>,
    drag: Option<Drag>,
    panning: bool,
    pan: Vec2,
    zoom: f32,
    show_grid: bool,
    snap_enabled: bool,
    editing: Option<LabelEdit>,
    clipboard: Option<Clipboard>,
    fit_requested: bool,
    pan_delta: Vec2,
    gesture_active: bool,
    jobs: JobRunner<String>,
    pending: Option<PendingAction>,
}

impl DiagramApp {
    /// Build the application, opening the startup project when requested.
    pub fn new(context: &eframe::CreationContext<'_>, options: CliOptions) -> Self {
        let identity = AppIdentity {
            id: "diagramcraft",
            name: "DiagramCraft",
            version: APP_VERSION,
            tagline: "Diagram editor for shapes, connectors and SVG/PDF export",
            repository: Some("https://github.com/emilio-n8n/diagramcraft"),
        };
        let shell = AppShell::new(identity);
        let document = if options.sample {
            Document::sample()
        } else {
            Document::default()
        };
        let mut app = Self {
            shell,
            host: DocumentHost::new(document, FORMAT, SCHEMA_VERSION, APP_VERSION),
            selected: Vec::new(),
            selected_connector: None,
            tool: Tool::Select,
            routing: Routing::Straight,
            connect_from: None,
            drag: None,
            panning: false,
            pan: Vec2::new(24.0, 24.0),
            zoom: 1.0,
            show_grid: true,
            snap_enabled: true,
            editing: None,
            clipboard: None,
            fit_requested: false,
            pan_delta: Vec2::ZERO,
            gesture_active: false,
            jobs: JobRunner::new(),
            pending: None,
        };
        app.shell.install_theme(&context.egui_ctx);
        app.shell
            .set_status("Tool: Select — double-click a shape to edit its label.");
        if let Some(path) = options.open {
            app.load_path(path);
        }
        app
    }

    fn load_path(&mut self, path: PathBuf) {
        match DocumentHost::from_file(path.clone(), FORMAT, SCHEMA_VERSION, APP_VERSION) {
            Ok(mut host) => {
                host.apply(|document: &mut Document| document.normalize());
                self.host = host;
                self.reset_interaction();
                self.shell.set_status(format!("Opened {}", path.display()));
            }
            Err(error) => {
                self.shell
                    .notify(Tone::Error, "Could not open project", error.to_string());
            }
        }
    }

    fn reset_interaction(&mut self) {
        self.finish_gesture();
        self.selected.clear();
        self.selected_connector = None;
        self.connect_from = None;
        self.drag = None;
        self.panning = false;
        self.editing = None;
    }

    fn new_document(&mut self) {
        self.reset_interaction();
        self.host.reset(Document::default());
        self.shell.set_status("New document");
    }

    fn finish_gesture(&mut self) {
        if self.gesture_active {
            self.host.history_mut().end_gesture();
            self.gesture_active = false;
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.connect_from = None;
        self.shell.set_status(match tool {
            Tool::Select => "Tool: Select — drag to move, double-click a label to edit.",
            Tool::Connect => "Tool: Connect — click two shapes to link them.",
            _ => "Tool: click the canvas to place a shape.",
        });
    }

    fn save(&mut self) -> bool {
        self.finish_gesture();
        if self.host.path().is_none() {
            return self.save_as();
        }
        match self.host.save() {
            Ok(path) => {
                self.shell.set_status(format!("Saved {}", path.display()));
                true
            }
            Err(error) => {
                self.shell
                    .notify(Tone::Error, "Could not save project", error.to_string());
                false
            }
        }
    }

    fn save_as(&mut self) -> bool {
        self.finish_gesture();
        let default_name = format!("{}.json", slugify(&self.host.document().title));
        let Some(path) = dialogs::save_project("Save project", &default_name, "json") else {
            return false;
        };
        match self.host.save_as(path) {
            Ok(path) => {
                self.shell.set_status(format!("Saved {}", path.display()));
                true
            }
            Err(error) => {
                self.shell
                    .notify(Tone::Error, "Could not save project", error.to_string());
                false
            }
        }
    }

    fn export_svg(&mut self) {
        self.finish_gesture();
        let Some(path) = dialogs::save_project("Export SVG", "diagram.svg", "svg") else {
            return;
        };
        let output = svg::render(self.host.document());
        match std::fs::write(&path, output) {
            Ok(()) => self
                .shell
                .set_status(format!("Exported SVG to {}", path.display())),
            Err(error) => self
                .shell
                .notify(Tone::Error, "Could not export SVG", error.to_string()),
        }
    }

    fn export_pdf(&mut self) {
        self.finish_gesture();
        let Some(path) = dialogs::save_project("Export PDF", "diagram.pdf", "pdf") else {
            return;
        };
        let output = pdf::render(self.host.document());
        match std::fs::write(&path, output) {
            Ok(()) => self
                .shell
                .set_status(format!("Exported PDF to {}", path.display())),
            Err(error) => self
                .shell
                .notify(Tone::Error, "Could not export PDF", error.to_string()),
        }
    }

    fn request_new(&mut self) {
        self.finish_gesture();
        if self.host.is_dirty() {
            self.pending = Some(PendingAction::New);
        } else {
            self.new_document();
        }
    }

    fn request_open(&mut self) {
        self.finish_gesture();
        let Some(path) = dialogs::open_project("Open project", &["json"]) else {
            return;
        };
        if self.host.is_dirty() {
            self.pending = Some(PendingAction::Open(path));
        } else {
            self.load_path(path);
        }
    }

    fn perform(&mut self, action: PendingAction) {
        match action {
            PendingAction::New => self.new_document(),
            PendingAction::Open(path) => self.load_path(path),
        }
    }

    fn resolve_pending(&mut self, choice: PendingChoice) {
        let Some(action) = self.pending.take() else {
            return;
        };
        match choice {
            PendingChoice::Cancel => {}
            PendingChoice::Discard => self.perform(action),
            PendingChoice::Save => {
                if self.save() {
                    self.perform(action);
                } else {
                    self.pending = Some(action);
                }
            }
        }
    }

    fn clamp_selection(&mut self) {
        let document = self.host.document();
        self.selected
            .retain(|id| document.find_shape(*id).is_some());
        if self
            .selected_connector
            .is_some_and(|id| document.find_connector(id).is_none())
        {
            self.selected_connector = None;
        }
        if self
            .connect_from
            .is_some_and(|id| document.find_shape(id).is_none())
        {
            self.connect_from = None;
        }
    }

    fn delete_selection(&mut self) {
        self.finish_gesture();
        let shapes = self.selected.clone();
        let links: Vec<u64> = self.selected_connector.into_iter().collect();
        if shapes.is_empty() && links.is_empty() {
            return;
        }
        let changed = self.host.apply(|document| {
            document.remove_shapes(&shapes);
            document.remove_connectors(&links);
        });
        if changed {
            self.selected.clear();
            self.selected_connector = None;
            self.shell.set_status("Deleted the selection");
        }
    }

    fn copy_selection(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let clipboard = self.host.document().copy(&self.selected);
        if clipboard.shapes.is_empty() {
            return;
        }
        self.clipboard = Some(clipboard);
        self.shell.set_status("Copied the selection");
    }

    fn paste_clipboard(&mut self) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let mut ids = Vec::new();
        let changed = self.host.apply(|document| {
            ids = document.paste(&clipboard, PASTE_OFFSET, PASTE_OFFSET);
        });
        if changed {
            self.selected = ids;
            self.selected_connector = None;
            self.shell.set_status("Pasted the clipboard");
        }
    }

    fn duplicate_selection(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let ids = self.selected.clone();
        let mut new_ids = Vec::new();
        let changed = self.host.apply(|document| {
            new_ids = document.duplicate_shapes(&ids, PASTE_OFFSET, PASTE_OFFSET);
        });
        if changed {
            self.selected = new_ids;
            self.selected_connector = None;
            self.shell.set_status("Duplicated the selection");
        }
    }

    fn select_all(&mut self) {
        self.host.document().shapes.iter().for_each(|shape| {
            if !self.selected.contains(&shape.id) {
                self.selected.push(shape.id);
            }
        });
        self.selected_connector = None;
    }

    fn start_label_edit(&mut self, id: u64) {
        let Some(shape) = self.host.document().find_shape(id) else {
            return;
        };
        self.editing = Some(LabelEdit {
            id,
            text: shape.label.clone(),
            focus: true,
        });
    }

    fn set_routing(&mut self, routing: Routing) {
        self.routing = routing;
        self.shell
            .set_status(format!("New connectors: {}", routing.label()));
        if let Some(id) = self.selected_connector {
            self.host.apply(|document| {
                document.set_routing(id, routing);
            });
        }
    }

    fn start_job(&mut self) {
        let result = self.jobs.spawn("Export job", |context| {
            for step in 0..=20 {
                context
                    .check_cancelled()
                    .map_err(|error| error.to_string())?;
                context.set_progress_message(step as f32 / 20.0, format!("Step {step}/20"));
                std::thread::sleep(std::time::Duration::from_millis(60));
            }
            Ok("Processed 20 steps".to_owned())
        });
        if let Err(error) = result {
            self.shell
                .notify(Tone::Error, "Could not start job", error.to_string());
        }
    }

    fn poll_jobs(&mut self) {
        for (_, result) in self.jobs.poll() {
            match result {
                Ok(message) => self.shell.notify(Tone::Success, "Job finished", message),
                Err(error) => self.shell.notify(Tone::Error, "Job failed", error),
            }
        }
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::New => self.request_new(),
            Action::Open => self.request_open(),
            Action::Save => {
                self.save();
            }
            Action::SaveAs => {
                self.save_as();
            }
            Action::ExportSvg => self.export_svg(),
            Action::ExportPdf => self.export_pdf(),
            Action::Undo => {
                self.finish_gesture();
                if self.host.undo() {
                    self.clamp_selection();
                    self.shell.set_status("Undo");
                }
            }
            Action::Redo => {
                self.finish_gesture();
                if self.host.redo() {
                    self.clamp_selection();
                    self.shell.set_status("Redo");
                }
            }
            Action::Delete => self.delete_selection(),
            Action::EditLabel => {
                if let Some(&id) = self.selected.first() {
                    self.start_label_edit(id);
                }
            }
            Action::Copy => self.copy_selection(),
            Action::Paste => self.paste_clipboard(),
            Action::Duplicate => self.duplicate_selection(),
            Action::SelectAll => self.select_all(),
            Action::Fit => self.fit_requested = true,
            Action::ToggleGrid => self.show_grid = !self.show_grid,
            Action::ToggleSnap => self.snap_enabled = !self.snap_enabled,
            Action::Align(mode) => {
                let ids = self.selected.clone();
                self.host.apply(|document| {
                    document.align_shapes(&ids, mode);
                });
            }
            Action::Distribute(axis) => {
                let ids = self.selected.clone();
                self.host.apply(|document| {
                    document.distribute_shapes(&ids, axis);
                });
            }
            Action::About => self.shell.open_about(),
            Action::Settings => self.shell.open_settings(),
        }
    }

    fn handle_shortcuts(&mut self, ui: &egui::Ui) {
        if ui.ctx().egui_wants_keyboard_input() || self.editing.is_some() {
            return;
        }
        let action = ui.input_mut(|input| {
            if input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S,
            ) {
                Some(Action::SaveAs)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::S) {
                Some(Action::Save)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::O) {
                Some(Action::Open)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::N) {
                Some(Action::New)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
                Some(Action::Undo)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y) {
                Some(Action::Redo)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::C) {
                Some(Action::Copy)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::V) {
                Some(Action::Paste)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::D) {
                Some(Action::Duplicate)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::A) {
                Some(Action::SelectAll)
            } else if input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::E,
            ) {
                Some(Action::ExportSvg)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
            {
                Some(Action::Delete)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::F) {
                Some(Action::Fit)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                Some(Action::EditLabel)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                self.editing = None;
                self.selected.clear();
                self.selected_connector = None;
                self.connect_from = None;
                None
            } else {
                None
            }
        });
        if let Some(action) = action {
            self.handle_action(action);
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let title = self.host.display_name();
        let dirty = self.host.is_dirty();
        let can_undo = self.host.can_undo();
        let can_redo = self.host.can_redo();
        let mut action = None;
        self.shell.top_bar(ui, &title, dirty, |ui| {
            ui.menu_button("File", |ui| {
                for (label, candidate) in [
                    ("New", Some(Action::New)),
                    ("Open...", Some(Action::Open)),
                    ("Save", Some(Action::Save)),
                    ("Save As...", Some(Action::SaveAs)),
                    ("Export SVG...", Some(Action::ExportSvg)),
                    ("Export PDF...", Some(Action::ExportPdf)),
                ] {
                    if candidate.is_some() && ui.button(label).clicked() {
                        action = candidate;
                        ui.close();
                    }
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .clicked()
                {
                    action = Some(Action::Undo);
                    ui.close();
                }
                if ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .clicked()
                {
                    action = Some(Action::Redo);
                    ui.close();
                }
                for (label, candidate) in [
                    ("Edit label...", Some(Action::EditLabel)),
                    ("Duplicate", Some(Action::Duplicate)),
                    ("Delete", Some(Action::Delete)),
                ] {
                    if ui.button(label).clicked() {
                        action = candidate;
                        ui.close();
                    }
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Zoom to fit").clicked() {
                    action = Some(Action::Fit);
                    ui.close();
                }
                if ui
                    .button(if self.show_grid {
                        "Hide grid"
                    } else {
                        "Show grid"
                    })
                    .clicked()
                {
                    action = Some(Action::ToggleGrid);
                    ui.close();
                }
                if ui
                    .button(if self.snap_enabled {
                        "Disable snap"
                    } else {
                        "Enable snap"
                    })
                    .clicked()
                {
                    action = Some(Action::ToggleSnap);
                    ui.close();
                }
            });
            if ui.button("Export PDF").clicked() {
                action = Some(Action::ExportPdf);
            }
            if ui.button("About").clicked() {
                action = Some(Action::About);
            }
            if ui.button("Settings").clicked() {
                action = Some(Action::Settings);
            }
        });
        if let Some(action) = action {
            self.handle_action(action);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let tool = self.tool.label();
        let zoom = format!("{}%", (self.zoom * 100.0).round() as i32);
        let shapes = self.host.document().shapes.len();
        let links = self.host.document().connectors.len();
        let grid = if self.show_grid {
            "grid on"
        } else {
            "grid off"
        };
        let snap = if self.snap_enabled {
            "snap on"
        } else {
            "snap off"
        };
        let running = self.jobs.jobs();
        self.shell.status_bar_with(ui, |ui| {
            ui.label(RichText::new(format!("Tool: {tool}")).color(theme::ACCENT));
            ui.label(RichText::new(zoom).color(theme::MUTED));
            ui.label(
                RichText::new(format!("{shapes} shapes · {links} links · {grid} · {snap}"))
                    .color(theme::MUTED),
            );
            for job in running {
                widgets::job_row(ui, job);
            }
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        let shapes: Vec<(u64, String)> = self
            .host
            .document()
            .shapes
            .iter()
            .map(|shape| (shape.id, shape.label.clone()))
            .collect();
        let selection_count = self.selected.len();

        egui::Panel::left("palette-panel")
            .resizable(true)
            .default_size(214.0)
            .min_size(170.0)
            .show(ui, |ui| {
                widgets::section_label(ui, "Tools");
                if ui
                    .selectable_label(self.tool == Tool::Select, "Select")
                    .clicked()
                {
                    self.set_tool(Tool::Select);
                }
                if ui
                    .selectable_label(self.tool == Tool::Connect, "Connect")
                    .clicked()
                {
                    self.set_tool(Tool::Connect);
                }

                ui.separator();
                widgets::section_label(ui, "Shapes");
                for kind in ShapeKind::ALL {
                    let tool = Tool::from_kind(kind);
                    if ui
                        .selectable_label(self.tool == tool, kind.label())
                        .clicked()
                    {
                        self.set_tool(tool);
                    }
                }

                ui.separator();
                widgets::section_label(ui, "Routing");
                ui.horizontal(|ui| {
                    for routing in [Routing::Straight, Routing::Orthogonal] {
                        if ui
                            .selectable_label(self.routing == routing, routing.label())
                            .clicked()
                        {
                            self.set_routing(routing);
                        }
                    }
                });

                ui.separator();
                widgets::section_label(ui, "Arrange");
                ui.add_enabled_ui(selection_count >= 2, |ui| {
                    for mode in AlignMode::ALL {
                        if ui.button(mode.label()).clicked() {
                            self.handle_action(Action::Align(mode));
                        }
                    }
                });
                ui.add_enabled_ui(selection_count >= 3, |ui| {
                    for axis in [Axis::Horizontal, Axis::Vertical] {
                        if ui.button(axis.label()).clicked() {
                            self.handle_action(Action::Distribute(axis));
                        }
                    }
                });

                ui.separator();
                widgets::section_label(ui, "Objects");
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (id, label) in &shapes {
                            let selected = self.selected.contains(id);
                            if ui.selectable_label(selected, label).clicked() {
                                if ui.input(|input| input.modifiers.shift) {
                                    if selected {
                                        self.selected.retain(|value| value != id);
                                    } else {
                                        self.selected.push(*id);
                                    }
                                } else {
                                    self.selected = vec![*id];
                                }
                                self.selected_connector = None;
                            }
                        }
                    });

                ui.separator();
                ui.checkbox(&mut self.show_grid, "Show grid");
                ui.checkbox(&mut self.snap_enabled, "Snap to grid");
                ui.label(
                    RichText::new(
                        "Drag shapes to move · drag grips to resize · double-click a shape to rename · Del deletes",
                    )
                    .size(10.0)
                    .color(theme::FAINT),
                );
            });
    }

    fn central_panel(&mut self, ui: &mut egui::Ui) {
        let mut gained_focus = false;
        let mut lost_focus = false;

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Diagram");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::toolbar_button(ui, "Fit", "Zoom to fit the whole diagram").clicked()
                    {
                        self.fit_requested = true;
                    }
                    if widgets::toolbar_button(ui, "Export PDF", "Vector PDF export").clicked() {
                        self.export_pdf();
                    }
                    if widgets::toolbar_button(ui, "Export SVG", "Standalone SVG export").clicked()
                    {
                        self.export_svg();
                    }
                    if widgets::toolbar_button(ui, "Job", "Start a demo background job").clicked() {
                        self.start_job();
                    }
                });
            });
            ui.add_space(4.0);

            {
                let document = self.host.history_mut().current_mut();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut document.title).hint_text("Diagram title"),
                );
                gained_focus |= response.gained_focus();
                lost_focus |= response.lost_focus();
            }
            ui.add_space(4.0);

            self.canvas(ui);
        });

        if gained_focus {
            self.gesture_active = true;
        }
        if lost_focus {
            self.finish_gesture();
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        if self.fit_requested {
            self.fit_requested = false;
            let bounds = self
                .host
                .document()
                .bounds()
                .unwrap_or((0.0, 0.0, 800.0, 600.0));
            let (pan_x, pan_y, zoom) = geometry::fit_view(
                bounds,
                (rect.width(), rect.height()),
                40.0,
                MIN_ZOOM,
                MAX_ZOOM,
            );
            self.pan = Vec2::new(pan_x, pan_y);
            self.zoom = zoom;
        }

        let zoom_delta = ui.input(|input| input.zoom_delta());
        if (zoom_delta - 1.0).abs() > f32::EPSILON {
            let hover = ui
                .input(|input| input.pointer.hover_pos())
                .unwrap_or(rect.center());
            let before = View {
                origin: rect.min + self.pan,
                zoom: self.zoom,
            };
            let world = before.to_world(hover);
            self.zoom = (self.zoom * zoom_delta).clamp(MIN_ZOOM, MAX_ZOOM);
            self.pan = hover - rect.min - Vec2::new(world.0 as f32, world.1 as f32) * self.zoom;
        }

        let view = View {
            origin: rect.min + self.pan,
            zoom: self.zoom,
        };
        let cursor = match self.tool {
            Tool::Select => egui::CursorIcon::Default,
            Tool::Connect => egui::CursorIcon::Crosshair,
            _ => egui::CursorIcon::Copy,
        };
        if response.hovered() {
            ui.ctx().set_cursor_icon(cursor);
        }

        let hover_pos = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|pos| rect.contains(*pos));
        let press_origin = ui
            .input(|input| input.pointer.press_origin())
            .filter(|pos| rect.contains(*pos));
        let pressed = ui.input(|input| input.pointer.primary_pressed());
        let down = ui.input(|input| input.pointer.primary_down());
        let released = ui.input(|input| input.pointer.primary_released());
        self.pan_delta = ui.input(|input| input.pointer.delta());

        if pressed && let Some(pos) = hover_pos.or(press_origin) {
            let world = view.to_world(pos);
            self.handle_press(world, ui.input(|input| input.modifiers.shift));
        }
        if down
            && !pressed
            && let Some(pos) = hover_pos
        {
            self.handle_drag(view.to_world(pos));
        }
        if released {
            self.end_drag();
        }
        if response.double_clicked()
            && let Some(pos) = hover_pos.or(press_origin)
        {
            self.handle_double_click(view.to_world(pos));
        }

        self.paint_canvas(ui, rect, view);
    }

    fn handle_press(&mut self, world: (f64, f64), shift: bool) {
        let tolerance = HANDLE_TOLERANCE / f64::from(self.zoom);
        let document = self.host.document();

        // Resize grip of the single selected shape.
        if self.selected.len() == 1
            && let Some(shape) = document.find_shape(self.selected[0])
            && let Some(handle) = document::handle_at(shape, world.0, world.1, tolerance)
        {
            self.drag = Some(Drag::Resize {
                id: shape.id,
                handle,
                origin: shape.rect(),
                start: world,
            });
            self.host.history_mut().begin_gesture();
            return;
        }

        // Waypoint of the selected connector.
        if let Some(link) = self
            .selected_connector
            .and_then(|id| document.find_connector(id))
        {
            for (index, point) in link.waypoints.iter().enumerate() {
                if geometry::point_segment_distance(
                    world.0, world.1, point.x, point.y, point.x, point.y,
                ) <= tolerance
                {
                    self.drag = Some(Drag::Waypoint { id: link.id, index });
                    self.host.history_mut().begin_gesture();
                    return;
                }
            }
        }

        // Shape placement: a shape tool adds a new shape anywhere on the canvas.
        if let Some(kind) = self.tool.shape_kind() {
            let x = document::snap_if(self.snap_enabled, world.0);
            let y = document::snap_if(self.snap_enabled, world.1);
            let next = document.next_shape;
            if self.host.apply(|document| {
                document.add_shape(kind, x, y);
            }) {
                self.selected = vec![next];
                self.selected_connector = None;
                self.shell
                    .set_status(format!("Added a {} shape", kind.label()));
            }
            return;
        }

        // Connector selection.
        if self.tool == Tool::Select
            && let Some(id) =
                document.hit_connector(world.0, world.1, CONNECTOR_TOLERANCE / f64::from(self.zoom))
        {
            self.selected.clear();
            self.selected_connector = Some(id);
            self.shell.set_status(format!("Selected connector {id}"));
            return;
        }

        if let Some(id) = self.hit_shape(world.0, world.1) {
            if self.tool == Tool::Connect {
                if let Some(from) = self.connect_from.take() {
                    if from != id {
                        let routing = self.routing;
                        let added = self.host.apply(|document| {
                            document.add_connector_with_routing(from, id, routing);
                        });
                        self.shell.set_status(if added {
                            format!("Linked shape {from} to shape {id}")
                        } else {
                            "Shapes are already linked".to_owned()
                        });
                    }
                } else {
                    self.connect_from = Some(id);
                    self.shell.set_status("Connect: pick a second shape");
                }
                return;
            }

            let already_selected = self.selected.contains(&id);
            if shift {
                if already_selected {
                    self.selected.retain(|value| *value != id);
                } else {
                    self.selected.push(id);
                }
            } else if !already_selected {
                self.selected = vec![id];
            }
            self.selected_connector = None;

            if self.selected.contains(&id) {
                let origins: Vec<(u64, f64, f64)> = self
                    .selected
                    .iter()
                    .filter_map(|id| self.host.document().find_shape(*id))
                    .map(|shape| (shape.id, shape.x, shape.y))
                    .collect();
                self.drag = Some(Drag::Move {
                    start: world,
                    origins,
                });
                self.host.history_mut().begin_gesture();
            }
            return;
        }

        if self.tool == Tool::Select && !shift {
            self.selected.clear();
            self.selected_connector = None;
            self.panning = true;
        }
    }

    fn handle_drag(&mut self, world: (f64, f64)) {
        match self.drag.clone() {
            Some(Drag::Move { start, origins }) => {
                let dx = world.0 - start.0;
                let dy = world.1 - start.1;
                let snap = self.snap_enabled;
                let positions: Vec<(u64, f64, f64)> = origins
                    .iter()
                    .map(|(id, x, y)| {
                        (
                            *id,
                            document::snap_if(snap, x + dx),
                            document::snap_if(snap, y + dy),
                        )
                    })
                    .collect();
                self.host
                    .history_mut()
                    .current_mut()
                    .move_shapes_to(&positions);
            }
            Some(Drag::Resize {
                id,
                handle,
                origin,
                start,
            }) => {
                let (dx, dy) = (world.0 - start.0, world.1 - start.1);
                let rect = geometry::resize_rect(origin, handle, dx, dy);
                let snapped = (
                    document::snap_if(self.snap_enabled, rect.0),
                    document::snap_if(self.snap_enabled, rect.1),
                    document::snap_if(self.snap_enabled, rect.2),
                    document::snap_if(self.snap_enabled, rect.3),
                );
                self.host
                    .history_mut()
                    .current_mut()
                    .set_shape_rect(id, snapped);
            }
            Some(Drag::Waypoint { id, index }) => {
                let point = Point::new(
                    document::snap_if(self.snap_enabled, world.0),
                    document::snap_if(self.snap_enabled, world.1),
                );
                self.host
                    .history_mut()
                    .current_mut()
                    .move_waypoint(id, index, point);
            }
            None => {
                if self.panning {
                    self.pan += self.pan_delta;
                }
            }
        }
    }

    fn end_drag(&mut self) {
        if let Some(Drag::Move { .. } | Drag::Resize { .. } | Drag::Waypoint { .. }) = self.drag {
            self.host.history_mut().end_gesture();
        }
        self.drag = None;
        self.panning = false;
    }

    fn handle_double_click(&mut self, world: (f64, f64)) {
        if let Some(id) = self.hit_shape(world.0, world.1) {
            self.selected = vec![id];
            self.selected_connector = None;
            self.start_label_edit(id);
            return;
        }
        if let Some(id) = self
            .host
            .document()
            .hit_connector(world.0, world.1, CONNECTOR_TOLERANCE)
        {
            let orthogonal = self
                .host
                .document()
                .find_connector(id)
                .is_some_and(|link| link.routing == Routing::Orthogonal);
            if orthogonal {
                let point = Point::new(
                    document::snap_if(self.snap_enabled, world.0),
                    document::snap_if(self.snap_enabled, world.1),
                );
                self.host.apply(|document| {
                    document.add_waypoint(id, point);
                });
            }
            self.selected.clear();
            self.selected_connector = Some(id);
        }
    }

    fn hit_shape(&self, x: f64, y: f64) -> Option<u64> {
        self.host
            .document()
            .shapes
            .iter()
            .rev()
            .find(|shape| document::contains_point(shape, x, y))
            .map(|shape| shape.id)
    }

    fn paint_canvas(&self, ui: &egui::Ui, rect: egui::Rect, view: View) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, theme::SUNKEN);

        if self.show_grid {
            let step = GRID_STEP * view.zoom as f64;
            if step >= 8.0 {
                let grid_stroke =
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(94, 102, 117, 28));
                let mut gx = rect.min.x + (view.origin.x - rect.min.x).rem_euclid(step as f32);
                while gx <= rect.max.x {
                    painter.line_segment(
                        [Pos2::new(gx, rect.min.y), Pos2::new(gx, rect.max.y)],
                        grid_stroke,
                    );
                    gx += step as f32;
                }
                let mut gy = rect.min.y + (view.origin.y - rect.min.y).rem_euclid(step as f32);
                while gy <= rect.max.y {
                    painter.line_segment(
                        [Pos2::new(rect.min.x, gy), Pos2::new(rect.max.x, gy)],
                        grid_stroke,
                    );
                    gy += step as f32;
                }
            }
        }

        let document = self.host.document();
        if document.shapes.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Pick a shape in the palette, then click here to place it",
                FontId::proportional(14.0),
                theme::FAINT,
            );
        }

        for connector in &document.connectors {
            self.paint_connector(painter, connector, view);
        }
        for shape in &document.shapes {
            let selected = self.selected.contains(&shape.id);
            paint_shape(painter, shape, view, selected);
        }

        if self.selected.len() == 1
            && let Some(shape) = document.find_shape(self.selected[0])
        {
            for handle in Handle::ALL {
                let point = document::handle_point(shape, handle);
                let screen = view.to_screen(point.x, point.y);
                painter.rect_filled(
                    egui::Rect::from_center_size(screen, Vec2::splat(8.0)),
                    1.0,
                    theme::ACCENT_LIGHT,
                );
            }
        }
    }

    fn paint_connector(&self, painter: &egui::Painter, connector: &Connector, view: View) {
        let points = self.host.document().connector_points(connector);
        if points.len() < 2 {
            return;
        }
        let highlighted = self.selected_connector == Some(connector.id);
        let stroke = Stroke::new(
            if highlighted { 3.0 } else { 2.0 },
            if highlighted {
                theme::ACCENT_LIGHT
            } else {
                theme::MUTED
            },
        );
        let screen: Vec<Pos2> = points
            .iter()
            .map(|point| view.to_screen(point.x, point.y))
            .collect();
        for pair in screen.windows(2) {
            painter.line_segment([pair[0], pair[1]], stroke);
        }
        let last = screen.len() - 1;
        let direction = screen[last] - screen[last - 1];
        if direction.length() > 1.0 {
            let unit = direction.normalized();
            let perpendicular = Vec2::new(-unit.y, unit.x);
            let back = screen[last] - unit * 10.0;
            painter.add(egui::epaint::Shape::convex_polygon(
                vec![
                    screen[last],
                    back + perpendicular * 5.0,
                    back - perpendicular * 5.0,
                ],
                stroke.color,
                Stroke::NONE,
            ));
        }
        if highlighted {
            for point in &connector.waypoints {
                let screen = view.to_screen(point.x, point.y);
                painter.circle_filled(screen, 5.0, theme::ACCENT);
            }
        }
    }

    fn label_window(&mut self, ctx: &egui::Context) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        let id = edit.id;
        let label_id = egui::Id::unique(format!("label-edit-{id}"));
        let mut commit = false;
        let mut cancel = false;
        let mut open = true;

        egui::Window::new("Edit label")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                if edit.focus {
                    ui.memory_mut(|memory| memory.request_focus(label_id));
                    edit.focus = false;
                }
                let response = ui.add(
                    egui::TextEdit::singleline(&mut edit.text)
                        .id(label_id)
                        .hint_text("Shape label"),
                );
                let focused = ui.memory(|memory| memory.has_focus(label_id));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked()
                        || (focused && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                        || (response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    {
                        commit = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if commit {
            let text = self
                .editing
                .as_ref()
                .map_or_else(String::new, |edit| edit.text.clone());
            self.host.apply(|document| {
                document.set_label(id, &text);
            });
            self.editing = None;
            self.shell.set_status("Label updated");
        } else if cancel || !open {
            self.editing = None;
        }
    }

    fn unsaved_window(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let mut open = true;
        let mut choice = None;
        egui::Window::new("Unsaved changes")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.label("The current project has unsaved changes.");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Save and continue").clicked() {
                        choice = Some(PendingChoice::Save);
                    }
                    if ui.button("Discard").clicked() {
                        choice = Some(PendingChoice::Discard);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(PendingChoice::Cancel);
                    }
                });
            });
        if let Some(choice) = choice {
            self.resolve_pending(choice);
        } else if !open {
            self.pending = None;
        }
    }
}

impl eframe::App for DiagramApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_jobs();
        self.handle_shortcuts(ui);
        if self.gesture_active {
            self.host.history_mut().begin_gesture();
        }
        self.top_bar(ui);
        self.status_bar(ui);
        self.left_panel(ui);
        self.central_panel(ui);
        self.label_window(ui.ctx());
        self.unsaved_window(ui.ctx());
        self.shell.show_windows(ui.ctx());
        self.shell
            .update_title(ui.ctx(), &self.host.display_name(), self.host.is_dirty());
    }
}

/// Draw a single shape and its centred label.
fn paint_shape(painter: &egui::Painter, shape: &Shape, view: View, selected: bool) {
    let top_left = view.to_screen(shape.x, shape.y);
    let bottom_right = view.to_screen(shape.x + shape.w, shape.y + shape.h);
    let rect = egui::Rect::from_min_max(top_left, bottom_right);
    let (r, g, b) = document::rgb(shape.color);
    let color = Color32::from_rgb(r, g, b);
    let stroke = Stroke::new(
        if selected { 2.5 } else { 1.5 },
        if selected { theme::ACCENT_LIGHT } else { color },
    );
    let fill = Color32::from_rgba_unmultiplied(r, g, b, 45);

    match shape.kind {
        ShapeKind::Rectangle | ShapeKind::Note => {
            painter.rect(
                rect,
                egui::CornerRadius::same(6),
                fill,
                stroke,
                egui::StrokeKind::Middle,
            );
            if shape.kind == ShapeKind::Note {
                let fold = 16.0 * view.zoom;
                let corner = Pos2::new(rect.max.x - fold, rect.min.y + fold);
                painter.line_segment([Pos2::new(corner.x, rect.min.y), corner], stroke);
                painter.line_segment([corner, Pos2::new(rect.max.x, corner.y)], stroke);
            }
        }
        ShapeKind::Ellipse => {
            painter.add(egui::epaint::EllipseShape {
                center: rect.center(),
                radius: rect.size() / 2.0,
                fill,
                stroke,
                angle: 0.0,
            });
        }
        ShapeKind::Diamond => {
            let center = rect.center();
            painter.add(egui::epaint::Shape::convex_polygon(
                vec![
                    Pos2::new(center.x, rect.min.y),
                    Pos2::new(rect.max.x, center.y),
                    Pos2::new(center.x, rect.max.y),
                    Pos2::new(rect.min.x, center.y),
                ],
                fill,
                stroke,
            ));
        }
    }

    let font_size = (14.0 * view.zoom).clamp(8.0, 26.0);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        &shape.label,
        FontId::proportional(font_size),
        theme::TEXT,
    );
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "untitled".to_owned()
    } else {
        slug.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_round_trips_world_coordinates() {
        let view = View {
            origin: Pos2::new(100.0, 80.0),
            zoom: 2.0,
        };
        let screen = view.to_screen(50.0, 30.0);
        assert_eq!(screen, Pos2::new(200.0, 140.0));
        assert_eq!(view.to_world(screen), (50.0, 30.0));
    }

    #[test]
    fn tool_maps_shape_kinds() {
        assert_eq!(Tool::from_kind(ShapeKind::Diamond), Tool::Diamond);
        assert_eq!(Tool::Diamond.shape_kind(), Some(ShapeKind::Diamond));
        assert_eq!(Tool::Select.shape_kind(), None);
    }

    #[test]
    fn slugify_produces_safe_file_names() {
        assert_eq!(slugify("DiagramCraft sample"), "diagramcraft-sample");
        assert_eq!(slugify("  "), "untitled");
    }
}
