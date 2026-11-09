pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 32;
pub const RAM_SIZE: usize = 4096;
const REGISTER_COUNT: usize = 16;
const STACK_SIZE: usize = 16;
const PROGRAM_START: usize = 0x200;

// Pixel format is 0RGB (minifb)
const PX_OFF: u32 = 0xff000000;
const PX_ON: u32 = 0xffffffff;

const FONT: [[u8; 5]; 16] = [
    [0xF0, 0x90, 0x90, 0x90, 0xF0], // 0
    [0x20, 0x60, 0x20, 0x20, 0x70], // 1
    [0xF0, 0x10, 0xF0, 0x80, 0xF0], // 2
    [0xF0, 0x10, 0xF0, 0x10, 0xF0], // 3
    [0x90, 0x90, 0xF0, 0x10, 0x10], // 4
    [0xF0, 0x80, 0xF0, 0x10, 0xF0], // 5
    [0xF0, 0x80, 0xF0, 0x90, 0xF0], // 6
    [0xF0, 0x10, 0x20, 0x40, 0x40], // 7
    [0xF0, 0x90, 0xF0, 0x90, 0xF0], // 8
    [0xF0, 0x90, 0xF0, 0x10, 0xF0], // 9
    [0xF0, 0x90, 0xF0, 0x90, 0x90], // A
    [0xE0, 0x90, 0xE0, 0x90, 0xE0], // B
    [0xF0, 0x80, 0x80, 0x80, 0xF0], // C
    [0xE0, 0x90, 0x90, 0x90, 0xE0], // D
    [0xF0, 0x80, 0xF0, 0x80, 0xF0], // E
    [0xF0, 0x80, 0xF0, 0x80, 0x80], // F
];

enum ProgramCounter {
    Next,
    Skip,
    Jump(u16),
    Stay, // don't advance (used while waiting for a key)
}

// Reference: http://devernay.free.fr/hacks/chip8/C8TECH10.HTM
pub struct C8 {
    ram: [u8; RAM_SIZE],
    pub v: [u8; REGISTER_COUNT],
    pub display: [u32; WIDTH * HEIGHT],
    i: u16,
    pub dt: u8,
    pub st: u8,
    stack: Vec<u16>,
    pub pc: u16,
}

impl C8 {
    pub fn new() -> C8 {
        C8 {
            ram: [0; RAM_SIZE],
            v: [0; REGISTER_COUNT],
            display: [PX_OFF; WIDTH * HEIGHT],
            i: 0,
            dt: 0,
            st: 0,
            stack: Vec::new(),
            pc: PROGRAM_START as u16,
        }
    }

    pub fn load_ram(&mut self, rom: &[u8]) {
        for (idx, byte) in FONT.iter().flatten().enumerate() {
            self.ram[idx] = *byte;
        }
        if PROGRAM_START + rom.len() > RAM_SIZE {
            panic!("Out of memory: program too large");
        }
        self.ram[PROGRAM_START..PROGRAM_START + rom.len()].copy_from_slice(rom);
    }

    /// Call once per 60 Hz frame.
    pub fn tick_timers(&mut self) {
        self.dt = self.dt.saturating_sub(1);
        self.st = self.st.saturating_sub(1);
    }

    // Memory address I + offset, wrapped so it can never go out of bounds
    fn mem(&self, offset: usize) -> usize {
        (self.i as usize + offset) % RAM_SIZE
    }

