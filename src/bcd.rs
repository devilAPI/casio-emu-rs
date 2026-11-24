//! ClassWiz II BCD coprocessor ("enhanced calculator").
//!
//! Four 12-byte packed-BCD registers (A..D at 0xF480/A0/C0/E0), a command
//! register (0xF400) that runs one digit-parallel operation, and a macro
//! sequencer (0xF404/0xF405) that runs the small built-in programs the
//! firmware uses for multiplication, division and shifts.  The firmware does
//! all of its arithmetic through this unit, so without it every result is 0.
//!
//! Port of `Peripheral/BCDCalc.cpp` from CasioEmuX (GPL-3.0).

const MUL_PGM: [u16; 32] = [
    0x21B7, 0x21B6, 0x211A, 0x211A, 0x21A1, 0x86D0, 0x1F91, 0x2F91, 0x2F91, 0x2F91, 0x2F91, 0x2F91,
    0x2F91, 0x2F91, 0x2F91, 0x2F91, 0x1F91, 0x1F91, 0x1F91, 0x1F91, 0x1F91, 0x1F91, 0x1F1D, 0x161D,
    0x212D, 0x1F19, 0x1619, 0x1719, 0x1819, 0x1919, 0x1A19, 0xE500,
];
const DIV_PGM: [u16; 32] = [
    0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0x7029, 0x7329,
    0x4929, 0x681D, 0x671D, 0x061D, 0x621D, 0x611D, 0x001D, 0x651D, 0x641D, 0x031D, 0x21B7, 0x21B6,
    0x2116, 0x2116, 0x21B1, 0x22A0, 0x2181, 0x0A80, 0xFC00, 0xFC00,
];
const DIVSN_PGM: [u16; 32] = [
    0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0xBC00, 0x7029, 0x7329,
    0x4929, 0x681D, 0x671D, 0x061D, 0x621D, 0x611D, 0x001D, 0x651D, 0x641D, 0x031D, 0x21B7, 0x21B6,
    0x2116, 0x2116, 0x21CD, 0x0A8C, 0x21C1, 0x0A80, 0xFC00, 0xFC00,
];
const SFT_PGM: [u16; 32] = [
    0xC180, 0xC284, 0xC488, 0xC88C, 0xC181, 0xC285, 0xC489, 0xC88D, 0xC182, 0xC286, 0xC48A, 0xC88E,
    0xC183, 0xC287, 0xC48B, 0xC88F, 0xC190, 0xC294, 0xC498, 0xC89C, 0xC191, 0xC295, 0xC499, 0xC89D,
    0xC192, 0xC296, 0xC49A, 0xC89E, 0xC193, 0xC297, 0xC49B, 0xC89F,
];
const PGM_ENTRY: [u8; 16] = [
    0x00, 0x00, 0x00, 0x05, 0x16, 0x1C, 0x16, 0x1C, 0x00, 0x04, 0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C,
];

