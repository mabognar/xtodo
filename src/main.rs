mod todo_colors;

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
use ratatui::widgets::BorderType;
use crate::todo_colors::{DARKGRAY, DARKORANGE, DARKRED, HELP_BACKGROUND, LIGHTGRAY, RED, YELLOW, DARKYELLOW, ORANGE};
use arboard;

#[derive(Serialize, Deserialize, Clone)]
struct TodoItem {
    task: String,
    complete: bool,
    important: bool,
    delete: bool,
}

enum InputMode { Normal, Edit, Move }

struct TaskList {
    title: String,
    title_color: Color,
    path: String,
    items: Vec<TodoItem>,
    state: ListState,
}

impl TaskList {

    fn new(title: &str, title_color: Color, path: &str) -> Self {
        let items: Vec<TodoItem> = fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        let mut state = ListState::default();
        if !items.is_empty() { state.select(Some(0)); }
        Self { title: title.to_string(), title_color, path: path.to_string(), items, state }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.items) {
            let _ = fs::write(&self.path, json);
        }
    }

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
}

impl App {
    fn active_list(&mut self) -> &mut TaskList { &mut self.lists[self.active_idx] }
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    let file_path1: PathBuf = home_dir().unwrap().join(".xtodo-list1.json");
    let file_path2: PathBuf = home_dir().unwrap().join(".xtodo-list2.json");

