// Communication with SQLite
// Philosophy of CRUD lives here
// Based on https://github.com/rusqlite/rusqlite/blob/master/examples/persons/main.rs
use rusqlite::{Connection, Result};

use crate::app::models::Task;

pub struct Storage {
    pub db_con: Connection,
}

impl Storage {
    pub fn create_table_if_not_exists(&self) {
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

    // CREATE
    pub fn insert_task(&self, task: &Task) -> Result<usize> {
        return self.db_con.execute(
            "INSERT INTO task_item (Title, Description, DueDate, PriorityLevel, Completed) VALUES (?1, ?2, ?3, ?4, ?5);",
            (&task.title, &task.description, &task.due_date, &task.priority, &task.completed),
        );
    }

    // READ
    pub fn get_all_tasks(&self) -> Vec<Task> {
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

    // UPDATE
    pub fn update_task(&self, task: &Task) -> Result<usize> {
        return self.db_con.execute(
            "UPDATE task_item SET Title = ?, Description = ?, DueDate = ?, PriorityLevel = ?, Completed = ? WHERE Id = ?;",
            (&task.title, &task.description, &task.due_date, &task.priority, &task.completed, &task.id),
        );
    }

    // DELETE
    pub fn delete_task(&self, task_id: i32) -> Result<usize> {
        return self
            .db_con
            .execute("DELETE FROM task_item WHERE Id = ?;", [task_id]);
    }
}
