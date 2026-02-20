pub struct App {
    pub counter: i32,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            counter: 0,
            should_quit: false,
        }
    }

    pub fn tick(&mut self) {}

    pub fn increment(&mut self) {
        self.counter += 1;
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}