fn program(index: u8) -> Option<&'static [u16; 32]> {
    match index {
        2 | 3 => Some(&MUL_PGM),
        4 | 5 => Some(&DIV_PGM),
        6 | 7 => Some(&DIVSN_PGM),
        8..=15 => Some(&SFT_PGM),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Bcd {
    pub reg: [[u8; 12]; 4],
    pub cmd: u8,
    pub con: u8,
    /// Latched macro count register (0xF404).
    pub mcn_reg: u8,
    pub llz: u8,
    pub mlz: u8,
    /// Remaining iterations of the running macro.
    mcn: u8,
    c_flag: bool,
    z_flag: bool,
    macro_running: bool,
    cmd_req: u8,
    mcr_req: u8,
    cmd_pend: bool,
    mcr_pend: bool,
    pgm: Option<&'static [u16; 32]>,
    pc: u8,
}

impl Default for Bcd {
    fn default() -> Self {
        Self::new()
    }
}

impl Bcd {
    pub fn new() -> Self {
        Bcd {
            reg: [[0; 12]; 4],
            cmd: 0,
            con: 6,
            mcn_reg: 0,
            llz: 0,
            mlz: 0,
            mcn: 0,
            c_flag: false,
            z_flag: false,
            macro_running: false,
            cmd_req: 0,
            mcr_req: 0,
            cmd_pend: false,
            mcr_pend: false,
            pgm: None,
            pc: 0,
        }
    }

    /// True while a command or macro start is queued behind a running macro;
    /// the CPU is held until it has been accepted.
    pub fn cpu_stalled(&self) -> bool {
        self.cmd_pend || self.mcr_pend
    }

    pub fn macro_running(&self) -> bool {
        self.macro_running
    }

    pub fn flags(&self) -> u8 {
        (if self.c_flag { 0x80 } else { 0 }) | (if self.z_flag { 0x40 } else { 0 })
    }

    pub fn set_flags(&mut self, v: u8) {
        self.c_flag = v & 0x80 != 0;
        self.z_flag = v & 0x40 != 0;
    }

    pub fn write_con(&mut self, v: u8) {
        self.con = (v & 0x0F).clamp(1, 6);
    }

    pub fn write_mcn(&mut self, v: u8) {
        self.mcn_reg = v & 0x1F;
    }

    pub fn write_cmd(&mut self, v: u8) {
        self.cmd_req = v;
        if !self.macro_running {
            self.run_command(v);
        } else {
            self.cmd_pend = true;
        }
    }

    pub fn write_mcr(&mut self, v: u8) {
        self.mcr_req = v;
        if !self.macro_running {
            self.start_macro(v);
        } else {
            self.mcr_pend = true;
        }
    }

    /// Advance the macro sequencer by one instruction (one HSCLK cycle).
    pub fn tick(&mut self) {
        if !self.macro_running {
            return;
        }
        let pgm = match self.pgm {
            Some(p) => p,
            None => {
                self.macro_running = false;
                return;
            }
        };
        let mut inst;
        let mut offset;
        let mut cond = 0u8;
        loop {
            inst = pgm[(self.pc & 0x1F) as usize];
            offset = ((inst >> 8) & 0x1F) as u8;
            let mut refetch = false;
            match inst >> 13 {
                0 => self.pc = offset,
                1 => self.pc = (self.pc.wrapping_add(offset)) & 0x1F,
                2 => {
                    self.pc = self.pc.wrapping_add(1);
                    cond = 1;
                }
                3 => {
                    self.pc = self.pc.wrapping_add(1);
                    cond = 2;
                }
                4 => self.pc = ((self.reg[0][0] & 0x0F).wrapping_add(offset)) & 0x1F,
                5 => {
                    self.reg[0][0] = (self.reg[0][0] & 0xF0) | (self.pc & 0x0F);
                    self.pc = offset;
                    self.mcn = self.mcn.wrapping_sub(1);
                    if self.mcn != 0 {
                        refetch = true;
                    } else {
                        self.macro_running = false;
                    }
                }
                6 => {
                    self.mcn = self.mcn.wrapping_sub(offset);
                    self.pc &= 0xFC;
                    if self.mcn & 0xF8 != 0 {
                        self.pc |= 3;
                    } else if self.mcn & 0x04 != 0 {
                        self.pc |= 2;
                    } else if self.mcn & 0x02 != 0 {
                        self.pc |= 1;
                    } else if self.mcn == 0 {
                        self.macro_running = false;
                    }
                }
                _ => {
                    self.pc = offset;
                    self.mcn = self.mcn.wrapping_sub(1);
                    if self.mcn != 0 {
                        refetch = true;
                    } else {
                        self.macro_running = false;
                    }
                }
            }
            if !refetch {
                break;
            }
        }

        self.run_command((inst & 0xFF) as u8);
        if cond != 0 && ((cond & 1 != 0) ^ self.c_flag) {
            self.pc = offset;
        }

        if !self.macro_running {
            if self.cmd_pend {
                self.cmd_pend = false;
                self.run_command(self.cmd_req);
            }
            if self.mcr_pend {
                self.mcr_pend = false;
                self.start_macro(self.mcr_req);
            }
        }
    }

    fn start_macro(&mut self, index: u8) {
        self.mcn = self.mcn_reg.wrapping_add(1);
        if index > 0x0F {
            self.pgm = None;
            self.pc = 0;
        } else {
            self.pgm = program(index);
            self.pc = PGM_ENTRY[index as usize];
            if index & 8 != 0 {
                if self.mcn & 0xF8 != 0 {
                    self.pc |= 3;
                } else if self.mcn & 0x04 != 0 {
                    self.pc |= 2;
                } else if self.mcn & 0x02 != 0 {
                    self.pc |= 1;
                }
            }
        }
        if self.pgm.is_some() {
            self.macro_running = true;
        }
    }

    pub fn run_command(&mut self, cmd: u8) {
        self.cmd = cmd;
        let src = ((cmd >> 2) & 3) as usize;
        let dst = (cmd & 3) as usize;
        let op = cmd >> 4;
        let arithmetic_mode = op & 0x08 == 0;
        let mut calc_ptr: usize = if op >= 8 || op == 1 || op == 2 {
            0
        } else if op == 7 {
            2
        } else {
            6
        };
        while calc_ptr < 6 {
            if calc_ptr == 0 {
                self.c_flag = false;
                self.z_flag = true;
            }
            let mut carry = if op == 2 { !self.c_flag } else { self.c_flag };
            let mut op_src = self.reg[src][calc_ptr << 1] as u16
                | ((self.reg[src][(calc_ptr << 1) + 1] as u16) << 8);
            let mut op_dst = self.reg[dst][calc_ptr << 1] as u16
                | ((self.reg[dst][(calc_ptr << 1) + 1] as u16) << 8);
            let mut res: u16 = 0;
            for i in 0..4 {
                let mut op1 = (op_src & 0x0F) as u8;
                let mut op2 = (op_dst & 0x0F) as u8;
                op_src >>= 4;
                op_dst >>= 4;
                if op == 2 {
                    op1 = 9u8.wrapping_sub(op1) & 0x0F;
                }
                op2 = op2.wrapping_add(op1).wrapping_add(carry as u8);
                carry = op2 >= 10;
                if carry {
                    op2 -= 10;
                }
                res |= (op2 as u16) << (i * 4);
            }
            if op == 2 {
                carry = !carry;
            }
            if res != 0 {
                self.z_flag = false;
            }
            if arithmetic_mode {
                self.reg[dst][calc_ptr << 1] = (res & 0xFF) as u8;
                self.reg[dst][(calc_ptr << 1) + 1] = (res >> 8) as u8;
                self.c_flag = carry;
                calc_ptr += 1;
                if op < 7 && calc_ptr >= self.con as usize {
                    break;
                }
            } else {
                if calc_ptr < self.con as usize {
                    self.c_flag = carry;
                }
                let first = calc_ptr == 0;
                calc_ptr += 1;
                if !first {
                    break;
                }
            }
        }

        if !arithmetic_mode {
            match op & 0x07 {
                0 => self.shift_left(src, dst, false),
                1 => self.shift_right(src, dst, false),
                2 => {
                    self.reg[dst] = [0; 12];
                    self.reg[dst][0] = if src == 3 { 5 } else { src as u8 };
                }
                3 => self.reg[dst] = self.reg[src],
                4 => self.shift_left(src, dst, true),
                5 => self.shift_right(src, dst, true),
                7 => self.reg[dst] = [0; 12],
                _ => {}
            }
        }

        let con = self.con as usize;
        self.llz = 0;
        for i in 0..12 {
            if i >= 2 * con {
                self.llz += 2;
                continue;
            }
            if self.reg[dst][i] & 0x0F != 0 {
                break;
            }
            self.llz += 1;
            if self.reg[dst][i] >> 4 != 0 {
                break;
            }
            self.llz += 1;
        }

        self.mlz = (24 - 4 * con) as u8;
        for i in (0..2 * con).rev() {
            if self.reg[dst][i] >> 4 != 0 {
                break;
            }
            self.mlz += 1;
            if self.reg[dst][i] & 0x0F != 0 {
                break;
            }
            self.mlz += 1;
        }
    }

    fn shift_left(&mut self, src: usize, dst: usize, continuous: bool) {
        if src == 0 {
            for i in (1..12).rev() {
                self.reg[dst][i] = (self.reg[dst][i] << 4) | (self.reg[dst][i - 1] >> 4);
            }
            let carry_in = if continuous {
                self.reg[(dst + 3) & 3][11] >> 4
            } else {
                0
            };
            self.reg[dst][0] = (self.reg[dst][0] << 4) | carry_in;
        } else {
            let size = 1usize << (src - 1);
            let fill: Vec<u8> = if continuous {
                self.reg[(dst + 3) & 3][12 - size..].to_vec()
            } else {
                vec![0; size]
            };
            self.reg[dst].copy_within(0..12 - size, size);
            self.reg[dst][..size].copy_from_slice(&fill);
        }
    }

    fn shift_right(&mut self, src: usize, dst: usize, continuous: bool) {
        if src == 0 {
            for i in 0..11 {
                self.reg[dst][i] = (self.reg[dst][i] >> 4) | (self.reg[dst][i + 1] << 4);
            }
            let carry_in = if continuous {
                self.reg[(dst + 1) & 3][0] << 4
            } else {
                0
            };
            self.reg[dst][11] = (self.reg[dst][11] >> 4) | carry_in;
        } else {
            let size = 1usize << (src - 1);
            let fill: Vec<u8> = if continuous {
                self.reg[(dst + 1) & 3][..size].to_vec()
            } else {
                vec![0; size]
            };
            self.reg[dst].copy_within(size..12, 0);
            self.reg[dst][12 - size..].copy_from_slice(&fill);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(b: &mut Bcd, r: usize, digits: &str) {
        // `digits` is most-significant first; nibble 0 of byte 0 is the units.
        let d: Vec<u8> = digits.bytes().rev().map(|c| c - b'0').collect();
        b.reg[r] = [0; 12];
        for (i, v) in d.iter().enumerate() {
            b.reg[r][i / 2] |= v << ((i % 2) * 4);
        }
    }

    fn read(b: &Bcd, r: usize) -> String {
        let mut s = String::new();
        for i in (0..24).rev() {
            s.push((b'0' + ((b.reg[r][i / 2] >> ((i % 2) * 4)) & 0x0F)) as char);
        }
        s.trim_start_matches('0').to_string()
    }

    #[test]
    fn add_command_adds_packed_bcd() {
        let mut b = Bcd::new();
        load(&mut b, 0, "99");
        load(&mut b, 1, "3");
        // op 1 = ADD, src = B(1), dst = A(0)
        b.write_cmd(0x14);
        assert_eq!(read(&b, 0), "102");
    }

    #[test]
    fn copy_and_clear_commands() {
        let mut b = Bcd::new();
        load(&mut b, 2, "12345");
        b.write_cmd(0xB0 | (2 << 2) | 1); // COPY C -> B
        assert_eq!(read(&b, 1), "12345");
        b.write_cmd(0xF0 | 1); // CLEAR B
        assert_eq!(read(&b, 1), "");
    }
}
