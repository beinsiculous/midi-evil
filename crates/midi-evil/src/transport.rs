#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transport {
    pub start_tick: u64,
    pub end_tick: u64,
    pub repeat: bool,
}

impl Transport {
    pub fn new(bar_ticks: u64) -> Self {
        Self {
            start_tick: 0,
            end_tick: bar_ticks,
            repeat: false,
        }
    }

    pub fn range(self) -> (u64, Option<u64>) {
        (self.start_tick, self.repeat.then_some(self.end_tick))
    }

    pub fn start_bar(self, bar_ticks: u64) -> u64 {
        self.start_tick / bar_ticks + 1
    }

    pub fn end_bar(self, bar_ticks: u64) -> u64 {
        (self.end_tick / bar_ticks).max(self.start_bar(bar_ticks))
    }

    pub fn set_bars(&mut self, start: u64, end: u64, bar_ticks: u64) {
        let start = start.max(1);
        let end = end.max(start + 1);
        self.start_tick = (start - 1) * bar_ticks;
        self.end_tick = end * bar_ticks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_plays_from_start_and_repeats_the_full_section() {
        let mut transport = Transport::new(480);
        assert_eq!(transport.range(), (0, None));
        transport.set_bars(3, 5, 480);
        assert_eq!(transport.start_tick, 960);
        assert_eq!(transport.end_tick, 2400);
        transport.repeat = true;
        assert_eq!(transport.range(), (960, Some(2400)));
        transport.set_bars(4, 4, 480);
        assert_eq!(transport.end_bar(480), 5);
    }
}
