//! `cadvm ui` — a full-screen interactive terminal dashboard (ratatui).
//!
//! Browse the commit history like a source-control panel, inspect each commit's
//! files and metadata, and launch metadata/geometric diffs and the 3D viewer —
//! all without leaving the terminal. The TUI calls `cadvm-core` directly (it is
//! the same binary), so geometry actions need the `cadvm-geom` helper.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::{DefaultTerminal, Frame};

use cadvm_core::checkout;
use cadvm_core::diff;
use cadvm_core::geom;
use cadvm_core::model::{Commit, FileEntry};
use cadvm_core::status::working_tree_status;
use cadvm_core::{ObjectId, Repository};

use crate::viewer;

// ---- theme (tokyo-night-ish) ----------------------------------------------
const ACCENT: Color = Color::Rgb(122, 162, 247);
const FG: Color = Color::Rgb(192, 202, 245);
const DIM: Color = Color::Rgb(110, 115, 135);
const GREEN: Color = Color::Rgb(158, 206, 106);
const RED: Color = Color::Rgb(247, 118, 142);
const YELLOW: Color = Color::Rgb(224, 175, 104);
const PANEL: Color = Color::Rgb(36, 40, 59);

/// One row in the commit list.
struct Row {
    commit: Commit,
    tips: Vec<String>,
    is_head: bool,
}

/// A modal overlay.
enum Modal {
    None,
    Help,
    /// Scrollable text (title, lines, scroll offset).
    Text(String, Vec<Line<'static>>, u16),
    /// Branch switcher.
    Branches(ListState),
}

struct App {
    repo: Repository,
    rows: Vec<Row>,
    branches: Vec<String>,
    current_branch: Option<String>,
    dirty: bool,
    list: ListState,
    anchor: Option<usize>,
    modal: Modal,
    toast: Option<(String, Instant, bool)>, // message, since, is_error
    now: i64,
    quit: bool,
}

/// Entry point for `cadvm ui`.
pub fn run(repo: Repository) -> Result<()> {
    let mut app = App::new(repo)?;
    let mut terminal = ratatui::init();
    let res = app.event_loop(&mut terminal);
    ratatui::restore();
    res
}

impl App {
    fn new(repo: Repository) -> Result<App> {
        let mut app = App {
            repo,
            rows: Vec::new(),
            branches: Vec::new(),
            current_branch: None,
            dirty: false,
            list: ListState::default(),
            anchor: None,
            modal: Modal::None,
            toast: None,
            now: Utc::now().timestamp(),
            quit: false,
        };
        app.reload()?;
        Ok(app)
    }

    /// (Re)load all repository state.
    fn reload(&mut self) -> Result<()> {
        self.now = Utc::now().timestamp();
        self.branches = self.repo.list_branches().unwrap_or_default();
        self.current_branch = self.repo.current_branch().unwrap_or(None);
        self.dirty = working_tree_status(&self.repo)
            .map(|s| !s.is_clean())
            .unwrap_or(false);

        // Gather every commit reachable from any branch tip and from HEAD.
        let head_id = self.repo.head_commit_id().unwrap_or(None);
        let mut tips_by_commit: HashMap<ObjectId, Vec<String>> = HashMap::new();
        let mut frontier: Vec<ObjectId> = Vec::new();
        for b in &self.branches {
            if let Ok(Some(id)) = self.repo.read_ref(b) {
                tips_by_commit
                    .entry(id.clone())
                    .or_default()
                    .push(b.clone());
                frontier.push(id);
            }
        }

        let mut seen: HashMap<ObjectId, Commit> = HashMap::new();
        while let Some(id) = frontier.pop() {
            if seen.contains_key(&id) {
                continue;
            }
            if let Ok(commit) = self.repo.read_commit(&id) {
                for p in &commit.parents {
                    frontier.push(p.clone());
                }
                seen.insert(id, commit);
            }
        }

        let mut rows: Vec<Row> = seen
            .into_iter()
            .map(|(id, commit)| Row {
                tips: tips_by_commit.get(&id).cloned().unwrap_or_default(),
                is_head: head_id.as_ref() == Some(&commit.id),
                commit,
            })
            .collect();
        // Newest first.
        rows.sort_by(|a, b| {
            b.commit
                .timestamp_unix
                .cmp(&a.commit.timestamp_unix)
                .then_with(|| b.commit.id.hex().cmp(a.commit.id.hex()))
        });
        self.rows = rows;

        if self.rows.is_empty() {
            self.list.select(None);
        } else {
            let sel = self.list.selected().unwrap_or(0).min(self.rows.len() - 1);
            self.list.select(Some(sel));
        }
        if let Some(a) = self.anchor {
            if a >= self.rows.len() {
                self.anchor = None;
            }
        }
        Ok(())
    }

