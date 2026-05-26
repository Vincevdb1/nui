use ratatui::widgets::{ListState, TableState};

pub struct ListWrapper<'a, S> {
    pub state: &'a mut S,
    pub items_len: usize,
}

impl<'a> ListWrapper<'a, ListState> {
    pub fn new(state: &'a mut ListState, items_len: usize) -> Self {
        Self { state, items_len }
    }

    pub fn next(&mut self) {
        if self.items_len == 0 {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items_len.saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.items_len == 0 {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items_len.saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

impl<'a> ListWrapper<'a, TableState> {
    pub fn new_table(state: &'a mut TableState, items_len: usize) -> Self {
        Self { state, items_len }
    }

    pub fn next(&mut self) {
        if self.items_len == 0 {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items_len {
                    1
                } else {
                    i + 1
                }
            }
            None => 1,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.items_len == 0 {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i <= 1 {
                    self.items_len
                } else {
                    i - 1
                }
            }
            None => 1,
        };
        self.state.select(Some(i));
    }
}
