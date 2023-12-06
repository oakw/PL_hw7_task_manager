use chrono::{DateTime, Utc};

pub struct Task {
    pub id: Option<i32>,
    pub title: String,
    pub description: String,
    pub due_date: DateTime<Utc>,
    pub priority: i32,
    pub completed: bool,
}
