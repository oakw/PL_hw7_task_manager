use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{Connection, Result};
use now::DateTimeNow;
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
    vec, ops::Add,
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, widgets::*};

use derivative::Derivative;

#[derive(PartialEq)]
enum SortedBy {
    ByDueDate,
    ByName,
    ByPriority,
}
struct StatefulList<'a, Task> {
    state: ListState,
    items: Vec<Task>,
    storage: &'a Storage,
    sorted_by: Option<SortedBy>,
}

impl<'a> StatefulList<'a, Task> {
    fn with_items_from_storage(storage: &'a Storage) -> StatefulList<'a, Task> {
        StatefulList {
            state: ListState::default(),
            items: storage.get_all_tasks(),
            storage: storage,
            sorted_by: None,
        }
    }

    fn update_items(&mut self) {
        self.items = self.storage.get_all_tasks();
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn unselect(&mut self) {
        self.state.select(None);
    }

    fn toggle_completed(&mut self) {
        self.apply_for_selected_task({
            |task| {
                task.completed = !task.completed;
                self.storage
                    .update_task(task)
                    .expect("Failed to update a task");
            }
        });
    }

    fn apply_for_selected_task(&mut self, function: impl Fn(&mut Task)) {
        match self.state.selected() {
            Some(i) => {
                match self.items.get_mut(i) {
                    Some(item) => {
                        function(item);
                    }
                    None => return,
                };
            }
            None => {}
        };
    }

    fn delete_selected(&mut self) {
        self.apply_for_selected_task({
            |task| {
                self.storage
                    .delete_task(task.id.unwrap_or(-1))
                    .expect("Failed to update a task");
            }
        });
        self.update_items();
    }

    fn get_selected(&self) -> Option<&Task> {
        match self.state.selected() {
            Some(i) => self.items.get(i),
            None => None,
        }
    }

    fn get_uncompleted(&self) -> Vec<&Task> {
        return self.items .iter() .filter(|task| !task.completed) .collect::<Vec<&Task>>();
    }

    fn get_due_next_week(&self) -> Vec<&Task> {
        let next_week = Utc::now().add(chrono::Duration::weeks(1));
        return self.items .iter() .filter(|task| !task.completed && task.due_date < next_week) .collect::<Vec<&Task>>();
    }

    fn get_late(&self) -> Vec<&Task> {
        return self.items .iter() .filter(|task| !task.completed && task.due_date < Utc::now().beginning_of_day()) .collect::<Vec<&Task>>();
    }

    fn set_sort(&mut self, sorted_by: SortedBy) {
        if self.sorted_by.is_some() && self.sorted_by.as_ref() == Some(&sorted_by) {
            self.items.reverse();
        } else {
            match &sorted_by {
                SortedBy::ByName => self.items.sort_by(|a, b| a.title.cmp(&b.title) ),
                SortedBy::ByPriority => self.items.sort_by(|a, b| a.priority.cmp(&b.priority)),
                _ => self.items.sort_by(|a, b| a.due_date.cmp(&b.due_date)),
            }
        }
        
        self.sorted_by = Some(sorted_by);
    }
}

struct TaskEditDialogState {
    dialog_active: bool,
    task_id: Option<i32>,
    content: Option<TaskEditDialogContent>,
    error_message: Option<String>,
    cursor_position: Option<(usize, usize)>,
}

#[derive(Derivative)]
#[derivative(Default)]
struct TaskEditDialogContent {
    title: String,
    description: String,
    due_date: String,
    priority: i32,
}

impl TaskEditDialogState {
    fn create_a_new_task(&mut self) {
        self.dialog_active = true;
        self.task_id = None;
        self.content = Some(TaskEditDialogContent::default());
    }

    fn edit_task(&mut self, task: &Task) {
        self.dialog_active = true;
        self.task_id = task.id;
        self.cursor_position = Some((0, 0));
        self.content = Some(TaskEditDialogContent {
            title: task.title.clone(),
            description: task.description.clone(),
            due_date: task.due_date.format("%d.%m.%Y").to_string(),
            priority: task.priority,
        });
    }

    fn move_cursor_down(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        // TODO: Make this 3 dynamic
        let future_y_position = (cursor_position.1 + 1).min(3);
        self.cursor_position = Some((
            (cursor_position.0).min(self.content_of_string_at_y_pos(future_y_position).len()),
            future_y_position,
        ));
    }

    fn move_cursor_up(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        if cursor_position.1 > 0 {
            self.cursor_position = Some((cursor_position.0, (cursor_position.1 - 1).max(0)));
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        if cursor_position.0 > 0 {
            self.cursor_position = Some((cursor_position.0 - 1, cursor_position.1));
        }
    }

    fn move_cursor_right(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        self.cursor_position = Some((
            (cursor_position.0 + 1).min(self.content_of_string_at_y_pos(cursor_position.1).len()),
            cursor_position.1,
        ));
    }

    fn delete_char(&mut self) {
        let mut cursor_position = self.cursor_position.unwrap_or((0, 0));
        if cursor_position.0 == 0 {
            return;
        }

        let content_length = self.content_of_string_at_y_pos(cursor_position.1).len();
        if cursor_position.0 >= content_length {
            cursor_position.0 -= 1;
        }

        match self.content.as_mut() {
            Some(content) => match cursor_position.1 {
                0 => content.title.remove(cursor_position.0),
                1 => content.description.remove(cursor_position.0),
                2 => content.due_date.remove(cursor_position.0),
                _ => ' ',
            },
            None => return,
        };
        self.move_cursor_left();
    }

    fn content_of_string_at_y_pos(&self, y_position: usize) -> String {
        return match y_position {
            0 => self.content.as_ref().unwrap_or_default().title.clone(),
            1 => self
                .content
                .as_ref()
                .unwrap_or_default()
                .description
                .clone(),
            2 => self.content.as_ref().unwrap_or_default().due_date.clone(),
            3 => self
                .content
                .as_ref()
                .unwrap_or_default()
                .priority
                .to_string(),
            _ => "".to_string(),
        };
    }

    fn save_task(&mut self, storage: &Storage) {
        let content = self.content.as_ref().unwrap_or_default();
        let date = match NaiveDateTime::parse_from_str(
            format!("{} 00:00:00Z", content.due_date).as_str(),
            "%d.%m.%Y %H:%M:%SZ",
        ) {
            Ok(date) => date,
            Err(e) => {
                self.error_message = Some("Date should be in format dd.mm.yyyy".to_string());
                return;
            }
        };
        if content.title.len() == 0 {
            self.error_message = Some("Title cannot be empty".to_string());
            return;
        } else if content.description.len() == 0 {
            self.error_message = Some("Description cannot be empty".to_string());
            return;
        }

        let task = Task {
            id: self.task_id,
            title: content.title.clone(),
            description: content.description.clone(),
            due_date: date.and_utc(),
            priority: content.priority,
            completed: false,
        };
        
        if self.task_id.is_some() {
            storage.update_task(&task).expect("Failed to update a task");
        } else {
            storage.insert_task(&task).expect("Failed to create a task");
        }

        self.error_message = None;
        self.dialog_active = false;
    }

    fn input(&mut self, to_insert: char) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        let content = match self.content.as_mut() {
            Some(content) => match cursor_position.1 {
                0 => content.title.insert(cursor_position.0, to_insert),
                1 => content.description.insert(cursor_position.0, to_insert),
                2 => content.due_date.insert(cursor_position.0, to_insert),
                3 => {
                    if vec!['0', '1', '2'].contains(&to_insert) {
                        content.priority = to_insert.to_string().parse::<i32>().unwrap_or(0)
                    }
                }

                _ => {}
            },
            None => return,
        };
        self.move_cursor_right();
        // self.content = Some(content);
        // self.move_cursor_down();
    }
}

// https://stackoverflow.com/a/66609806
impl<'a> Default for &'a TaskEditDialogContent {
    fn default() -> &'a TaskEditDialogContent {
        static VALUE: TaskEditDialogContent = TaskEditDialogContent {
            title: String::new(),
            description: String::new(),
            due_date: String::new(),
            priority: 0,
        };
        &VALUE
    }
}

/// This struct holds the current state of the app. In particular, it has the `items` field which is
/// a wrapper around `ListState`. Keeping track of the items state let us render the associated
/// widget with its state and have access to features such as natural scrolling.
///
/// Check the event handling at the bottom to see how to change the state on incoming events.
/// Check the drawing logic for items on how to specify the highlighting style for selected items.
struct App<'a> {
    items: StatefulList<'a, Task>,
    task_edit_dialog_state: TaskEditDialogState,
    storage: &'a Storage,
}

