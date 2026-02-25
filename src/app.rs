pub struct App {
    pub counter: i32,
    pub should_quit: bool,
    pub selected_index: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            counter: 0,
            should_quit: false,
            selected_index: 2,
        }
    }

    pub fn tick(&mut self) {}

    pub fn increment(&mut self) {
        self.counter += 1;
    }

    pub fn next_tab(&mut self) {
        self.selected_index = (self.selected_index + 1) % 6;
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
