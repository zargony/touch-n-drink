use embassy_futures::select::select_array;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyInput, AnyOutputOpenDrain};
use log::{debug, info};

/// Time to wait for an output pin to settle before scanning inputs
const OUTPUT_SETTLE_TIME: Duration = Duration::from_micros(1);

/// Time to wait for debounce after detected keypress
const INPUT_DEBOUNCE_TIME: Duration = Duration::from_millis(10);

/// Key that can be pressed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Digit(u8),
    Enter,
    Cancel,
    Other(char),
}

impl Key {
    /// Returns digit as number if key is a digit
    #[allow(dead_code)]
    pub fn digit(self) -> Option<usize> {
        match self {
            Self::Digit(n) => Some(n as usize),
            _ => None,
        }
    }

    /// Returns true if key is enter key
    #[allow(dead_code)]
    pub fn enter(self) -> bool {
        self == Self::Enter
    }

    /// Returns true if key is cancel key
    #[allow(dead_code)]
    pub fn cancel(self) -> bool {
        self == Self::Cancel
    }

    /// Returns key as character
    #[allow(dead_code)]
    pub fn as_char(self) -> char {
        match self {
            Self::Digit(n) => char::from_digit(u32::from(n), 16).unwrap_or('?'),
            Self::Enter => '#',
            Self::Cancel => '*',
            Self::Other(ch) => ch,
        }
    }
}

/// Matrix keypad driver
pub struct Keypad<'a, const COLS: usize, const ROWS: usize> {
    cols: [AnyInput<'a>; COLS],
    rows: [AnyOutputOpenDrain<'a>; ROWS],
}

impl<'a, const COLS: usize, const ROWS: usize> Keypad<'a, COLS, ROWS> {
    /// Create matrix keypad from given input columns and output rows
    pub fn new(cols: [AnyInput<'a>; COLS], rows: [AnyOutputOpenDrain<'a>; ROWS]) -> Self {
        info!("Keypad: {ROWS}x{COLS} matrix initialized");
        Self { cols, rows }
    }

    /// Wait for keypress and return scancode of pressed key
    pub async fn read_scancode(&mut self) -> usize {
        loop {
            // Wait for any key pressed
            self.wait_for_keypress().await;
            // Wait for bounced contacts to settle. Not a perfect debounce, but simple and good enough.
            Timer::after(INPUT_DEBOUNCE_TIME).await;
            // Scan keypad for pressed keys
            let states = self.scan().await;
            // TODO: Use better algorithm to detect pressed key? (e.g. compare to previous states)
            for (y, row) in states.iter().enumerate() {
                for (x, state) in row.iter().enumerate() {
                    if *state {
                        return y * COLS + x;
                    }
                }
            }
            // Keypress detected, but no pressed key scanned. Happens when contacts bounce on release.
        }
    }
}

impl<'a, const COLS: usize, const ROWS: usize> Keypad<'a, COLS, ROWS> {
    /// Wait for any key to be pressed
    async fn wait_for_keypress(&mut self) {
        // Assuming inputs have pull up resistors, so keys will pull low when pressed
        for out in &mut self.rows {
            out.set_low();
        }
        // Wait for any input to be pulled low
        select_array(self.cols.each_mut().map(AnyInput::wait_for_falling_edge)).await;
    }

    /// Scan all keys and return array of pressed false/true states
    async fn scan(&mut self) -> [[bool; COLS]; ROWS] {
        // Assuming inputs have pull up resistors, so keys will pull low when pressed
        for out in &mut self.rows {
            out.set_high();
        }
        let mut states = [[false; COLS]; ROWS];
        for (output, states) in self.rows.iter_mut().zip(states.iter_mut()) {
            output.set_low();
            Timer::after(OUTPUT_SETTLE_TIME).await;
            // Easier with feature array_try_map (see https://github.com/rust-lang/rust/issues/79711):
            //   `self.cols.each_mut().try_map(AnyInput::is_low)?`
            for (input, state) in self.cols.iter_mut().zip(states.iter_mut()) {
                *state = input.is_low();
            }
            output.set_high();
        }
        states
    }
}

impl<'a> Keypad<'a, 3, 4> {
    // 1 2 3
    // 4 5 6
    // 7 8 9
    // * 0 #
    const KEYS: [Key; 12] = [
        Key::Digit(1),
        Key::Digit(2),
        Key::Digit(3),
        Key::Digit(4),
        Key::Digit(5),
        Key::Digit(6),
        Key::Digit(7),
        Key::Digit(8),
        Key::Digit(9),
        Key::Cancel,
        Key::Digit(0),
        Key::Enter,
    ];

    /// Wait for keypress and return pressed key
    pub async fn read(&mut self) -> Key {
        let scancode = self.read_scancode().await;
        let key = Self::KEYS[scancode];
        debug!("Keypad: {:?} pressed", key);
        key
    }
}

#[allow(dead_code)]
impl<'a> Keypad<'a, 4, 4> {
    // 1 2 3 A
    // 4 5 6 B
    // 7 8 9 C
    // * 0 # D
    const KEYS: [Key; 16] = [
        Key::Digit(1),
        Key::Digit(2),
        Key::Digit(3),
        Key::Other('A'),
        Key::Digit(4),
        Key::Digit(5),
        Key::Digit(6),
        Key::Other('B'),
        Key::Digit(7),
        Key::Digit(8),
        Key::Digit(9),
        Key::Other('C'),
        Key::Cancel,
        Key::Digit(0),
        Key::Enter,
        Key::Other('D'),
    ];

    /// Wait for keypress and return pressed key
    pub async fn read(&mut self) -> Key {
        let scancode = self.read_scancode().await;
        let key = Self::KEYS[scancode];
        debug!("Keypad: {:?} pressed", key);
        key
    }
}