    fn selected(&self) -> Option<&Row> {
        self.list.selected().and_then(|i| self.rows.get(i))
    }

    // ---- event loop -------------------------------------------------------

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.quit {
            terminal.draw(|f| self.draw(f))?;
            // Poll so toasts can expire even without input.
            if !event::poll(Duration::from_millis(500))? {
                self.expire_toast();
                continue;
            }
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                self.on_key(key.code, terminal)?;
            }
        }
        Ok(())
    }

    fn expire_toast(&mut self) {
        if let Some((_, since, _)) = &self.toast {
            if since.elapsed() > Duration::from_secs(4) {
                self.toast = None;
            }
        }
    }

    fn toast(&mut self, msg: impl Into<String>, error: bool) {
        self.toast = Some((msg.into(), Instant::now(), error));
    }

    fn on_key(&mut self, code: KeyCode, terminal: &mut DefaultTerminal) -> Result<()> {
        // Modal-specific handling first.
        match &mut self.modal {
            Modal::Help => {
                if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')) {
                    self.modal = Modal::None;
                }
                return Ok(());
            }
            Modal::Text(_, lines, scroll) => {
                let max = lines.len() as u16;
                match code {
                    KeyCode::Esc | KeyCode::Char('q') => self.modal = Modal::None,
                    KeyCode::Down | KeyCode::Char('j') => *scroll = (*scroll + 1).min(max),
                    KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                    _ => {}
                }
                return Ok(());
            }
            Modal::Branches(state) => {
                match code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('b') => {
                        self.modal = Modal::None
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = state.selected().unwrap_or(0);
                        state.select(Some((i + 1).min(self.branches.len().saturating_sub(1))));
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = state.selected().unwrap_or(0);
                        state.select(Some(i.saturating_sub(1)));
                    }
                    KeyCode::Enter => {
                        let pick = state.selected().and_then(|i| self.branches.get(i)).cloned();
                        self.modal = Modal::None;
                        if let Some(name) = pick {
                            self.do_switch(&name);
                        }
                    }
                    _ => {}
                }
                return Ok(());
            }
            Modal::None => {}
        }

        // Main view keys.
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::Char('m') => self.toggle_anchor(),
            KeyCode::Char('d') => self.action_meta_diff(),
            KeyCode::Char('G') | KeyCode::Char('g') => self.action_geom_diff(terminal),
            KeyCode::Char('v') => self.action_view(terminal),
            KeyCode::Char('s') => self.action_status(),
            KeyCode::Char('b') => self.open_branches(),
            KeyCode::Char('r') => {
                self.reload()?;
                self.toast("Reloaded", false);
            }
            KeyCode::Char('?') => self.modal = Modal::Help,
            _ => {}
        }
        Ok(())
    }

    fn move_sel(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let i = self.list.selected().unwrap_or(0) as i32 + delta;
        let i = i.clamp(0, self.rows.len() as i32 - 1) as usize;
        self.list.select(Some(i));
    }

    fn toggle_anchor(&mut self) {
        let sel = self.list.selected();
        if self.anchor == sel {
            self.anchor = None;
            self.toast("Anchor cleared", false);
        } else {
            self.anchor = sel;
            self.toast(
                "Anchor set — diff/geom/view now compare anchor → selected",
                false,
            );
        }
    }

    /// Resolve (base, target) commit ids for a diff action.
    fn diff_pair(&self) -> Option<(ObjectId, ObjectId)> {
        let target = self.selected()?.commit.id.clone();
        let base = match self.anchor.and_then(|i| self.rows.get(i)) {
            Some(a) => a.commit.id.clone(),
            None => self.selected()?.commit.parents.first().cloned()?,
        };
        Some((base, target))
    }

    fn action_meta_diff(&mut self) {
        let Some((base, target)) = self.diff_pair() else {
            self.toast("Nothing to compare (root commit, no anchor)", true);
            return;
        };
        let (Ok(ma), Ok(mb)) = (
            self.repo.manifest_of_commit(&base),
            self.repo.manifest_of_commit(&target),
        ) else {
            self.toast("Could not load manifests", true);
            return;
        };
        let d = diff::diff_manifests(&ma, &mb);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(short(&base), Style::new().fg(YELLOW)),
            Span::raw(" → "),
            Span::styled(short(&target), Style::new().fg(YELLOW)),
        ]));
        lines.push(Line::raw(""));
        if d.is_empty() {
            lines.push(Line::styled("No changes.", Style::new().fg(DIM)));
        }
        for p in &d.added {
            lines.push(Line::from(vec![
                Span::styled("  + ", Style::new().fg(GREEN)),
                Span::raw(p.display().to_string()),
            ]));
        }
        for p in &d.removed {
            lines.push(Line::from(vec![
                Span::styled("  - ", Style::new().fg(RED)),
                Span::raw(p.display().to_string()),
            ]));
        }
        for f in &d.modified {
            lines.push(Line::styled(
                format!("  ~ {}", f.path.display()),
                Style::new().fg(YELLOW),
            ));
            lines.push(Line::styled(
                format!(
                    "      size {} → {} · lines {} → {} · entities {} → {}",
                    f.size_bytes.0,
                    f.size_bytes.1,
                    optnum(f.line_count.0),
                    optnum(f.line_count.1),
                    optnum(f.entity_count.0),
                    optnum(f.entity_count.1),
                ),
                Style::new().fg(DIM),
            ));
        }
        self.modal = Modal::Text("Metadata diff".into(), lines, 0);
    }

    fn action_geom_diff(&mut self, terminal: &mut DefaultTerminal) {
        let Some((base, target)) = self.diff_pair() else {
            self.toast("Nothing to compare", true);
            return;
        };
        let _ = self.busy_frame(terminal, "Computing geometry…");
        match self.run_geom(&base, &target) {
            Ok(lines) => self.modal = Modal::Text("Geometric diff".into(), lines, 0),
            Err(e) => self.toast(format!("{e}"), true),
        }
    }

    fn run_geom(&self, base: &ObjectId, target: &ObjectId) -> Result<Vec<Line<'static>>> {
        let (file, ea, eb) = self.pick_modified(base, target)?;
        let tmp = self.repo.tmp_dir();
        let pa = self.extract(&tmp, &ea, "ui-a")?;
        let pb = self.extract(&tmp, &eb, "ui-b")?;
        let result = geom::diff_files(&pa, &pb);
        let _ = std::fs::remove_file(&pa);
        let _ = std::fs::remove_file(&pb);
        let g = result?;
        if !g.is_ok() {
            anyhow::bail!("geometry error: {}", g.error.unwrap_or_default());
        }
        let a = g.a.context("missing metrics")?;
        let b = g.b.context("missing metrics")?;
        let mut lines = vec![
            Line::styled(file.display().to_string(), Style::new().fg(ACCENT).bold()),
            Line::raw(""),
            kv("volume", &format!("{:.3} → {:.3}", a.volume, b.volume)),
            kv("area", &format!("{:.3} → {:.3}", a.area, b.area)),
            kv("faces", &format!("{} → {}", a.faces, b.faces)),
        ];
        if let Some(c) = &g.common {
            lines.push(line_color("common", &format!("vol {:.3}", c.volume), DIM));
        }
        if let Some(ad) = &g.added {
            lines.push(line_color("added", &format!("vol {:.3}", ad.volume), GREEN));
        }
        if let Some(rm) = &g.removed {
            lines.push(line_color("removed", &format!("vol {:.3}", rm.volume), RED));
        }
        if let Some(ft) = &g.faces_topo {
            lines.push(Line::raw(""));
            lines.push(kv(
                "faces (topo)",
                &format!(
                    "{} common · {} added · {} removed",
                    ft.common, ft.added, ft.removed
                ),
            ));
        }
        Ok(lines)
    }

    fn action_view(&mut self, terminal: &mut DefaultTerminal) {
        let Some((base, target)) = self.diff_pair() else {
            self.toast("Nothing to compare", true);
            return;
        };
        let _ = self.busy_frame(terminal, "Meshing & building viewer…");
        match self.build_viewer(&base, &target) {
            Ok(path) => {
                open_in_browser(&path);
                self.toast(format!("Wrote & opened {}", path.display()), false);
            }
            Err(e) => self.toast(format!("{e}"), true),
        }
    }

    fn build_viewer(&self, base: &ObjectId, target: &ObjectId) -> Result<PathBuf> {
        let (file, ea, eb) = self.pick_modified(base, target)?;
        let tmp = self.repo.tmp_dir();
        let pa = self.extract(&tmp, &ea, "uiv-a")?;
        let pb = self.extract(&tmp, &eb, "uiv-b")?;
        let out_json = tmp.join("ui-mesh.json");
        let mesh = geom::mesh_files(&pa, &pb, &out_json);
        let _ = std::fs::remove_file(&pa);
        let _ = std::fs::remove_file(&pb);
        let mesh = mesh?;
        if !mesh.is_ok() {
            let _ = std::fs::remove_file(&out_json);
            anyhow::bail!("geometry error: {}", mesh.error.unwrap_or_default());
        }
        let json = std::fs::read_to_string(&out_json)?;
        let _ = std::fs::remove_file(&out_json);
        let title = format!("{}  ({}..{})", file.display(), short(base), short(target));
        let html = viewer::render(&title, &json);
        let out = PathBuf::from("cadvm-view.html");
        std::fs::write(&out, html)?;
        Ok(out)
    }

    /// First modified STEP file between two commits, with both versions' entries.
    fn pick_modified(
        &self,
        base: &ObjectId,
        target: &ObjectId,
    ) -> Result<(PathBuf, FileEntry, FileEntry)> {
        let ma = self.repo.manifest_of_commit(base)?;
        let mb = self.repo.manifest_of_commit(target)?;
        let d = diff::diff_manifests(&ma, &mb);
        let path = d
            .modified
            .first()
            .map(|f| f.path.clone())
            .context("no modified STEP file between these commits")?;
        let ea = ma.files.get(&path).cloned().context("missing entry")?;
        let eb = mb.files.get(&path).cloned().context("missing entry")?;
        Ok((path, ea, eb))
    }

    fn extract(&self, tmp: &std::path::Path, entry: &FileEntry, tag: &str) -> Result<PathBuf> {
        let content = self.repo.store().read_file_content(&entry.blob_ref)?;
        let dest = tmp.join(format!("{tag}.{}", entry.format.extension()));
        std::fs::write(&dest, content)?;
        Ok(dest)
    }

    fn action_status(&mut self) {
        let Ok(st) = working_tree_status(&self.repo) else {
            self.toast("Could not read status", true);
            return;
        };
        let mut lines = vec![Line::styled(
            format!("On branch {}", st.branch.as_deref().unwrap_or("(detached)")),
            Style::new().fg(ACCENT),
        )];
        lines.push(Line::raw(""));
        if st.is_clean() {
            lines.push(Line::styled("Clean working tree.", Style::new().fg(GREEN)));
        }
        for p in &st.new {
            lines.push(line_color("new", &p.display().to_string(), GREEN));
        }
        for p in &st.modified {
            lines.push(line_color("modified", &p.display().to_string(), YELLOW));
        }
        for p in &st.deleted {
            lines.push(line_color("deleted", &p.display().to_string(), RED));
        }
        self.modal = Modal::Text("Working tree status".into(), lines, 0);
    }

    fn open_branches(&mut self) {
        if self.branches.is_empty() {
            self.toast("No branches", true);
            return;
        }
        let mut state = ListState::default();
        let cur = self
            .current_branch
            .as_ref()
            .and_then(|c| self.branches.iter().position(|b| b == c))
            .unwrap_or(0);
        state.select(Some(cur));
        self.modal = Modal::Branches(state);
    }

    fn do_switch(&mut self, name: &str) {
        match checkout::switch(&self.repo, name, false) {
            Ok(_) => {
                let _ = self.reload();
                self.toast(format!("Switched to {name}"), false);
            }
            Err(e) => self.toast(format!("{e}"), true),
        }
    }

    /// Draw a one-off "busy" frame before a blocking action.
    fn busy_frame(&mut self, terminal: &mut DefaultTerminal, msg: &str) -> Result<()> {
        let text = msg.to_string();
        terminal.draw(|f| {
            let area = centered(f.area(), 40, 3);
            f.render_widget(Clear, area);
            let p = Paragraph::new(Line::from(vec![
                Span::styled("⏳ ", Style::new().fg(YELLOW)),
                Span::styled(text, Style::new().fg(FG)),
            ]))
            .alignment(Alignment::Center)
            .block(panel_block("working"));
            f.render_widget(p, area);
        })?;
        Ok(())
    }

    // ---- rendering --------------------------------------------------------

    fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());
        self.draw_header(f, chunks[0]);
        let body = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(chunks[1]);
        self.draw_commits(f, body[0]);
        self.draw_detail(f, body[1]);
        self.draw_footer(f, chunks[2]);

        match &self.modal {
            Modal::Help => self.draw_help(f),
            Modal::Text(title, lines, scroll) => draw_text_modal(f, title, lines, *scroll),
            Modal::Branches(_) => self.draw_branches(f),
            Modal::None => {}
        }
        self.draw_toast(f);
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let branch = self.current_branch.as_deref().unwrap_or("detached");
        let (dot, dot_c) = if self.dirty {
            ("●", YELLOW)
        } else {
            ("●", GREEN)
        };
        let left = Line::from(vec![
            Span::styled(" ◆ cadvm ", Style::new().bg(ACCENT).fg(Color::Black).bold()),
            Span::raw(" "),
            Span::styled(
                self.repo.workdir().display().to_string(),
                Style::new().fg(DIM),
            ),
        ]);
        let right = Line::from(vec![
            Span::styled(format!("⎇ {branch} "), Style::new().fg(ACCENT).bold()),
            Span::styled(dot, Style::new().fg(dot_c)),
            Span::styled(
                if self.dirty { " dirty " } else { " clean " },
                Style::new().fg(dot_c),
            ),
        ])
        .alignment(Alignment::Right);
        f.render_widget(Paragraph::new(left), area);
        f.render_widget(Paragraph::new(right), area);
    }

    fn draw_commits(&mut self, f: &mut Frame, area: Rect) {
        let now = self.now;
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut spans = vec![
                    Span::styled(
                        "● ",
                        Style::new().fg(if r.is_head { GREEN } else { ACCENT }),
                    ),
                    Span::styled(short(&r.commit.id), Style::new().fg(YELLOW)),
                    Span::raw(" "),
                ];
                if self.anchor == Some(i) {
                    spans.push(Span::styled("⚓ ", Style::new().fg(ACCENT)));
                }
                if r.is_head {
                    spans.push(Span::styled(
                        " HEAD ",
                        Style::new().bg(GREEN).fg(Color::Black),
                    ));
                    spans.push(Span::raw(" "));
                }
                for t in &r.tips {
                    spans.push(Span::styled(
                        format!("⎇ {t} "),
                        Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ));
                }
                spans.push(Span::styled(
                    first_line(&r.commit.message),
                    Style::new().fg(FG),
                ));
                let meta = format!(
                    "  · {} · {}",
                    r.commit
                        .author
                        .as_ref()
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| "unknown".into()),
                    rel_time(r.commit.timestamp_unix, now)
                );
                spans.push(Span::styled(meta, Style::new().fg(DIM)));
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(panel_block("commits"))
            .highlight_style(Style::new().bg(PANEL).add_modifier(Modifier::BOLD))
            .highlight_symbol("▌ ");
        f.render_stateful_widget(list, area, &mut self.list);

        if self.rows.is_empty() {
            let p = Paragraph::new(Line::styled(
                "No commits yet — run `cadvm snapshot`.",
                Style::new().fg(DIM),
            ))
            .block(panel_block("commits"));
            f.render_widget(p, area);
        }
    }

    fn draw_detail(&self, f: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        if let Some(r) = self.selected() {
            let c = &r.commit;
            lines.push(Line::from(vec![
                Span::styled("commit ", Style::new().fg(DIM)),
                Span::styled(
                    c.id.hex()[..12.min(c.id.hex().len())].to_string(),
                    Style::new().fg(YELLOW),
                ),
            ]));
            if let Some(a) = &c.author {
                lines.push(kv("author", &a.display()));
            }
            lines.push(kv("date", &fmt_date(c.timestamp_unix)));
            if !c.parents.is_empty() {
                let ps: Vec<String> = c.parents.iter().map(short).collect();
                lines.push(kv("parents", &ps.join(", ")));
            }
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                c.message.clone(),
                Style::new().fg(FG).add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::raw(""));

            if let Ok(m) = self.repo.read_manifest(&c.manifest) {
                lines.push(Line::styled(
                    format!("Files ({})", m.file_count()),
                    Style::new().fg(ACCENT),
                ));
                for entry in m.files.values() {
                    lines.push(Line::from(vec![
                        Span::styled("  ▪ ", Style::new().fg(ACCENT)),
                        Span::styled(entry.path.display().to_string(), Style::new().fg(FG)),
                    ]));
                    let schema = entry
                        .step_metadata
                        .as_ref()
                        .and_then(|s| s.file_schema.clone())
                        .unwrap_or_else(|| "?".into());
                    let entities = entry
                        .step_metadata
                        .as_ref()
                        .and_then(|s| s.entity_count)
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "?".into());
                    lines.push(Line::styled(
                        format!(
                            "      {} B · {} lines · {} · {} entities",
                            entry.size_bytes,
                            optnum(entry.line_count),
                            schema,
                            entities
                        ),
                        Style::new().fg(DIM),
                    ));
                }
            }
        } else {
            lines.push(Line::styled("Select a commit.", Style::new().fg(DIM)));
        }

        let p = Paragraph::new(Text::from(lines))
            .block(panel_block("details"))
            .wrap(Wrap { trim: false });
        f.render_widget(p, area);
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let key = |k: &'static str, d: &'static str| {
            vec![
                Span::styled(k, Style::new().fg(ACCENT).bold()),
                Span::styled(format!(" {d}  "), Style::new().fg(DIM)),
            ]
        };
        let mut spans = Vec::new();
        for (k, d) in [
            ("↑↓", "move"),
            ("m", "anchor"),
            ("d", "diff"),
            ("g", "geom"),
            ("v", "view"),
            ("b", "branch"),
            ("s", "status"),
            ("?", "help"),
            ("q", "quit"),
        ] {
            spans.extend(key(k, d));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_toast(&self, f: &mut Frame) {
        if let Some((msg, _, error)) = &self.toast {
            let area = f.area();
            let w = (msg.len() as u16 + 4).min(area.width.saturating_sub(2));
            let rect = Rect {
                x: area.x + 1,
                y: area.height.saturating_sub(2),
                width: w,
                height: 1,
            };
            f.render_widget(Clear, rect);
            let c = if *error { RED } else { GREEN };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        if *error { " ✗ " } else { " ✓ " },
                        Style::new().bg(c).fg(Color::Black),
                    ),
                    Span::styled(format!(" {msg}"), Style::new().fg(c)),
                ])),
                rect,
            );
        }
    }

    fn draw_help(&self, f: &mut Frame) {
        let lines = vec![
            kv("↑ / k, ↓ / j", "move selection"),
            kv("m", "set/clear anchor (the diff base)"),
            kv("d", "metadata diff (anchor→sel, or parent→sel)"),
            kv("g", "geometric diff (OCCT volumes + faces)"),
            kv("v", "build & open the 3D viewer"),
            kv("b", "switch branch"),
            kv("s", "working tree status"),
            kv("r", "reload"),
            kv("? / Esc", "close · q quit"),
            Line::raw(""),
            Line::styled(
                "Geometry actions need the cadvm-geom helper (CADVM_GEOM_BIN).",
                Style::new().fg(DIM),
            ),
        ];
        draw_text_modal(f, "Help", &lines, 0);
    }

    fn draw_branches(&self, f: &mut Frame) {
        let area = centered(f.area(), 40, (self.branches.len() as u16 + 2).min(14));
        f.render_widget(Clear, area);
        let items: Vec<ListItem> = self
            .branches
            .iter()
            .map(|b| {
                let cur = self.current_branch.as_deref() == Some(b.as_str());
                let mark = if cur { "● " } else { "  " };
                ListItem::new(Line::from(vec![
                    Span::styled(mark, Style::new().fg(GREEN)),
                    Span::styled(b.clone(), Style::new().fg(if cur { GREEN } else { FG })),
                ]))
            })
            .collect();
        let mut state = if let Modal::Branches(s) = &self.modal {
            s.clone()
        } else {
            ListState::default()
        };
        let list = List::new(items)
            .block(panel_block("switch branch  (Enter)"))
            .highlight_style(Style::new().bg(PANEL).bold())
            .highlight_symbol("▌ ");
        f.render_stateful_widget(list, area, &mut state);
    }
}