    /// Execute one instruction.
    pub fn run(&mut self, key_press: &[bool; 16]) {
        if self.pc as usize + 1 >= RAM_SIZE {
            return;
        }

        let inst = ((self.ram[self.pc as usize] as u16) << 8) | self.ram[self.pc as usize + 1] as u16;

        let nnn = inst & 0x0FFF;
        let n = (inst & 0x000F) as usize;
        let x = ((inst & 0x0F00) >> 8) as usize;
        let y = ((inst & 0x00F0) >> 4) as usize;
        let kk = (inst & 0x00FF) as u8;

        let tup = (
            ((inst & 0xF000) >> 12) as u8,
            x as u8,
            y as u8,
            (inst & 0x000F) as u8,
        );

        let pc_change = match tup {
            (0x0, 0x0, 0xE, 0x0) => {
                // CLS
                self.display = [PX_OFF; WIDTH * HEIGHT];
                ProgramCounter::Next
            }
            (0x0, 0x0, 0xE, 0xE) => {
                // RET: CALL already pushed the address *after* the call
                match self.stack.pop() {
                    Some(addr) => ProgramCounter::Jump(addr),
                    None => ProgramCounter::Next,
                }
            }
            (0x1, _, _, _) => ProgramCounter::Jump(nnn),
            (0x2, _, _, _) => {
                if self.stack.len() < STACK_SIZE {
                    self.stack.push(self.pc + 2);
                    ProgramCounter::Jump(nnn)
                } else {
                    ProgramCounter::Next // stack overflow: ignore the call
                }
            }
            (0x3, _, _, _) => {
                if self.v[x] == kk { ProgramCounter::Skip } else { ProgramCounter::Next }
            }
            (0x4, _, _, _) => {
                if self.v[x] != kk { ProgramCounter::Skip } else { ProgramCounter::Next }
            }
            (0x5, _, _, 0x0) => {
                if self.v[x] == self.v[y] { ProgramCounter::Skip } else { ProgramCounter::Next }
            }
            (0x6, _, _, _) => {
                self.v[x] = kk;
                ProgramCounter::Next
            }
            (0x7, _, _, _) => {
                self.v[x] = self.v[x].wrapping_add(kk);
                ProgramCounter::Next
            }
            (0x8, _, _, 0x0) => {
                self.v[x] = self.v[y];
                ProgramCounter::Next
            }
            (0x8, _, _, 0x1) => {
                self.v[x] |= self.v[y];
                ProgramCounter::Next
            }
            (0x8, _, _, 0x2) => {
                self.v[x] &= self.v[y];
                ProgramCounter::Next
            }
            (0x8, _, _, 0x3) => {
                self.v[x] ^= self.v[y];
                ProgramCounter::Next
            }
            (0x8, _, _, 0x4) => {
                let (res, over) = self.v[x].overflowing_add(self.v[y]);
                self.v[x] = res;
                self.v[0xF] = over as u8; // flag written last
                ProgramCounter::Next
            }
            (0x8, _, _, 0x5) => {
                let (res, over) = self.v[x].overflowing_sub(self.v[y]);
                self.v[x] = res;
                self.v[0xF] = !over as u8;
                ProgramCounter::Next
            }
            (0x8, _, _, 0x6) => {
                let vx = self.v[x];
                self.v[x] = vx >> 1;
                self.v[0xF] = vx & 0x1;
                ProgramCounter::Next
            }
            (0x8, _, _, 0x7) => {
                let (res, over) = self.v[y].overflowing_sub(self.v[x]);
                self.v[x] = res;
                self.v[0xF] = !over as u8;
                ProgramCounter::Next
            }
            (0x8, _, _, 0xE) => {
                let vx = self.v[x];
                self.v[x] = vx << 1;
                self.v[0xF] = vx >> 7;
                ProgramCounter::Next
            }
            (0x9, _, _, 0x0) => {
                if self.v[x] != self.v[y] { ProgramCounter::Skip } else { ProgramCounter::Next }
            }
            (0xA, _, _, _) => {
                self.i = nnn;
                ProgramCounter::Next
            }
            (0xB, _, _, _) => ProgramCounter::Jump((nnn + self.v[0] as u16) & 0x0FFF),
            (0xC, _, _, _) => {
                self.v[x] = rand::random::<u8>() & kk;
                ProgramCounter::Next
            }
            (0xD, _, _, _) => {
                // DRW Vx, Vy, n: XOR sprite onto screen, wrapping at the edges
                let px = self.v[x] as usize;
                let py = self.v[y] as usize;
                let mut collision = 0;

                for row in 0..n {
                    let sprite_byte = self.ram[self.mem(row)];
                    for col in 0..8 {
                        if (sprite_byte >> (7 - col)) & 1 == 1 {
                            let cx = (px + col) % WIDTH;
                            let cy = (py + row) % HEIGHT;
                            let idx = cy * WIDTH + cx;
                            if self.display[idx] == PX_ON {
                                collision = 1;
                                self.display[idx] = PX_OFF;
                            } else {
                                self.display[idx] = PX_ON;
                            }
                        }
                    }
                }
                self.v[0xF] = collision;
                ProgramCounter::Next
            }
            (0xE, _, 0x9, 0xE) => {
                if key_press[(self.v[x] & 0xF) as usize] { ProgramCounter::Skip } else { ProgramCounter::Next }
            }
            (0xE, _, 0xA, 0x1) => {
                if !key_press[(self.v[x] & 0xF) as usize] { ProgramCounter::Skip } else { ProgramCounter::Next }
            }
            (0xF, _, 0x0, 0x7) => {
                self.v[x] = self.dt;
                ProgramCounter::Next
            }
            (0xF, _, 0x0, 0xA) => {
                // Wait for key: stay on this instruction until one is pressed
                match key_press.iter().position(|&p| p) {
                    Some(k) => {
                        self.v[x] = k as u8;
                        ProgramCounter::Next
                    }
                    None => ProgramCounter::Stay,
                }
            }
            (0xF, _, 0x1, 0x5) => {
                self.dt = self.v[x];
                ProgramCounter::Next
            }
            (0xF, _, 0x1, 0x8) => {
                self.st = self.v[x];
                ProgramCounter::Next
            }
            (0xF, _, 0x1, 0xE) => {
                self.i = self.i.wrapping_add(self.v[x] as u16);
                ProgramCounter::Next
            }
            (0xF, _, 0x2, 0x9) => {
                self.i = (self.v[x] & 0xF) as u16 * 5;
                ProgramCounter::Next
            }
            (0xF, _, 0x3, 0x3) => {
                let vx = self.v[x];
                let (a, b, c) = (self.mem(0), self.mem(1), self.mem(2));
                self.ram[a] = vx / 100;
                self.ram[b] = (vx % 100) / 10;
                self.ram[c] = vx % 10;
                ProgramCounter::Next
            }
            (0xF, _, 0x5, 0x5) => {
                for j in 0..=x {
                    let a = self.mem(j);
                    self.ram[a] = self.v[j];
                }
                ProgramCounter::Next
            }
            (0xF, _, 0x6, 0x5) => {
                for j in 0..=x {
                    self.v[j] = self.ram[self.mem(j)];
                }
                ProgramCounter::Next
            }
            _ => {
                println!("Warning: unrecognized instruction: {:04x}", inst);
                ProgramCounter::Next
            }
        };

        match pc_change {
            ProgramCounter::Next => self.pc += 2,
            ProgramCounter::Skip => self.pc += 4,
            ProgramCounter::Jump(addr) => self.pc = addr,
            ProgramCounter::Stay => {}
        }
    }
}