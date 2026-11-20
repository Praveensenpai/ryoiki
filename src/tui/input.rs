use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::actions::Effect;
use crate::tui::app::{Action, App, Focus, Tab};
use crate::tui::views;

/// Dispatches one key press against the current modal or the dashboard.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Effect {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Effect::Quit;
    }
    if app.help {
        app.help = false;
        return Effect::None;
    }
    if app.choice.is_some() {
        return handle_choice(app, key);
    }
    if app.confirm.is_some() {
        return handle_confirm(app, key);
    }
    if app.input.is_some() {
        return handle_input(app, key);
    }
    handle_normal(app, key)
}

fn handle_choice(app: &mut App, key: KeyEvent) -> Effect {
    let Some(mut choice) = app.choice.take() else {
        return Effect::None;
    };
    if choice.options.is_empty() {
        return Effect::None;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            choice.index = (choice.index + 1) % choice.options.len();
            app.choice = Some(choice);
            Effect::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            choice.index = (choice.index + choice.options.len() - 1) % choice.options.len();
            app.choice = Some(choice);
            Effect::None
        }
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            let idx = (c as usize) - ('1' as usize);
            if idx < choice.options.len() {
                Effect::Action(choice.options[idx].action.clone())
            } else {
                app.choice = Some(choice);
                Effect::None
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            Effect::Action(choice.options[choice.index].action.clone())
        }
        KeyCode::Esc | KeyCode::Char('q' | 'n') => Effect::None,
        _ => {
            app.choice = Some(choice);
            Effect::None
        }
    }
}

fn handle_confirm(app: &mut App, key: KeyEvent) -> Effect {
    let Some(confirm) = app.confirm.take() else {
        return Effect::None;
    };
    match key.code {
        KeyCode::Char('y' | 'Y') | KeyCode::Enter => Effect::Action(confirm.action),
        KeyCode::Char('n' | 'N') | KeyCode::Esc => Effect::None,
        _ => {
            app.confirm = Some(confirm);
            Effect::None
        }
    }
}

fn handle_input(app: &mut App, key: KeyEvent) -> Effect {
    match key.code {
        KeyCode::Enter => {
            app.submit_input();
            return Effect::None;
        }
        KeyCode::Esc => {
            app.input = None;
            return Effect::None;
        }
        _ => {}
    }

    let Some(input) = app.input.as_mut() else {
        return Effect::None;
    };
    match key.code {
        KeyCode::Backspace => {
            input.value.pop();
        }
        KeyCode::Delete => input.value.clear(),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => input.value.clear(),
        KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.value.push(character);
        }
        _ => {}
    }
    Effect::None
}

fn handle_normal(app: &mut App, key: KeyEvent) -> Effect {
    match key.code {
        KeyCode::Char('q') => Effect::Quit,
        KeyCode::Char('?') => {
            app.help = true;
            Effect::None
        }
        KeyCode::Tab => {
            toggle_focus(app);
            Effect::None
        }
        KeyCode::BackTab => {
            app.prev_tab();
            views::ensure_cursor(app);
            Effect::None
        }
        KeyCode::Char(character @ '1'..='8') => {
            switch_tab(app, character);
            Effect::None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            horizontal(app, -1);
            Effect::None
        }
        KeyCode::Right | KeyCode::Char('l') => {
            horizontal(app, 1);
            Effect::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            vertical(app, false);
            Effect::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            vertical(app, true);
            Effect::None
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.set_cursor(views::first_selectable(app));
            Effect::None
        }
        KeyCode::End | KeyCode::Char('G') => {
            last_selectable(app);
            Effect::None
        }
        KeyCode::Enter | KeyCode::Char(' ') => activate(app),
        KeyCode::Esc => {
            app.focus = Focus::Nav;
            Effect::None
        }
        KeyCode::Char('r') => dispatch(Action::Refresh),
        KeyCode::Char('s') => dispatch(Action::SaveSettings),
        _ => Effect::None,
    }
}

fn switch_tab(app: &mut App, character: char) {
    let index = character as usize - '1' as usize;
    app.set_tab(Tab::ALL[index]);
    views::ensure_cursor(app);
}

fn activate(app: &mut App) -> Effect {
    if app.focus == Focus::Nav {
        app.focus = Focus::Content;
        views::ensure_cursor(app);
        return Effect::None;
    }
    dispatch(views::activate(app, app.cursor()))
}

fn horizontal(app: &mut App, delta: i32) {
    if app.focus == Focus::Nav {
        if delta < 0 {
            app.prev_tab();
        } else {
            app.next_tab();
        }
        views::ensure_cursor(app);
    } else {
        views::adjust(app, app.cursor(), delta);
    }
}

fn vertical(app: &mut App, forward: bool) {
    if app.focus == Focus::Nav {
        if forward {
            app.next_tab();
        } else {
            app.prev_tab();
        }
        views::ensure_cursor(app);
    } else {
        views::move_cursor(app, forward);
    }
}

fn toggle_focus(app: &mut App) {
    app.focus = match app.focus {
        Focus::Nav => Focus::Content,
        Focus::Content => Focus::Nav,
    };
    if app.focus == Focus::Content {
        views::ensure_cursor(app);
    }
}

fn last_selectable(app: &mut App) {
    let rows = views::rows(app);
    if let Some((index, _)) = rows
        .iter()
        .enumerate()
        .rev()
        .find(|(_, row)| row.is_selectable())
    {
        app.set_cursor(index);
    }
}

fn dispatch(action: Action) -> Effect {
    match action {
        Action::None => Effect::None,
        other => Effect::Action(other),
    }
}
