use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{prelude::*, widgets::*};
use std::{
    io,
    time::{Duration, Instant},
};

use crate::app::models::Task;
use crate::app::storage::Storage;
use crate::app::{task_edit::*, task_list::*};

pub struct App<'a> {
    pub items: crate::app::task_list::TaskList<'a, Task>,
    pub task_edit_dialog_state: TaskEditDialogState,
    pub storage: &'a Storage,
}

impl<'a> App<'a> {
    pub fn new(storage: &Storage) -> App {
        App {
            items: TaskList::with_items_from_storage(&storage),
            task_edit_dialog_state: TaskEditDialogState::default(),
            storage: &storage,
        }
    }
}

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let last_tick = Instant::now();
    loop {
        terminal.draw(|f| draw_ui(f, &mut app))?;
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.task_edit_dialog_state.dialog_active {
                        // Handle input for the task edit dialog
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
                        // Handle input for the task list navigation, sorting and state change
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

// Draws the whole user interface
fn draw_ui(f: &mut Frame, app: &mut App) {
    // Create two chunks of screen in 60-40 ratio
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(f.size());

    // DRAW LEFT PART
    // Create a List from all tasks and highlight the currently selected one
    let task_list = List::new(get_list_items_ui(app.items.items.as_slice()))
        .block(Block::default().borders(Borders::ALL).title("List"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    f.render_stateful_widget(task_list, chunks[0], &mut app.items.state);

    // DRAW RIGHT PART
    if app.task_edit_dialog_state.dialog_active {
        let create_or_edit_task = Paragraph::new(get_task_edit_ui(app))
            .block(Block::new().title("Add/Edit Task").borders(Borders::ALL))
            .style(Style::new().white());

        f.render_widget(create_or_edit_task, chunks[1]);
        
    } else {
        // If not editing, display statistics and instructions in vertically split layout
        let right_side = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let instructions = Paragraph::new(get_instructions_ui())
            .block(Block::new().title("Commands").borders(Borders::ALL))
            .style(Style::new().white());

        let statistics = Paragraph::new(get_statistics_ui(app))
            .block(Block::new().title("Statistics").borders(Borders::ALL))
            .style(Style::new().white());

        f.render_widget(instructions, right_side[0]);
        f.render_widget(statistics, right_side[1]);
    }
}