impl<'a> App<'a> {
    fn new(storage: &Storage) -> App {
        App {
            items: StatefulList::with_items_from_storage(&storage),
            task_edit_dialog_state: TaskEditDialogState {
                dialog_active: false,
                task_id: None,
                content: None,
                cursor_position: None,
                error_message: None,
            },
            storage: &storage,
        }
    }
}

pub fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(250);
    let storage = Storage {
        db_con: Connection::open("database.db").expect("Failed to open the DB file"),
    };
    let app = App::new(&storage);
    // &app.items.set_sort(SortedBy::ByDueDate);
    let res = run_app(&mut terminal, app, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.task_edit_dialog_state.dialog_active {
                        match key.code {
                            KeyCode::Down => app.task_edit_dialog_state.move_cursor_down(),
                            KeyCode::Up => app.task_edit_dialog_state.move_cursor_up(),
                            KeyCode::Esc => app.task_edit_dialog_state.dialog_active = false,
                            KeyCode::Enter => {
                                app.task_edit_dialog_state.save_task(&app.storage);
                                app.items.update_items();
                            }
                            KeyCode::Left => app.task_edit_dialog_state.move_cursor_left(),
                            KeyCode::Right => app.task_edit_dialog_state.move_cursor_right(),
                            KeyCode::Backspace => app.task_edit_dialog_state.delete_char(),
                            KeyCode::Char(to_insert) => app.task_edit_dialog_state.input(to_insert),
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('x') => app.items.delete_selected(),
                            KeyCode::Left => app.items.unselect(),
                            KeyCode::Down => app.items.next(),
                            KeyCode::Up => app.items.previous(),
                            KeyCode::Char('a') => app.task_edit_dialog_state.create_a_new_task(),
                            KeyCode::Char('e') => match app.items.get_selected() {
                                Some(task) => app.task_edit_dialog_state.edit_task(task),
                                None => {}
                            },
                            KeyCode::Char('d') => app.items.set_sort(SortedBy::ByDueDate),
                            KeyCode::Char('f') => app.items.set_sort(SortedBy::ByName),
                            KeyCode::Char('g') => app.items.set_sort(SortedBy::ByPriority),
                            KeyCode::Enter => app.items.toggle_completed(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    // Create two chunks with equal horizontal screen space
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(f.size());

    // Iterate through all elements in the `items` app and append some debug text to it.
    let items: Vec<ListItem> = app
        .items
        .items
        .iter()
        .map(|i| {
            let mut lines = Vec::new();

            let title_color = match i.priority {
                1 => Color::Yellow,
                2 => Color::Red,
                _ => Color::White,
            };

            lines.push(Line::from(vec![
                Span::from(if i.completed { "[✓] " } else { "[ ] " }),
                Span::from(i.title.as_str()).fg(title_color),
            ]));

            lines.push(Line::from(vec![
                Span::from(format!("    Due: {}", i.due_date.format("%d.%m.%Y"))),
                Span::from(format!(" Description: {}", i.description)),
            ]));
            ListItem::new(lines).style(Style::default().fg(Color::White))
        })
        .collect();

    // Create a List from all list items and highlight the currently selected one
    let items = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("List"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // println!("what here");
    // We can now render the item list
    f.render_stateful_widget(items, chunks[0], &mut app.items.state);

    if app.task_edit_dialog_state.dialog_active {
        const GRAY_TEXT: Style = Style::new().fg(Color::Rgb(62, 62, 62));
        const WHITE_TEXT: Style = Style::new().fg(Color::White);
        const BLACK_ON_WHITE: Style = Style::new().fg(Color::Black).bg(Color::White);

        struct TextDialogInputLine {
            prefix: String,
            placeholder: String,
            value: String,
        }

        let lines = vec![
            TextDialogInputLine {
                prefix: "Title:       ".into(),
                placeholder: "My task name".into(),
                value: app
                    .task_edit_dialog_state
                    .content
                    .as_ref()
                    .unwrap_or_default()
                    .title
                    .clone(),
            },
            TextDialogInputLine {
                prefix: "Description: ".into(),
                placeholder: "My description".into(),
                value: app
                    .task_edit_dialog_state
                    .content
                    .as_ref()
                    .unwrap_or_default()
                    .description
                    .clone(),
            },
            TextDialogInputLine {
                prefix: "Due date:    ".into(),
                placeholder: "23.11.2023".into(),
                value: app
                    .task_edit_dialog_state
                    .content
                    .as_ref()
                    .unwrap_or_default()
                    .due_date
                    .clone(),
            },
            TextDialogInputLine {
                prefix: "Priority:    ".into(),
                placeholder: "0".into(),
                value: app
                    .task_edit_dialog_state
                    .content
                    .as_ref()
                    .unwrap_or_default()
                    .priority
                    .to_string(),
            },
        ];

        let mut text = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            let cursor_position = app
                .task_edit_dialog_state
                .cursor_position
                .unwrap_or((line.placeholder.len(), 0));
            let mut spans = Vec::new();

            spans.push(Span::styled(&line.prefix, WHITE_TEXT));

            if line.value.len() == 0 {
                if cursor_position.1 == i {
                    spans.push(Span::styled(
                        line.placeholder.chars().take(1).collect::<String>(),
                        BLACK_ON_WHITE,
                    ));
                    spans.push(Span::styled(
                        line.placeholder.chars().skip(1).collect::<String>(),
                        GRAY_TEXT,
                    ));
                } else {
                    spans.push(Span::styled(&line.placeholder, GRAY_TEXT));
                }
            } else {
                if cursor_position.1 == i {
                    spans.push(Span::styled(
                        line.value
                            .chars()
                            .take(cursor_position.0)
                            .collect::<String>(),
                        WHITE_TEXT,
                    ));
                    spans.push(Span::styled(
                        line.value
                            .chars()
                            .skip(cursor_position.0)
                            .take(1)
                            .collect::<String>(),
                        BLACK_ON_WHITE,
                    ));
                    spans.push(Span::styled(
                        line.value
                            .chars()
                            .skip(cursor_position.0 + 1)
                            .collect::<String>(),
                        WHITE_TEXT,
                    ));

                    if cursor_position.0 == line.value.len() {
                        spans.push(Span::styled(" ", BLACK_ON_WHITE));
                    }
                } else {
                    spans.push(Span::styled(&line.value, WHITE_TEXT));
                }
            }

            text.push(Line::from(spans));
        }

        text.push(Line::raw("\n"));

        match app.task_edit_dialog_state.error_message {
            Some(ref error_message) => {
                text.push(Line::from(vec![Span::styled(
                    error_message,
                    Style::new().fg(Color::Red),
                )]));
                text.push(Line::raw("\n"));
            }
            None => {}
        }

        text.push(Line::from(vec![Span::styled(
            "\nEnter - save, Esc - cancel",
            WHITE_TEXT,
        )]));

        let events_list = Paragraph::new(text)
            .block(Block::new().title("Add/Edit Task").borders(Borders::ALL))
            .style(Style::new().white());
        f.render_widget(events_list, chunks[1]);
    } else {
        let right_side = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

        let text = vec![
            "Enter - toggle do/done".into(),
            "a - add a task".into(),
            "e - edit a task".into(),
            "x - delete a task".into(),
            "d - sort by due date".into(),
            "f - sort by name".into(),
            "g - sort by priority".into(),
            "q - quit".into(),
        ];
        let events_list = Paragraph::new(text)
            .block(Block::new().title("Commands").borders(Borders::ALL))
            .style(Style::new().white());

        let text = vec![
            Line::from(format!("Total tasks: {}", app.items.items.len())),
            Line::from(format!(
                "Uncompleted tasks: {}",
                app.items.get_uncompleted().len()
            )),
            Line::from(format!(
                "Due next week: {}",
                app.items.get_due_next_week().len()
            )),
            Line::from(format!("Late: {}", app.items.get_late().len())),
        ];
        let statistics = Paragraph::new(text)
            .block(Block::new().title("Statistics").borders(Borders::ALL))
            .style(Style::new().white());
        f.render_widget(events_list, right_side[0]);
        f.render_widget(statistics, right_side[1]);
    }
}

#[derive(Debug)]
struct Task {
    id: Option<i32>,
    title: String,
    description: String,
    due_date: DateTime<Utc>,
    priority: i32,
    completed: bool,
}

struct Storage {
    db_con: Connection,
}

impl Storage {
    fn create_table_if_not_exists(&self) {
        self.db_con
            .execute(
                "CREATE TABLE IF NOT EXISTS task_item (
                Id INTEGER PRIMARY KEY AUTOINCREMENT,
                Title TEXT,
                Description TEXT,
                DueDate DATETIME,
                PriorityLevel INT,
                Completed TINYINT
            );",
                (),
            )
            .expect("Could not create the initial DB table");
    }

    fn insert_task(&self, task: &Task) -> Result<usize> {
        return self.db_con.execute(
            "INSERT INTO task_item (Title, Description, DueDate, PriorityLevel, Completed) VALUES (?1, ?2, ?3, ?4, ?5);",
            (&task.title, &task.description, &task.due_date, &task.priority, &task.completed),
        );
    }

    fn update_task(&self, task: &Task) -> Result<usize> {
        return self.db_con.execute(
            "UPDATE task_item SET Title = ?, Description = ?, DueDate = ?, PriorityLevel = ?, Completed = ? WHERE Id = ?;",
            (&task.title, &task.description, &task.due_date, &task.priority, &task.completed, &task.id),
        );
    }

    fn delete_task(&self, task_id: i32) -> Result<usize> {
        return self
            .db_con
            .execute("DELETE FROM task_item WHERE Id = ?;", [task_id]);
    }

    fn get_all_tasks(&self) -> Vec<Task> {
        let mut stmt = self
            .db_con
            .prepare("SELECT * FROM task_item")
            .expect("Failed to prepare for task retrieval");
        let results = stmt.query_map([], |row| {
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                due_date: row.get(3)?,
                priority: row.get(4)?,
                completed: row.get(5)?,
            })
        });

        return match results {
            Ok(tasks) => tasks.filter_map(|task_result| task_result.ok()).collect(),
            Err(_) => Vec::new(),
        };
    }
}
