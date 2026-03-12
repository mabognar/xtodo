use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    text::{Line, Span},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs, io};
use std::path::PathBuf;
use ratatui::style::{Stylize};
use home;
use home::home_dir;
use ratatui::widgets::{BorderType, Clear};
use arboard;
use crossterm::event::KeyEventKind;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
struct TodoItem {
    task: String,
    complete: bool,
    important: bool,
    delete: bool,
}

enum InputMode { Normal, Edit, Move }
#[derive(Serialize, Deserialize, Clone)]
enum Theme { Default, Light }

struct TaskList {
    title: String,
    path: String,
    items: Vec<TodoItem>,
    state: ListState,
}

impl TaskList {

    fn new(title: &str, path: &str) -> Self {
        let items: Vec<TodoItem> = fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        let mut state = ListState::default();
        if !items.is_empty() { state.select(Some(0)); }
        Self { title: title.to_string(), path: path.to_string(), items, state }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.items) {
            let _ = fs::write(&self.path, json);
        }
    }

    // fn save_theme(app: App) {
    //     if let Ok(json) = serde_json::to_string_pretty(&app.theme) {
    //         let _ = fs::write(Path::new(home_dir().unwrap().as_path()).join(".xtodo/theme.json"), json);
    //     }
    // }

    fn scroll(&mut self, delta: i32) {
        if self.items.is_empty() { return; }
        let i = self.state.selected().unwrap_or(0) as i32;
        let new_idx = (i + delta).clamp(0, self.items.len() as i32 - 1) as usize;
        self.state.select(Some(new_idx));
    }

    fn move_item(&mut self, delta: i32) {
        if let Some(i) = self.state.selected() {
            let target = i as i32 + delta;
            if target >= 0 && target < self.items.len() as i32 {
                self.items.swap(i, target as usize);
                self.state.select(Some(target as usize));
                self.save();
            }
        }
    }

    fn toggle_selected<F>(&mut self, f: F) where F: FnOnce(&mut TodoItem) {
        if let Some(i) = self.state.selected() {
            if let Some(item) = self.items.get_mut(i) {
                f(item);
                self.save();
            }
        }
    }
}

struct App {
    input: String,
    input_mode: InputMode,
    lists: [TaskList; 2],
    active_idx: usize,
    cursor_pos: usize,
    edit_task: bool,
    show_help: bool,
    theme: Theme,
}

impl App {
    fn active_list(&mut self) -> &mut TaskList { &mut self.lists[self.active_idx] }
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    // Setup file path and directory. Move old JSON files to .xtodo directory, if needed.
    let path = Path::new(home_dir().unwrap().as_path()).join(".xtodo");
    if !path.exists() { // Optional: check if directory already exists
        fs::create_dir(path)?;
        if home_dir().unwrap().join(".xtodo-list1.json").exists() {
            fs::rename(home_dir().unwrap().as_path().join(".xtodo-list1.json"),
                       home_dir().unwrap().as_path().join(".xtodo/xtodo-list1.json"))?;
        }
        if home_dir().unwrap().join(".xtodo-list2.json").exists() {
            fs::rename(home_dir().unwrap().as_path().join(".xtodo-list2.json"),
                       home_dir().unwrap().as_path().join(".xtodo/xtodo-list2.json"))?;
        }
    }
    let file_path1: PathBuf = home_dir().unwrap().join(".xtodo/xtodo-list1.json");
    let file_path2: PathBuf = home_dir().unwrap().join(".xtodo/xtodo-list2.json");

    let mut app = App {
        input: String::new(),
        input_mode: InputMode::Normal,
        lists: [
            TaskList::new(" List 1 ", file_path1.to_str().unwrap()),
            TaskList::new(" List 2 ", file_path2.to_str().unwrap()),
        ],
        active_idx: 0,
        cursor_pos: 0,
        edit_task: false,
        show_help: false,
        theme: Theme::Default,
    };

