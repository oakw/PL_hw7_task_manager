use chrono::NaiveDateTime;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::vec;

use crate::app::models::Task;
use crate::app::storage::Storage;
use derivative::Derivative;

use super::ui::App;

// State object for the task edit dialog
// Keeps track of the state of the dialog and the content of the task being edited
#[derive(Derivative)]
#[derivative(Default)]
pub struct TaskEditDialogState {
    pub dialog_active: bool,
    task_id: Option<i32>,
    content: Option<TaskEditDialogContent>,
    error_message: Option<String>,
    cursor_position: Option<(usize, usize)>,
}

// Current content of the task being edited/created
#[derive(Derivative)]
#[derivative(Default)]
struct TaskEditDialogContent {
    title: String,
    description: String,
    due_date: String,
    priority: i32,
}

// Refer to https://stackoverflow.com/a/66609806
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

impl TaskEditDialogState {
    // Opens the dialog and prepares to accept an input for the new task
    pub fn create_a_new_task(&mut self) {
        self.dialog_active = true;
        self.task_id = None;
        self.content = Some(TaskEditDialogContent::default());
    }

    // Opens the dialog and prepares to accept an input for the existing task
    pub fn edit_task(&mut self, task: &Task) {
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

    // Move the cursor one line BELOW the current one.
    // An overflow should be prevented, and the horizontal cursor position should be preserved if possible
    pub fn move_cursor_down(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        // TODO: Make this 3 dynamic
        let future_y_position = (cursor_position.1 + 1).min(3);
        self.cursor_position = Some((
            (cursor_position.0).min(self.content_of_string_at_y_pos(future_y_position).len()),
            future_y_position,
        ));
    }

    // Move the cursor one line ABOVE the current one.
    // An overflow should be prevented, and the horizontal cursor position should be preserved if possible
    pub fn move_cursor_up(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        if cursor_position.1 > 0 {
            self.cursor_position = Some((cursor_position.0, (cursor_position.1 - 1).max(0)));
        }
    }

    // Move the cursor one char LEFT to the current one.
    // An overflow should be prevented, and the vertical cursor position shall not change
    pub fn move_cursor_left(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        if cursor_position.0 > 0 {
            self.cursor_position = Some((cursor_position.0 - 1, cursor_position.1));
        }
    }

    // Move the cursor one char RIGHT to the current one.
    // An overflow should be prevented, and the vertical cursor position shall not change
    pub fn move_cursor_right(&mut self) {
        let cursor_position = self.cursor_position.unwrap_or((0, 0));
        self.cursor_position = Some((
            (cursor_position.0 + 1).min(self.content_of_string_at_y_pos(cursor_position.1).len()),
            cursor_position.1,
        ));
    }

    // Delete the char at the current cursor position
    pub fn delete_char(&mut self) {
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

    // Returns the content of the string at the given y position
    // Think of this as a mapper of vertical cursor position to the string content
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

    // Saves the task to the database
    pub fn save_task(&mut self, storage: &Storage) {
        let content = self.content.as_ref().unwrap_or_default();
        // Validate the input
        let date = match NaiveDateTime::parse_from_str(
            format!("{} 00:00:00Z", content.due_date).as_str(),
            "%d.%m.%Y %H:%M:%SZ",
        ) {
            Ok(date) => date,
            Err(_e) => {
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

        // Construct a task object
        let task = Task {
            id: self.task_id,
            title: content.title.clone(),
            description: content.description.clone(),
            due_date: date.and_utc(),
            priority: content.priority,
            completed: false,
        };

        // Update/insert the task and close the window
        if self.task_id.is_some() {
            storage.update_task(&task).expect("Failed to update a task");
        } else {
            storage.insert_task(&task).expect("Failed to create a task");
        }

        self.error_message = None;
        self.dialog_active = false;
    }

    // Handles the input of a char by appending it to the value of the currently active field
    pub fn input(&mut self, to_insert: char) {
        let mut cursor_position = self.cursor_position.unwrap_or((0, 0));
        if self.content_of_string_at_y_pos(cursor_position.1).len() == 0 {
            self.cursor_position = Some((0, cursor_position.1));
            cursor_position = self.cursor_position.unwrap_or((0, 0));
        }

        match self.content.as_mut() {
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
    }
}

// Returns the UI content for the task edit dialog
pub fn get_task_edit_ui<'a>(app: &'a App<'a>) -> Vec<Line<'a>> {
    const GRAY_TEXT: Style = Style::new().fg(Color::Rgb(62, 62, 62));
    const WHITE_TEXT: Style = Style::new().fg(Color::White);
    const BLACK_ON_WHITE: Style = Style::new().fg(Color::Black).bg(Color::White);
    let mut text = Vec::new();

    struct TextDialogInputLine {
        prefix: String,
        placeholder: String,
        value: String,
    }

    // Define the lines (input fie) of the dialog
    let lines = vec![
        TextDialogInputLine {
            prefix: "Title:       ".into(),
            placeholder: "My task name".into(),
            value: app.task_edit_dialog_state.content.as_ref().unwrap_or_default().title.clone(),
        },
        TextDialogInputLine {
            prefix: "Description: ".into(),
            placeholder: "My description".into(),
            value: app.task_edit_dialog_state.content.as_ref().unwrap_or_default().description.clone(),
        },
        TextDialogInputLine {
            prefix: "Due date:    ".into(),
            placeholder: "23.11.2023".into(),
            value: app.task_edit_dialog_state.content.as_ref().unwrap_or_default().due_date.clone(),
        },
        TextDialogInputLine {
            prefix: "Priority:    ".into(),
            placeholder: "0".into(),
            value: app.task_edit_dialog_state.content.as_ref().unwrap_or_default().priority.to_string(),
        },
    ];

    let cursor_position = app
        .task_edit_dialog_state
        .cursor_position
        .unwrap_or((lines[0].placeholder.len(), 0));

    for (i, line) in lines.iter().enumerate() {
        let mut spans = Vec::new();

        // Each line starts with a prefix, for example "Title: "
        spans.push(Span::styled(line.prefix.clone(), WHITE_TEXT));

        if line.value.len() == 0 {
            // If the line is empty, a placeholder is displayed
            if cursor_position.1 == i {
                // Line is selected. First char is highlighted, the rest is gray
                spans.push(Span::styled(
                    line.placeholder.chars().take(1).collect::<String>(),
                    BLACK_ON_WHITE,
                ));
                spans.push(Span::styled(
                    line.placeholder.chars().skip(1).collect::<String>(),
                    GRAY_TEXT,
                ));

            } else {
                // Line is not selected. All chars are gray
                spans.push(Span::styled(line.placeholder.clone(), GRAY_TEXT));
            }

        } else {
            // Line is not empty.
            if cursor_position.1 == i {
                // All chars are white, except for the one at the cursor position which is highlighted
                spans.push(Span::styled(
                    line.value
                        .clone()
                        .chars()
                        .take(cursor_position.0)
                        .collect::<String>(),
                    WHITE_TEXT,
                ));
                spans.push(Span::styled(
                    line.value
                        .clone()
                        .chars()
                        .skip(cursor_position.0)
                        .take(1)
                        .collect::<String>(),
                    BLACK_ON_WHITE,
                ));
                spans.push(Span::styled(
                    line.value
                        .clone()
                        .chars()
                        .skip(cursor_position.0 + 1)
                        .collect::<String>(),
                    WHITE_TEXT,
                ));

                if cursor_position.0 == line.value.len() {
                    spans.push(Span::styled(" ", BLACK_ON_WHITE));
                }
            } else {
                // All chars are white if the line is not selected
                spans.push(Span::styled(line.value.clone(), WHITE_TEXT));
            }
        }

        text.push(Line::from(spans));
    }

    text.push(Line::raw("\n"));

    // Display the error message if there is one
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

    // Display the help text
    text.push(Line::from(vec![Span::styled(
        "\nEnter - save, Esc - cancel",
        WHITE_TEXT,
    )]));

    return text;
}
