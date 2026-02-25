pub struct App {
    pub should_quit: bool,
    pub selected_index: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            selected_index: 2,
        }
    }

    pub fn tick(&mut self) {}

    pub fn next_tab(&mut self) {
        self.selected_index = (self.selected_index + 1) % 4;
    }

    pub fn previous_tab(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = 2;
        } else {
            self.selected_index -= 1;
        }
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}