    // Read theme.json file and set theme
    let path = Path::new(home_dir().unwrap().as_path()).join(".xtodo");
    if !path.exists() {} // Optional: check if directory already exists
    if home_dir().unwrap().join(".xtodo/theme.json").exists() {
        let file_content = fs::read_to_string(home_dir().unwrap().as_path().join(".xtodo/theme.json"))?;
        let deserialized_event: Theme = serde_json::from_str(&file_content)
            .expect("Failed to deserialize from JSON string");
        app.theme = deserialized_event;
    }
    else {
        app.theme = Theme::Default;
    }



    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => {
                            let json_string = serde_json::to_string(&app.theme)
                                .expect("Failed to serialize to JSON string");
                            fs::write(Path::new(home_dir().unwrap().as_path()).join(".xtodo/theme.json"), json_string)?;

                            // TaskList::save_theme(app);
                            break;
                        },
                        KeyCode::Char('?') => app.show_help = !app.show_help,
                        KeyCode::Char('a') => {
                            app.input_mode = InputMode::Edit;
                            app.edit_task = false;
                        }
                        KeyCode::Char('e') => {
                            app.input_mode = InputMode::Edit;
                            if let Some(selected) = app.active_list().state.selected() {
                                let selected_todo = app.active_list().items[selected].clone();
                                app.input.insert_str(0, selected_todo.task.to_string().as_str());
                                app.cursor_pos = app.input.len();
                            }
                            if app.active_list().items.len() > 0 { app.edit_task = true; } else { app.edit_task = false; }
                        }
                        KeyCode::Char('m') => app.input_mode = InputMode::Move,
                        KeyCode::Tab | KeyCode::Char('s') | KeyCode::Right | KeyCode::Left =>
                            app.active_idx = 1 - app.active_idx,
                        KeyCode::Up | KeyCode::Char('p') => app.active_list().scroll(-1),
                        KeyCode::Down | KeyCode::Char('n') => app.active_list().scroll(1),
                        KeyCode::Char('i') | KeyCode::Char('*') =>
                            app.active_list().toggle_selected(|i| i.important = !i.important),
                        KeyCode::Char('d') => app.active_list().toggle_selected(|i| i.delete = !i.delete),
                        KeyCode::Char('x') => {
                            app.active_list().items.retain(|i| !i.delete);
                            app.active_list().save();
                        }
                        KeyCode::Char('c') => {
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                                if let Some(selected) = app.active_list().state.selected() {
                                    let selected_text = app.active_list().items[selected].task.clone();
                                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                        let _ = clipboard.set_text(selected_text);
                                    }
                                }
                            } else {
                                app.active_list().toggle_selected(|i| i.complete = !i.complete)
                            }
                        }
                        KeyCode::Char('t') => {
                            match app.theme {
                                Theme::Default => {
                                    app.theme = Theme::Light;
                                }
                                Theme::Light => {
                                    app.theme = Theme::Default;
                                }
                            }
                        },
                        _ => {}
                    },
                    InputMode::Edit => match key.code {
                        KeyCode::Enter => {
                            let task = app.input.drain(..).collect();
                            if !String::from(&task).is_empty() {
                                if !app.edit_task {
                                    app.active_list().items.push(
                                        TodoItem { task, complete: false, important: false, delete: false });
                                    app.active_list().state.select_last();
                                } else {
                                    let selected: Option<usize>;
                                    selected = app.active_list().state.selected();
                                    app.cursor_pos = app.input.len();

                                    let task_c = app.active_list().items.get(selected.unwrap()).unwrap().complete;
                                    let task_d = app.active_list().items.get(selected.unwrap()).unwrap().delete;
                                    let task_i = app.active_list().items.get(selected.unwrap()).unwrap().important;

                                    app.active_list().items.remove(selected.unwrap());
                                    app.active_list().items.insert(
                                        selected.unwrap(),
                                        TodoItem {
                                            task: task.to_string(),
                                            complete: task_c,
                                            important: task_i,
                                            delete: task_d
                                        });
                                }
                            }
                            app.active_list().save();
                            app.input_mode = InputMode::Normal;
                            app.cursor_pos = 0;
                        }
                        KeyCode::Char(c) => {
                            app.input.insert(app.cursor_pos, c);
                            app.cursor_pos += 1;
                            // app.input.push(c)
                        },
                        KeyCode::Backspace => {
                            if app.cursor_pos > 0 {
                                // Remove character behind the cursor
                                app.input.remove(app.cursor_pos - 1);
                                app.cursor_pos -= 1;
                            } else if app.input.is_empty() {
                                app.input_mode = InputMode::Normal;
                            }
                            // if app.input.pop().is_none() { app.input_mode = InputMode::Normal }
                        }
                        KeyCode::Left => {
                            if app.cursor_pos > 0 {
                                app.cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if app.cursor_pos < app.input.len() {
                                app.cursor_pos += 1;
                            }
                        }
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                            app.cursor_pos = 0;
                            // app.input_mode = InputMode::Normal
                        },
                        _ => {}
                    },
                    InputMode::Move => match key.code {
                        KeyCode::Up | KeyCode::Char('p') => app.active_list().move_item(-1),
                        KeyCode::Down | KeyCode::Char('n') => app.active_list().move_item(1),
                        KeyCode::Esc | KeyCode::Enter => app.input_mode = InputMode::Normal,
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}


fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(
            if matches!(app.input_mode, InputMode::Edit) { 3 }
            else { 0 }),
        Constraint::Length(2)
    ]).split(f.area());

    let panels = Layout::horizontal([Constraint::Percentage(50),
        Constraint::Percentage(50)]).split(chunks[0]);

    let mut c_list1_border = Color::Rgb(250, 160, 160);
    let mut c_list2_border = Color::Rgb(255, 250, 160);
    let mut c_edit_border = Color::Rgb(250, 200, 152);
    let c_inactive_border = Color::Rgb(120, 120, 120);
    let mut c_list1_highlight = Color::Rgb(83, 53, 53);
    let mut c_list2_highlight = Color::Rgb(85, 82, 53);
    let mut c_list_highlight_move = Color::Rgb(150, 100, 76);
    let mut c_list_highlight_inactive = Color::Rgb(30, 30, 30);
    let mut c_list_delete = Color::Rgb(100,100,255);
    let mut c_menu_hotkey = Color::Rgb(255, 50, 50);
    let mut c_complete = Color::Rgb(50, 255, 50);
    let mut c_delete = Color::Rgb(255,0,255);
    let mut c_important = Color::Rgb(255, 50, 50);
    let mut c_pipe = Color::Rgb(50, 50, 50);
    let mut c_bg = Color::Rgb(0,0,0);
    let mut c_fg = Color::Rgb(230,230,230);
    match app.theme {
        Theme::Default => {
        }
        Theme::Light => {
            c_list1_border = Color::Rgb(50, 50, 200);
            c_list2_border = Color::Rgb(50, 200, 50);
            c_list1_highlight = Color::Rgb(100, 100, 200);
            c_list2_highlight = Color::Rgb(100, 200, 100);
            c_list_highlight_move = Color::Rgb(200, 150, 100);
            c_list_highlight_inactive = Color::Rgb(200, 200, 200);
            c_list_delete = Color::Rgb(200,50,200);
            c_bg = Color::Rgb(220,220,220);
            c_fg = Color::Rgb(0,0,0);
            c_edit_border = Color::Rgb(125, 100, 76);
            c_complete = Color::Rgb(50, 200, 50);
            c_delete = Color::Rgb(200,0,200);
            c_important = Color::Rgb(255, 50, 50);
            c_pipe = Color::Rgb(50, 50, 50);
            c_menu_hotkey = Color::Rgb(200, 50, 50);
        }
    }

    match app.input_mode  {
        InputMode::Edit => {
            f.set_cursor_position(
                // Put cursor at: margin + current cursor position
                (chunks[1].x + app.cursor_pos as u16 + 1,
                 chunks[1].y + 1)
            );
        }
        _ => {}
    }

    for (i, list) in app.lists.iter_mut().enumerate() {
        let is_active = i == app.active_idx;

        let c_border = match (is_active, &app.active_idx) {
            (true, 0) => { c_list1_border },
            (true, 1) => { c_list2_border},
            (false, _) => { c_inactive_border },
            _ => { c_inactive_border },
        };

        let c_title = match (is_active, &app.active_idx) {
            (true, 0) => { c_list1_border },
            (true, 1) => { c_list2_border},
            (false, 0) => { c_list2_border },
            (false, 1) => { c_list1_border },
            _ => { c_inactive_border },
        };

        let highlight_bg = match (is_active, &app.input_mode, &app.active_idx) {
            (true, InputMode::Edit, 0) => { c_list1_highlight },
            (true, InputMode::Edit, 1) => { c_list2_highlight },
            (true, InputMode::Normal, 0) => { c_list1_highlight },
            (true, InputMode::Normal, 1) => { c_list2_highlight },
            (true, InputMode::Move, _) => { c_list_highlight_move },
            (false, _, _) => { c_list_highlight_inactive },
            _ => { c_list_highlight_inactive },
        };

        // 1. Calculate the available text width dynamically
        let inner_width = panels[i].width.saturating_sub(3) as usize;
        let prefix_width = 6; // "[CD] * " takes 7 characters
        let wrap_width = inner_width.saturating_sub(prefix_width).max(5);

        // 2. Renamed closure arg to `item` to avoid shadowing outer `i`
        let items: Vec<ListItem> = list.items.iter().map(|item| {
            let sym_c = match (item.complete, item.delete) {
                (true, false) => { "C" },
                (true, true) => { "" },
                (false, true) => { "" },
                (false, false) => { " " },
            };
            let sym_d = match (item.complete, item.delete) {
                (true, false) => { "" },
                (true, true) => { "D" },
                (false, true) => { "D" },
                (false, false) => { "" },
            };
            let sym_i = match item.important {
                true => { "*" },
                false => { " " },
            };

            // 2a. Word wrapping logic
            let mut task_lines = Vec::new();
            let mut current_line = String::new();
            for word in item.task.split_whitespace() {
                if current_line.is_empty() {
                    current_line.push_str(word);
                } else if current_line.chars().count() + 1 + word.chars().count() <= wrap_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    task_lines.push(current_line);
                    current_line = String::from(word);
                }
            }
            if !current_line.is_empty() || task_lines.is_empty() {
                task_lines.push(current_line);
            }
            let sym_t = match item.delete {
                true => { Span::styled(task_lines[0].to_string(),
                                        Style::default().fg(c_list_delete))
                },
                false => { Span::styled(task_lines[0].clone().to_string(),
                                        Style::default()) },
            };

            // 2b. Constructing the multiline ListItem
            let mut lines = vec![Line::from(vec![
                Span::raw("["),
                Span::styled(sym_c, c_complete).bold(),
                Span::styled(sym_d, c_delete).bold(), Span::raw("] "),
                Span::styled(sym_i, c_important).bold(), Span::raw(" "),
                sym_t
            ])];

            // 2c. Add overflow lines with correct padding to match "[CD] * "
            for task_line in task_lines.into_iter().skip(1) {
                let sym_t = match item.delete {
                    true => { Span::styled(task_line.to_string(),
                                           Style::default().fg(c_list_delete))
                    },
                    false => { Span::styled(task_line.to_string(),
                                            Style::default())
                    },
                };
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    sym_t
                ]));
            }

            ListItem::new(lines)

        }).collect();

        let nrows = list.items.len();
        let mut srow= list.state.selected().unwrap_or(0);
        if nrows == 0 {srow = 0}
        if Some(srow) < Some(nrows) {srow = srow + 1}
        if Some(srow) > Some(nrows) {srow = nrows}

        let widget = List::new(items)
            .block(Block::default()
                .title(Line::from(vec![
                    Span::from(list.title.as_str()).style(c_title),
                    Span::from(format!(" [{}/{}] ",srow,nrows))
                ]))
                .title_style(Style::default().bold())
                .borders(Borders::ALL)
                .border_style(c_border)
                .border_type(BorderType::Rounded))
            .bg(c_bg)
            .fg(c_fg)
            .highlight_style(Style::default().bg(highlight_bg));

        f.render_stateful_widget(widget, panels[i], &mut list.state);
    }

    let input_block = Block::default()
        .title(
            if !app.edit_task {" Add: Type task followed by Enter "}
            else {" Edit: Revise task followed by Enter "})
        .title_style(Style::default().bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if matches!(app.input_mode, InputMode::Edit) {
            Style::default().fg(c_edit_border)
        } else {
            Style::default()
        })
        .bg(c_bg)
        .fg(c_fg);

    f.render_widget(Paragraph::new(app.input.as_str()).block(input_block), chunks[1]);

    let hotkey_style = Style::default().fg(c_menu_hotkey);
    let text_style = Style::default().fg(c_fg);
    let pipe_style = Style::default().fg(c_pipe);
    let help_msg1 = Line::from(vec![
        Span::styled(" a", hotkey_style), Span::styled("dd", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("e", hotkey_style), Span::styled("dit", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("p", hotkey_style), Span::styled("revious", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("n", hotkey_style), Span::styled("ext", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("c", hotkey_style), Span::styled("ompleted", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("i", hotkey_style), Span::styled("mportant", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("d", hotkey_style), Span::styled("elete", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("e", text_style), Span::styled("x", hotkey_style),
        Span::styled("punge", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("? ", hotkey_style),
    ]).left_aligned();
    let help_msg2 = Line::from(vec![
        Span::styled(" m", hotkey_style), Span::styled("ove", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("s", hotkey_style), Span::styled("witch", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("copy: ", text_style), Span::styled("ctrl-c ", hotkey_style),
        Span::styled(" | ", pipe_style),
        Span::styled("remove: ", text_style),
        Span::styled("d", hotkey_style), Span::styled("elete then ", text_style),
        Span::styled("e", text_style), Span::styled("x", hotkey_style),
        Span::styled("punge", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("t", hotkey_style), Span::styled("heme", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("q", hotkey_style), Span::styled("uit ", text_style),
    ]).left_aligned();

    f.render_widget(Paragraph::new(vec![help_msg1, help_msg2])
                        .bg(c_bg).fg(c_fg),
                    chunks[2]);

    if app.show_help {
        let area = centered_rect(f.area());
        let help_text = vec![
            Line::from(Span::styled(" https://github.com/mabognar ", c_fg)),
            Line::from(vec![Span::styled(" https://crates.io/crates/xtodo ", c_fg)]),
        ];

        const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
        let block = Block::default()
            .title(Line::from(vec![Span::raw(" xtodo "),
                                   Span::raw(format!("({}) ",PKG_VERSION))]))
            .title_bottom(Line::from(vec![Span::raw(" To close, type "),
                                          Span::styled("? ", c_menu_hotkey)]))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(c_edit_border))
            .bg(c_bg).fg(c_menu_hotkey);

        let help_para = Paragraph::new(help_text)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true });

        f.render_widget(Clear, area); // This clears the area under the popup
        f.render_widget(help_para, area);
    }

    fn centered_rect(r: ratatui::layout::Rect) -> ratatui::layout::Rect {
        let popup_layout = Layout::vertical([
            Constraint::Fill(1), Constraint::Length(4), Constraint::Fill(1),
        ]).split(r);

        Layout::horizontal([
            Constraint::Fill(1), Constraint::Length(33), Constraint::Fill(1),
        ]).split(popup_layout[1])[1]
    }

}


