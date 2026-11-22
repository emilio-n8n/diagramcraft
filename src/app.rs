//! DiagramCraft UI and project lifecycle.

use std::path::PathBuf;
use std::time::Duration;

use craftkit_io::DocumentHost;
use craftkit_jobs::JobRunner;
use craftkit_ui::cli::CliOptions;
use craftkit_ui::eframe::{self, egui};
use craftkit_ui::egui::{
    Align, Align2, Color32, FontId, Layout, Pos2, RichText, Stroke, StrokeKind, Vec2,
};
use craftkit_ui::{AppIdentity, AppShell, Tone, dialogs, theme, widgets};

use crate::document::{self, Document, Shape, ShapeKind};
use crate::svg;

const FORMAT: &str = "diagramcraft";
const SCHEMA_VERSION: u32 = 1;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    New,
    Open,
    Save,
    SaveAs,
    ExportSvg,
    Undo,
    Redo,
    Delete,
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

/// State of an in-progress shape move.
#[derive(Debug, Clone, Copy)]
struct DragState {
    id: u64,
    grab_x: f64,
    grab_y: f64,
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

/// The DiagramCraft application state.
pub struct DiagramApp {
    shell: AppShell,
    host: DocumentHost<Document>,
    selected: Option<u64>,
    tool: Tool,
    connect_from: Option<u64>,
    drag: Option<DragState>,
    panning: bool,
    pan: Vec2,
    zoom: f32,
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
            tagline: "Diagram editor for shapes, connectors and SVG export",
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
            selected: None,
            tool: Tool::Select,
            connect_from: None,
            drag: None,
            panning: false,
            pan: Vec2::new(24.0, 24.0),
            zoom: 1.0,
            gesture_active: false,
            jobs: JobRunner::new(),
            pending: None,
        };
        app.shell.install_theme(&context.egui_ctx);
        app.shell
            .set_status("Tool: Select — click a shape to select, drag the background to pan.");
        if let Some(path) = options.open {
            app.load_path(path);
        }
        app
    }

    fn load_path(&mut self, path: PathBuf) {
        match DocumentHost::from_file(path.clone(), FORMAT, SCHEMA_VERSION, APP_VERSION) {
            Ok(host) => {
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
        self.selected = None;
        self.connect_from = None;
        self.drag = None;
        self.panning = false;
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
        let status = match tool {
            Tool::Select => "Tool: Select — click to select, drag a shape to move it.".to_owned(),
            Tool::Connect => "Tool: Connect — click two shapes to link them.".to_owned(),
            kind => format!(
                "{}: click the canvas to place a shape (snapped to the grid).",
                kind.label()
            ),
        };
        self.shell.set_status(status);
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
        let svg = svg::render(self.host.document());
        match std::fs::write(&path, svg) {
            Ok(()) => self
                .shell
                .set_status(format!("Exported {}", path.display())),
            Err(error) => self
                .shell
                .notify(Tone::Error, "Could not export SVG", error.to_string()),
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
        if self
            .selected
            .is_some_and(|id| self.host.document().find_shape(id).is_none())
        {
            self.selected = None;
        }
        if self
            .connect_from
            .is_some_and(|id| self.host.document().find_shape(id).is_none())
        {
            self.connect_from = None;
        }
    }

    fn delete_selected(&mut self) {
        self.finish_gesture();
        let Some(id) = self.selected else {
            return;
        };
        if self.host.apply(|document| {
            document.remove_shape(id);
        }) {
            self.selected = None;
            self.connect_from = None;
            self.shell
                .set_status("Deleted shape and its attached connectors");
        }
    }

    fn start_job(&mut self) {
        let result = self.jobs.spawn("Demo job", |context| {
            for step in 0..=20 {
                context
                    .check_cancelled()
                    .map_err(|error| error.to_string())?;
                context.set_progress_message(step as f32 / 20.0, format!("Step {step}/20"));
                std::thread::sleep(Duration::from_millis(60));
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
            Action::Delete => self.delete_selected(),
            Action::About => self.shell.open_about(),
            Action::Settings => self.shell.open_settings(),
        }
    }

    fn handle_shortcuts(&mut self, ui: &egui::Ui) {
        if ui.ctx().egui_wants_keyboard_input() {
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
            } else if input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::E,
            ) {
                Some(Action::ExportSvg)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
                Some(Action::Undo)
            } else if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Y) {
                Some(Action::Redo)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
            {
                Some(Action::Delete)
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
                if ui.button("New").clicked() {
                    action = Some(Action::New);
                    ui.close();
                }
                if ui.button("Open...").clicked() {
                    action = Some(Action::Open);
                    ui.close();
                }
                if ui.button("Save").clicked() {
                    action = Some(Action::Save);
                    ui.close();
                }
                if ui.button("Save As...").clicked() {
                    action = Some(Action::SaveAs);
                    ui.close();
                }
                if ui.button("Export SVG...").clicked() {
                    action = Some(Action::ExportSvg);
                    ui.close();
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
                if ui.button("Delete shape").clicked() {
                    action = Some(Action::Delete);
                    ui.close();
                }
            });
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
        let objects = self.host.document().shapes.len();
        let links = self.host.document().connectors.len();
        let running = self.jobs.jobs();
        self.shell.status_bar_with(ui, |ui| {
            ui.label(RichText::new(format!("Tool: {tool}")).color(theme::ACCENT));
            ui.label(RichText::new(zoom).color(theme::MUTED));
            ui.label(
                RichText::new(format!("{objects} shapes · {links} links")).color(theme::MUTED),
            );
            for job in running {
                widgets::job_row(ui, job);
            }
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        let entries: Vec<(u64, String)> = self
            .host
            .document()
            .shapes
            .iter()
            .map(|shape| (shape.id, shape.label.clone()))
            .collect();

        egui::Panel::left("palette-panel")
            .resizable(true)
            .default_size(210.0)
            .min_size(160.0)
            .show(ui, |ui| {
                widgets::section_label(ui, "Tools");
                ui.add_space(2.0);
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
                ui.add_space(2.0);
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
                widgets::section_label(ui, "Objects");
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (id, label) in &entries {
                            if ui
                                .selectable_label(self.selected == Some(*id), label)
                                .clicked()
                            {
                                self.selected = Some(*id);
                            }
                        }
                    });

                ui.separator();
                ui.label(
                    RichText::new(
                        "Drag the background to pan · scroll to zoom · Delete removes a shape",
                    )
                    .size(10.5)
                    .color(theme::FAINT),
                );
            });
    }

    fn central_panel(&mut self, ui: &mut egui::Ui) {
        let mut gained_focus = false;
        let mut lost_focus = false;
        let mut run_job = false;

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Diagram");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::toolbar_button(ui, "Run job", "Start a background job").clicked() {
                        run_job = true;
                    }
                    if widgets::toolbar_button(ui, "Delete", "Delete the selected shape").clicked()
                    {
                        self.delete_selected();
                    }
                    if widgets::toolbar_button(ui, "Export SVG", "Export the diagram as SVG")
                        .clicked()
                    {
                        self.export_svg();
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
        if run_job {
            self.start_job();
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        // Zoom around the cursor so the point under it stays put.
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
            self.zoom = (self.zoom * zoom_delta).clamp(0.25, 4.0);
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

        let primary_pressed = ui.input(|input| input.pointer.primary_pressed());
        let primary_down = ui.input(|input| input.pointer.primary_down());
        let primary_released = ui.input(|input| input.pointer.primary_released());
        let hover_pos = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|pos| rect.contains(*pos));
        let press_origin = ui
            .input(|input| input.pointer.press_origin())
            .filter(|pos| rect.contains(*pos));

        if primary_pressed && let Some(pos) = hover_pos.or(press_origin) {
            let world = view.to_world(pos);
            self.handle_press(world);
        }
        if primary_down && !primary_pressed {
            if let Some(drag) = self.drag {
                if let Some(pos) = hover_pos {
                    let world = view.to_world(pos);
                    let x = document::snap(world.0 + drag.grab_x);
                    let y = document::snap(world.1 + drag.grab_y);
                    self.host
                        .history_mut()
                        .current_mut()
                        .move_shape(drag.id, x, y);
                }
            } else if self.panning {
                self.pan += ui.input(|input| input.pointer.delta());
            }
        }
        if primary_released {
            if self.drag.is_some() {
                self.host.history_mut().end_gesture();
                self.drag = None;
            }
            self.panning = false;
        }

        self.paint_canvas(ui, rect, view);
    }

    fn handle_press(&mut self, world: (f64, f64)) {
        match self.tool {
            Tool::Select => {
                if let Some(id) = self.hit_test(world.0, world.1) {
                    self.selected = Some(id);
                    if let Some(shape) = self.host.document().find_shape(id) {
                        self.drag = Some(DragState {
                            id,
                            grab_x: shape.x - world.0,
                            grab_y: shape.y - world.1,
                        });
                        self.host.history_mut().begin_gesture();
                    }
                } else {
                    self.selected = None;
                    self.panning = true;
                }
            }
            Tool::Connect => {
                if let Some(id) = self.hit_test(world.0, world.1) {
                    if let Some(from) = self.connect_from.take() {
                        if from != id
                            && self.host.apply(|document| {
                                document.add_connector(from, id);
                            })
                        {
                            self.shell
                                .set_status(format!("Linked shape {from} to shape {id}"));
                        } else {
                            self.shell.set_status("Shapes are already linked");
                        }
                    } else {
                        self.connect_from = Some(id);
                        self.shell
                            .set_status("Connect: first shape chosen, pick a second one");
                    }
                } else {
                    self.connect_from = None;
                }
            }
            tool => {
                if let Some(kind) = tool.shape_kind() {
                    let x = document::snap(world.0);
                    let y = document::snap(world.1);
                    let next = self.host.document().next_shape;
                    if self.host.apply(|document| {
                        document.add_shape(kind, x, y);
                    }) {
                        self.selected = Some(next);
                        self.shell
                            .set_status(format!("Added a {} shape", kind.label()));
                    }
                }
            }
        }
    }

    fn hit_test(&self, x: f64, y: f64) -> Option<u64> {
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

        let step = 50.0 * view.zoom;
        if step >= 10.0 {
            let grid_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(94, 102, 117, 28));
            let mut gx = rect.min.x + (view.origin.x - rect.min.x).rem_euclid(step);
            while gx <= rect.max.x {
                painter.line_segment(
                    [Pos2::new(gx, rect.min.y), Pos2::new(gx, rect.max.y)],
                    grid_stroke,
                );
                gx += step;
            }
            let mut gy = rect.min.y + (view.origin.y - rect.min.y).rem_euclid(step);
            while gy <= rect.max.y {
                painter.line_segment(
                    [Pos2::new(rect.min.x, gy), Pos2::new(rect.max.x, gy)],
                    grid_stroke,
                );
                gy += step;
            }
        }

        let document = self.host.document();
        for connector in &document.connectors {
            let (Some(from), Some(to)) = (
                document.find_shape(connector.from_shape),
                document.find_shape(connector.to_shape),
            ) else {
                continue;
            };
            let start = anchor(from, to, view);
            let end = anchor(to, from, view);
            painter.line_segment([start, end], Stroke::new(2.0, theme::MUTED));

            let direction = end - start;
            if direction.length() > 1.0 {
                let unit = direction.normalized();
                let perpendicular = Vec2::new(-unit.y, unit.x);
                let back = end - unit * 10.0;
                painter.add(egui::epaint::Shape::convex_polygon(
                    vec![end, back + perpendicular * 5.0, back - perpendicular * 5.0],
                    theme::MUTED,
                    Stroke::NONE,
                ));
            }
        }

        for shape in &document.shapes {
            paint_shape(
                painter,
                shape,
                view,
                self.selected == Some(shape.id),
                self.connect_from == Some(shape.id),
            );
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
        self.unsaved_window(ui.ctx());
        self.shell.show_windows(ui.ctx());
        self.shell
            .update_title(ui.ctx(), &self.host.display_name(), self.host.is_dirty());
    }
}

/// Screen point where a link anchored on `shape` points at `other`.
fn anchor(shape: &Shape, other: &Shape, view: View) -> Pos2 {
    let target = document::center(other);
    let point = document::edge_point(shape, target.0, target.1);
    view.to_screen(point.0, point.1)
}

/// Draw a single shape and its centred label.
fn paint_shape(
    painter: &egui::Painter,
    shape: &Shape,
    view: View,
    selected: bool,
    connecting: bool,
) {
    let top_left = view.to_screen(shape.x, shape.y);
    let bottom_right = view.to_screen(shape.x + shape.w, shape.y + shape.h);
    let rect = egui::Rect::from_min_max(top_left, bottom_right);
    let (r, g, b) = document::rgb(shape.color);
    let color = Color32::from_rgb(r, g, b);
    let highlight = selected || connecting;
    let stroke = Stroke::new(
        if highlight { 2.5 } else { 1.5 },
        if highlight {
            theme::ACCENT_LIGHT
        } else {
            color
        },
    );
    let fill = Color32::from_rgba_unmultiplied(r, g, b, 45);

    match shape.kind {
        ShapeKind::Rectangle | ShapeKind::Note => {
            painter.rect(
                rect,
                egui::CornerRadius::same(6),
                fill,
                stroke,
                StrokeKind::Middle,
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
