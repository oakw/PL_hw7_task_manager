use chrono::Utc;
use now::DateTimeNow;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use std::ops::Add;

use ratatui::widgets::*;

use crate::app::models::Task;
use crate::app::storage::Storage;

use super::ui::App;

// Possible task list sorting orders
#[derive(PartialEq)]
pub enum SortedBy {
    ByDueDate,
    ByName,
    ByPriority,
}

pub struct TaskList<'a, Task> {
    pub state: ListState,
    pub items: Vec<Task>,
    storage: &'a Storage,
    sorted_by: Option<SortedBy>,
}

impl<'a> TaskList<'a, Task> {
    // Initialize a task list with items from the database
    pub fn with_items_from_storage(storage: &'a Storage) -> TaskList<'a, Task> {
        TaskList {
            state: ListState::default(),
            items: storage.get_all_tasks(),
            storage: storage,
            sorted_by: None,
        }
    }

    // Refresh the items of this list with the items from the database
    pub fn update_items(&mut self) {
        self.items = self.storage.get_all_tasks();
    }

    // Move the selection to the next item
    // Coppied from original example
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.len() == 0 || i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    // Move the selection to the previous item
    // Coppied from original example
    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.len() == 0 {
                    0
                } else if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }

    // Change the state of the task to completed/to do; Save in database.
    pub fn toggle_completed(&mut self) {
        self.apply_for_selected_task({
            |task| {
                task.completed = !task.completed;
                self.storage
                    .update_task(task)
                    .expect("Failed to update a task");
            }
        });
    }

    // Perform a function on the object of the selected task
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

    // Delete the selected task from database; Update the items
    pub fn delete_selected(&mut self) {
        self.apply_for_selected_task({
            |task| {
                self.storage
                    .delete_task(task.id.unwrap_or(-1))
                    .expect("Failed to update a task");
            }
        });
        self.update_items();
    }

    // Get the selected task
    pub fn get_selected(&self) -> Option<&Task> {
        match self.state.selected() {
            Some(i) => self.items.get(i),
            None => None,
        }
    }

    // Get the uncompleted tasks
    pub fn get_uncompleted(&self) -> Vec<&Task> {
        return self
            .items
            .iter()
            .filter(|task| !task.completed)
            .collect::<Vec<&Task>>();
    }

    // Get the tasks due next week
    pub fn get_due_next_week(&self) -> Vec<&Task> {
        let next_week = Utc::now().add(chrono::Duration::weeks(1));
        return self
            .items
            .iter()
            .filter(|task| !task.completed && task.due_date < next_week)
            .collect::<Vec<&Task>>();
    }

    // Get the late tasks
    pub fn get_late(&self) -> Vec<&Task> {
        return self
            .items
            .iter()
            .filter(|task| !task.completed && task.due_date < Utc::now().beginning_of_day())
            .collect::<Vec<&Task>>();
    }

    // Sort the items by the given order
    pub fn set_sort(&mut self, sorted_by: SortedBy) {
        if self.sorted_by.is_some() && self.sorted_by.as_ref() == Some(&sorted_by) {
            self.items.reverse();
        } else {
            match &sorted_by {
                SortedBy::ByName => self.items.sort_by(|a, b| a.title.cmp(&b.title)),
                SortedBy::ByPriority => self.items.sort_by(|a, b| a.priority.cmp(&b.priority)),
                _ => self.items.sort_by(|a, b| a.due_date.cmp(&b.due_date)),
            }
        }

        self.sorted_by = Some(sorted_by);
    }
}

// Build the UI (list) for task list
pub fn get_list_items_ui<'a>(tasks: &'a [Task]) -> Vec<ListItem<'a>> {
    return tasks
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
}


// Build the UI (lines) for statistics infobox
pub fn get_statistics_ui<'a>(app: &'a App<'a>) -> Vec<Line<'a>> {
    return vec![
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
}

// Build the UI (lines) for instructions infobox
pub fn get_instructions_ui<'a>() -> Vec<Line<'a>> {
    return vec![
            "Enter - toggle do/done".into(),
            "a - add a task".into(),
            "e - edit a task".into(),
            "x - delete a task".into(),
            "d - sort by due date".into(),
            "f - sort by name".into(),
            "g - sort by priority".into(),
            "q - quit".into(),
        ];
}