    let mut app = App {
        input: String::new(),
        input_mode: InputMode::Normal,
        lists: [
            TaskList::new(" List 1 ", RED, file_path1.to_str().unwrap()),
            TaskList::new(" List 2 ", YELLOW, file_path2.to_str().unwrap()),
        ],
        active_idx: 0,
        cursor_pos: 0,
    };

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('a') => app.input_mode = InputMode::Edit,
                    KeyCode::Char('m') => app.input_mode = InputMode::Move,
                    KeyCode::Tab | KeyCode::Char('s') | KeyCode::Right | KeyCode::Left => app.active_idx = 1 - app.active_idx,
                    KeyCode::Up | KeyCode::Char('p') => app.active_list().scroll(-1),
                    KeyCode::Down | KeyCode::Char('n') => app.active_list().scroll(1),
                    KeyCode::Char('i') | KeyCode::Char('*') => app.active_list().toggle_selected(|i| i.important = !i.important),
                    KeyCode::Char('d') => app.active_list().toggle_selected(|i| i.delete = !i.delete),
                    KeyCode::Char('x') => { app.active_list().items.retain(|i| !i.delete); app.active_list().save(); }
                    KeyCode::Char('c') => {
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                            if let Some(selected) = app.active_list().state.selected() {
                                let selected_text = app.active_list().items[selected].task.clone();
                                // Use arboard to copy
                                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                    let _ = clipboard.set_text(selected_text);
                                }
                            }
                        }
                        else {
                            app.active_list().toggle_selected(|i| i.complete = !i.complete)
                        }
                    }
                    _ => {}
                },
                InputMode::Edit => match key.code {
                    KeyCode::Enter => {
                        let task = app.input.drain(..).collect();
                        if !String::from(&task).is_empty() {
                            app.active_list().items.push(TodoItem { task, complete: false, important: false, delete: false });
                            app.active_list().state.select_last();
                        }
                        // if !app.input.is_empty() { app.input_mode = InputMode::Normal }
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

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}


fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(if matches!(app.input_mode, InputMode::Edit) { 3 } else { 0 }),
        Constraint::Length(2)
    ]).split(f.area());

    let panels = Layout::horizontal([Constraint::Percentage(50),
        Constraint::Percentage(50)]).split(chunks[0]);

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

        // Original specific RGB border colors
        let border_color = match (is_active, &app.active_idx) {
            (true, 0) => { RED },
            (true, 1) => { YELLOW },
            (false, _) => { LIGHTGRAY },
            _ => { LIGHTGRAY },
        };

        // Original specific RGB highlight background colors
        let highlight_bg = match (is_active, &app.input_mode, &app.active_idx) {
            (true, InputMode::Edit, 0) => { DARKRED },
            (true, InputMode::Edit, 1) => { DARKYELLOW },
            (true, InputMode::Normal, 0) => { DARKRED },
            (true, InputMode::Normal, 1) => { DARKYELLOW },
            (true, InputMode::Move, 0) => { DARKORANGE },
            (true, InputMode::Move, 1) => { DARKORANGE },
            (false, _, _) => { DARKGRAY },
            _ => { DARKGRAY },
        };

        // 1. Calculate the available text width dynamically
        let inner_width = panels[i].width.saturating_sub(3) as usize; // Account for left/right borders
        let prefix_width = 6; // "[CD] * " takes 7 characters
        let wrap_width = inner_width.saturating_sub(prefix_width).max(5); // Minimum clamp

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
                                        Style::default().fg(Color::Rgb(100,100,255)))
                },
                false => { Span::styled(task_lines[0].clone().to_string(), Style::default()) },
            };

            // 2b. Constructing the multiline ListItem
            let mut lines = vec![Line::from(vec![
                Span::raw("["),
                Span::styled(sym_c, Color::Green).bold(),
                Span::styled(sym_d, Color::Magenta).bold(), Span::raw("] "),
                Span::styled(sym_i, Color::LightRed).bold(), Span::raw(" "),
                sym_t
                // Span::raw(task_lines[0].clone())
            ])];

            // 2c. Add overflow lines with correct padding to match "[CD] * "
            for task_line in task_lines.into_iter().skip(1) {
                let sym_t = match item.delete {
                    true => { Span::styled(task_line.to_string(),
                                           Style::default().fg(Color::Rgb(100,100,255)))
                    },
                    false => { Span::styled(task_line.clone().to_string(), Style::default()) },
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
                    Span::from(list.title.as_str()).style(list.title_color),
                    Span::from(format!(" [{}/{}] ",srow,nrows))
                ]))
                .title_style(Style::default().bold())
                .borders(Borders::ALL)
                .border_style(border_color)
                .border_type(BorderType::Rounded))
            .bg(Color::Rgb(0,0,0))
            .fg(Color::White)
            .highlight_style(Style::default().bg(highlight_bg));

        f.render_stateful_widget(widget, panels[i], &mut list.state);
    }

    let input_block = Block::default()
        .title(" Add task: Type task followed by Enter  ")
        .title_style(Style::default().bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if matches!(app.input_mode, InputMode::Edit) {
            Style::default().fg(ORANGE)
        } else {
            Style::default()
        })
        .bg(Color::Rgb(0,0,0))
        .fg(Color::White);

    f.render_widget(Paragraph::new(app.input.as_str()).block(input_block), chunks[1]);

    let hotkey_style = Style::default().fg(Color::LightRed);
    let text_style = Style::default().fg(Color::White);
    let pipe_style = Style::default().fg(Color::DarkGray);
    let help_msg1 = Line::from(vec![
        Span::styled(" a", hotkey_style), Span::styled("dd", text_style),
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
        Span::styled("m", hotkey_style), Span::styled("ove ", text_style),
    ]).left_aligned();
    let help_msg2 = Line::from(vec![
        Span::styled(" s", hotkey_style), Span::styled("witch", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("copy item: ", text_style), Span::styled("ctrl-c ", hotkey_style),
        Span::styled(" | ", pipe_style),
        Span::styled("remove item: ", text_style),
        Span::styled("d", hotkey_style), Span::styled("elete then ", text_style),
        Span::styled("e", text_style), Span::styled("x", hotkey_style),
        Span::styled("punge", text_style),
        Span::styled(" | ", pipe_style),
        Span::styled("q", hotkey_style), Span::styled("uit ", text_style),
    ]).left_aligned();

    f.render_widget(Paragraph::new(vec![help_msg1, help_msg2])
                        .bg(HELP_BACKGROUND).fg(Color::White),
                    chunks[2]);
}