// ---- small helpers ---------------------------------------------------------

fn panel_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(DIM))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(ACCENT).bold(),
        ))
}

fn draw_text_modal(f: &mut Frame, title: &str, lines: &[Line<'static>], scroll: u16) {
    let area = centered(f.area(), 70, 22);
    f.render_widget(Clear, area);
    let p = Paragraph::new(Text::from(lines.to_vec()))
        .block(panel_block(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(p, area);
}

fn centered(area: Rect, max_w: u16, max_h: u16) -> Rect {
    let w = max_w.min(area.width.saturating_sub(2));
    let h = max_h.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

fn kv(k: &str, v: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{k}: "), Style::new().fg(DIM)),
        Span::styled(v.to_string(), Style::new().fg(FG)),
    ])
}

fn line_color(tag: &str, v: &str, c: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {tag}: "), Style::new().fg(c).bold()),
        Span::styled(v.to_string(), Style::new().fg(FG)),
    ])
}

fn short(id: &ObjectId) -> String {
    id.short().to_string()
}

fn first_line(msg: &str) -> String {
    msg.lines().next().unwrap_or("").to_string()
}

fn optnum(v: Option<u64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
}

fn fmt_date(unix: i64) -> String {
    match Utc.timestamp_opt(unix, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => unix.to_string(),
    }
}

fn rel_time(ts: i64, now: i64) -> String {
    let d = (now - ts).max(0);
    if d < 60 {
        format!("{d}s ago")
    } else if d < 3600 {
        format!("{}m ago", d / 60)
    } else if d < 86400 {
        format!("{}h ago", d / 3600)
    } else {
        format!("{}d ago", d / 86400)
    }
}

/// Best-effort open in the platform browser (mirrors the CLI helper).
fn open_in_browser(path: &std::path::Path) {
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let cmd = "";
    if !cmd.is_empty() {
        let _ = std::process::Command::new(cmd).arg(path).spawn();
    }
}
