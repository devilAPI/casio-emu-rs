//! nX-U8/100 CPU core.
//!
//! This is a Rust port of the instruction semantics and decode table from the
//! GPL-3.0 `u8_emu` project by Fraser Price (Fraserbc) and contributors.
//! The port is intentionally table-driven so that it stays close to the
//! reference implementation and is easy to audit against the official
//! nX-U8/100 instruction manual.

use crate::table::TABLE;

/// Byte-wide access to the machine's memory spaces.
pub trait Bus {
    fn read_code(&mut self, csr: u8, offset: u16, size: usize) -> u64;
    #[allow(dead_code)]
    fn write_code(&mut self, csr: u8, offset: u16, size: usize, value: u64);
    fn read_data(&mut self, dsr: u8, offset: u16, size: usize) -> u64;
    fn write_data(&mut self, dsr: u8, offset: u16, size: usize, value: u64);
}

/// Architectural registers.
#[derive(Clone, Debug, Default)]
pub struct Regs {
    pub gp: [u8; 16],
    pub pc: u16,
    pub csr: u8,
    pub lcsr: u8,
    pub ecsr: [u8; 3],
    pub lr: u16,
    pub elr: [u16; 3],
    pub psw: u8,
    pub epsw: [u8; 3],
    pub sp: u16,
    pub ea: u16,
    pub dsr: u8,
}

#[derive(Clone, Debug)]
pub struct Cpu {
    pub regs: Regs,
    /// Data segment selected by a DSR prefix for the next instruction.
    pub cur_dsr: u8,
    pub cycles: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandKind {
    RegGp,
    RegCtrl,
    Mem,
    Imm,
}

#[derive(Clone, Copy, Debug)]
pub struct OperandSpec {
    pub mask: u16,
    pub shift: u8,
    pub flags: u16,
    pub kind: OperandKind,
}

impl OperandSpec {
    pub const fn none() -> Self {
        Self {
            mask: 0,
            shift: 0,
            flags: 0,
            kind: OperandKind::Imm,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstrSpec {
    pub handler: Handler,
    pub mask: u16,
    pub flags: u8,
    pub op0: OperandSpec,
    pub op1: OperandSpec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handler {
    Add,
    Addc,
    And,
    Cmp,
    Cmpc,
    Mov,
    Or,
    Xor,
    Sub,
    Subc,
    Shift,
    Dsr,
    Load,
    Store,
    Push,
    PushList,
    Pop,
    PopList,
    Lea,
    Daa,
    Das,
    Neg,
    Bit,
    Ei,
    Di,
    Sc,
    Rc,
    Cplc,
    Brel,
    Extbw,
    Swi,
    Brk,
    B,
    Bl,
    Mul,
    Div,
    Inc,
    Dec,
    Rt,
    Rti,
    Nop,
}

#[derive(Clone, Copy, Debug)]
pub enum Operand {
    RegGp { reg: u8, size: usize },
    RegCtrl { reg: u8, size: usize },
    Mem { addr: u16, size: usize },
    Imm { value: u64 },
}

const PSW_C: u8 = 0x80;
const PSW_Z: u8 = 0x40;
const PSW_S: u8 = 0x20;
const PSW_OV: u8 = 0x10;
const PSW_HC: u8 = 0x04;
const PSW_MIE: u8 = 0x08;
const PSW_EL: u8 = 0x03;

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Regs::default(),
            cur_dsr: 0,
            cycles: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut dyn Bus) {
        self.regs = Regs::default();
        self.cur_dsr = 0;
        self.cycles = 0;
        self.regs.sp = bus.read_data(0, 0x0000, 2) as u16;
        self.regs.pc = bus.read_data(0, 0x0002, 2) as u16;
    }

    pub fn fetch(&mut self, bus: &mut dyn Bus) -> u16 {
        let word = bus.read_code(self.regs.csr, self.regs.pc, 2) as u16;
        self.regs.pc = self.regs.pc.wrapping_add(2);
        word
    }

    pub fn step(&mut self, bus: &mut dyn Bus) {
        let word = self.fetch(bus);
        let Some(spec) = decode(word) else {
            // The reference core treats an unknown instruction as a halt.
            // Keep the PC at the faulting instruction so a debugger can see it.
            self.regs.pc = self.regs.pc.wrapping_sub(2);
            return;
        };

        let op0 = if spec.op0.kind != OperandKind::Imm || spec.op0.mask != 0 {
            let val = ((word & spec.op0.mask) >> spec.op0.shift) as u32;
            Some(self.resolve_operand(bus, spec.op0, val))
        } else {
            None
        };
        let op1 = if spec.op1.kind != OperandKind::Imm || spec.op1.mask != 0 {
            let val = ((word & spec.op1.mask) >> spec.op1.shift) as u32;
            Some(self.resolve_operand(bus, spec.op1, val))
        } else {
            None
        };

        (handler_for(spec.handler))(self, bus, spec.flags, op0, op1);
        self.cycles = self.cycles.wrapping_add(1);
    }

    fn resolve_operand(&mut self, bus: &mut dyn Bus, spec: OperandSpec, val: u32) -> Operand {
        match spec.kind {
            OperandKind::RegGp => Operand::RegGp {
                reg: (val.wrapping_sub((spec.flags >> 4) as u32) & 0x0f) as u8,
                size: (spec.flags & 0x0f) as usize,
            },
            OperandKind::RegCtrl => Operand::RegCtrl {
                reg: (spec.flags & 0x0f) as u8,
                size: (spec.flags >> 4) as usize,
            },
            OperandKind::Imm => {
                let mut v = val as u64;
                if spec.flags != 0 && ((val >> spec.flags) & 1) != 0 {
                    v |= u64::MAX << spec.flags;
                }
                Operand::Imm { value: v }
            }
            OperandKind::Mem => {
                let mut disp: u16 = match (spec.flags & 0x0f00) >> 8 {
                    0 => 0,
                    1 => self.fetch(bus),
                    2 => val as u16,
                    _ => 0,
                };
                let sext = ((spec.flags & 0x00f0) >> 4) as u32;
                if sext != 0 && ((val >> sext) & 1) != 0 {
                    disp |= u16::MAX << sext;
                }
                let reg_val = match spec.flags & 0x000f {
                    0 => 0u16,
                    1 => self.regs.ea,
                    2 => self.read_reg(val as u8, 2) as u16,
                    3 => self.read_reg(12, 2) as u16,
                    4 => self.read_reg(14, 2) as u16,
                    _ => 0,
                };
                let addr = reg_val.wrapping_add(disp);
                let size = (((spec.flags & 0x7000) >> 12) + 1) as usize;
                if spec.flags & 0x8000 != 0 {
                    self.regs.ea = self.regs.ea.wrapping_add(size as u16);
                }
                Operand::Mem { addr, size }
            }
        }
    }

    pub fn read_reg(&self, reg: u8, size: usize) -> u64 {
        let mut val = 0u64;
        for i in 0..size {
            val |= (self.regs.gp[((reg as usize) + i) & 0x0f] as u64) << (8 * i);
        }
        val
    }

    pub fn write_reg(&mut self, reg: u8, size: usize, val: u64) {
        for i in 0..size {
            self.regs.gp[((reg as usize) + i) & 0x0f] = ((val >> (8 * i)) & 0xff) as u8;
        }
    }

    fn read_operand(&mut self, bus: &mut dyn Bus, op: Operand) -> u64 {
        match op {
            Operand::RegGp { reg, size } => self.read_reg(reg, size),
            Operand::RegCtrl { reg, .. } => match reg {
                0 => self.regs.ecsr[(self.regs.psw & PSW_EL) as usize] as u64,
                1 => self.regs.elr[(self.regs.psw & PSW_EL) as usize] as u64,
                2 => self.regs.psw as u64,
                3 => self.regs.epsw[(self.regs.psw & PSW_EL) as usize] as u64,
                4 => self.regs.sp as u64,
                5 => self.regs.csr as u64,
                6 => self.regs.dsr as u64,
                _ => 0,
            },
            Operand::Mem { addr, size } => bus.read_data(self.cur_dsr, addr, size),
            Operand::Imm { value } => value,
        }
    }

    fn write_operand(&mut self, bus: &mut dyn Bus, op: Operand, val: u64) {
        match op {
            Operand::RegGp { reg, size } => self.write_reg(reg, size, val),
            Operand::RegCtrl { reg, .. } => match reg {
                0 => self.regs.ecsr[(self.regs.psw & PSW_EL) as usize] = val as u8,
                1 => self.regs.elr[(self.regs.psw & PSW_EL) as usize] = val as u16,
                2 => self.regs.psw = val as u8,
                3 => self.regs.epsw[(self.regs.psw & PSW_EL) as usize] = val as u8,
                4 => self.regs.sp = val as u16,
                5 => self.regs.csr = val as u8,
                _ => {}
            },
            Operand::Mem { addr, size } => bus.write_data(self.cur_dsr, addr, size, val),
            Operand::Imm { .. } => {}
        }
    }

    fn psw_set_zs(&mut self, size: usize, val: u64) {
        let mut psw = self.regs.psw & 0b1001_1111;
        let sign_bit = size * 8 - 1;
        psw |= ((val == 0) as u8) << 6;
        psw |= (((val >> sign_bit) & 1) as u8) << 5;
        self.regs.psw = psw;
    }

    fn psw_set_zs_on_size(&mut self, size: usize, val: u64) {
        self.psw_set_zs(size, val);
    }

    fn set_flags(&mut self, flags: u8) {
        self.regs.psw &= 0b0000_1011;
        self.regs.psw |= flags;
    }

    fn set_flags_keep_carry(&mut self, flags: u8) {
        self.regs.psw &= 0b1000_1011;
        self.regs.psw |= flags & 0b0111_0100;
    }
}

#[inline]
fn decode(word: u16) -> Option<&'static InstrSpec> {
    for spec in TABLE {
        let mut mask: u16 = 0;
        if !is_none(spec.op0) {
            mask |= spec.op0.mask;
        }
        if !is_none(spec.op1) {
            mask |= spec.op1.mask;
        }
        mask ^= 0xffff;
        if (word & mask) == spec.mask {
            return Some(spec);
        }
    }
    None
}

#[inline]
fn is_none(spec: OperandSpec) -> bool {
    spec.mask == 0 && spec.flags == 0 && spec.kind == OperandKind::Imm
}

fn handler_for(h: Handler) -> fn(&mut Cpu, &mut dyn Bus, u8, Option<Operand>, Option<Operand>) {
    match h {
        Handler::Add => h_add,
        Handler::Addc => h_addc,
        Handler::And => h_and,
        Handler::Cmp => h_cmp,
        Handler::Cmpc => h_cmpc,
        Handler::Mov => h_mov,
        Handler::Or => h_or,
        Handler::Xor => h_xor,
        Handler::Sub => h_sub,
        Handler::Subc => h_subc,
        Handler::Shift => h_shift,
        Handler::Dsr => h_dsr,
        Handler::Load => h_load,
        Handler::Store => h_store,
        Handler::Push => h_push,
        Handler::PushList => h_push_list,
        Handler::Pop => h_pop,
        Handler::PopList => h_pop_list,
        Handler::Lea => h_lea,
        Handler::Daa => h_daa,
        Handler::Das => h_das,
        Handler::Neg => h_neg,
        Handler::Bit => h_bit,
        Handler::Ei => h_ei,
        Handler::Di => h_di,
        Handler::Sc => h_sc,
        Handler::Rc => h_rc,
        Handler::Cplc => h_cplc,
        Handler::Brel => h_brel,
        Handler::Extbw => h_extbw,
        Handler::Swi => h_swi,
        Handler::Brk => h_brk,
        Handler::B => h_b,
        Handler::Bl => h_bl,
        Handler::Mul => h_mul,
        Handler::Div => h_div,
        Handler::Inc => h_inc,
        Handler::Dec => h_dec,
        Handler::Rt => h_rt,
        Handler::Rti => h_rti,
        Handler::Nop => h_nop,
    }
}

// ---------------------------------------------------------------------------
// ALU helpers
// ---------------------------------------------------------------------------

fn add_impl(val0: u64, val1: u64, size: usize) -> (u64, u8) {
    let bits = size * 8;
    let full = (val0 as u128) + (val1 as u128);
    let carry = ((full >> bits) & 1) as u8;
    let result = (full as u64) & mask_for(size);
    let zero = (result == 0) as u8;
    let s_bit = bits - 1;
    let sign = ((result >> s_bit) & 1) as u8;
    let ov = if ((val0 >> s_bit) & 1) == ((val1 >> s_bit) & 1) {
        ((((val0 >> s_bit) & 1) != ((result >> s_bit) & 1)) as u8) & 1
    } else {
        0
    };
    let hc_bit = bits - 4;
    let hc_mask = (1u64 << hc_bit) - 1;
    let half = (val0 & hc_mask) + (val1 & hc_mask);
    let hc = ((half >> hc_bit) & 1) as u8;
    (
        result,
        (carry << 7) | (zero << 6) | (sign << 5) | (ov << 4) | (hc << 2),
    )
}

fn sub_impl(val0: u64, val1: u64, size: usize) -> (u64, u8) {
    let bits = size * 8;
    let m = 1u64 << (bits - 1);
    let v0 = (val0 ^ m).wrapping_sub(m);
    let v1 = (val1 ^ m).wrapping_sub(m);
    let full = v0.wrapping_sub(v1);
    let mut sign_nt = full >> bits;
    let result = full & mask_for(size);
    let zero = (result == 0) as u8;
    let sign = ((result >> (bits - 1)) & 1) as u8;
    let borrow = (v1 > v0) as u8;
    sign_nt = sign_nt.wrapping_add(sign as u64);
    sign_nt &= u64::MAX >> bits;
    let ov = (sign_nt != 0) as u8;
    let hc_bit = bits - 4;
    let hc_mask = (1u64 << hc_bit) - 1;
    let hc = ((v1 & hc_mask) > (v0 & hc_mask)) as u8;
    (
        result,
        (borrow << 7) | (zero << 6) | (sign << 5) | (ov << 4) | (hc << 2),
    )
}

#[inline]
fn mask_for(size: usize) -> u64 {
    if size >= 8 {
        u64::MAX
    } else {
        (1u64 << (size * 8)) - 1
    }
}

fn op0(op: Option<Operand>) -> Operand {
    op.expect("instruction requires first operand")
}

fn op1(op: Option<Operand>) -> Operand {
    op.expect("instruction requires second operand")
}

// ---------------------------------------------------------------------------
// Instruction handlers
// ---------------------------------------------------------------------------

fn h_add(cpu: &mut Cpu, bus: &mut dyn Bus, flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mask = mask_for(size);
    let v0 = cpu.read_operand(bus, o0) & mask;
    let v1 = cpu.read_operand(bus, o1) & mask;
    let (res, new_flags) = add_impl(v0, v1, size);
    if flags == 0 {
        cpu.set_flags(new_flags);
    }
    cpu.write_operand(bus, o0, res);
}

fn h_addc(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mask = mask_for(size);
    let v0 = cpu.read_operand(bus, o0) & mask;
    let v1 = cpu.read_operand(bus, o1) & mask;
    let carry = (cpu.regs.psw >> 7) as u64;
    let (r0, p0) = add_impl(v0, v1, size);
    let (res, p1) = add_impl(r0, carry, size);
    let psw = (p0 & 0b1011_1111) | p1;
    cpu.regs.psw &= (psw | 0b1011_1111) & 0b0100_1011;
    cpu.regs.psw |= psw & 0b1011_1111;
    cpu.write_operand(bus, o0, res);
}

fn h_cmp(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mask = mask_for(size);
    let v0 = cpu.read_operand(bus, o0) & mask;
    let v1 = cpu.read_operand(bus, o1) & mask;
    let (_, flags) = sub_impl(v0, v1, size);
    cpu.set_flags(flags);
}

fn h_cmpc(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mask = mask_for(size);
    let v0 = cpu.read_operand(bus, o0) & mask;
    let v1 = cpu.read_operand(bus, o1) & mask;
    let carry = (cpu.regs.psw >> 7) as u64;
    let (r0, p0) = sub_impl(v0, v1, size);
    let (_, p1) = sub_impl(r0, carry, size);
    let psw = (p0 & 0b1011_1111) | p1;
    cpu.regs.psw &= (psw | 0b1011_1111) & 0b0100_1011;
    cpu.regs.psw |= psw & 0b1011_1111;
}

fn h_sub(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mask = mask_for(size);
    let v0 = cpu.read_operand(bus, o0) & mask;
    let v1 = cpu.read_operand(bus, o1) & mask;
    let (res, flags) = sub_impl(v0, v1, size);
    cpu.set_flags(flags);
    cpu.write_operand(bus, o0, res);
}

fn h_subc(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mask = mask_for(size);
    let v0 = cpu.read_operand(bus, o0) & mask;
    let v1 = cpu.read_operand(bus, o1) & mask;
    let carry = (cpu.regs.psw >> 7) as u64;
    let (r0, p0) = sub_impl(v0, v1, size);
    let (res, p1) = sub_impl(r0, carry, size);
    let psw = (p0 & 0b1011_1111) | p1;
    cpu.regs.psw &= (psw | 0b1011_1111) & 0b0100_1011;
    cpu.regs.psw |= psw & 0b1011_1111;
    cpu.write_operand(bus, o0, res);
}

fn h_mov(cpu: &mut Cpu, bus: &mut dyn Bus, flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let val = cpu.read_operand(bus, o1);
    if flags == 0 {
        let size = operand_size(&o0);
        cpu.psw_set_zs(size, val);
    }
    cpu.write_operand(bus, o0, val);
}

fn h_or(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let v0 = cpu.read_operand(bus, o0);
    let v1 = cpu.read_operand(bus, o1);
    let res = v0 | v1;
    let size = operand_size(&o0);
    cpu.psw_set_zs(size, res);
    cpu.write_operand(bus, o0, res);
}

fn h_and(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let v0 = cpu.read_operand(bus, o0);
    let v1 = cpu.read_operand(bus, o1);
    let res = v0 & v1;
    let size = operand_size(&o0);
    cpu.psw_set_zs(size, res);
    cpu.write_operand(bus, o0, res);
}

fn h_xor(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let v0 = cpu.read_operand(bus, o0);
    let v1 = cpu.read_operand(bus, o1);
    let res = v0 ^ v1;
    let size = operand_size(&o0);
    cpu.psw_set_zs(size, res);
    cpu.write_operand(bus, o0, res);
}

fn h_shift(cpu: &mut Cpu, bus: &mut dyn Bus, flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mut val = cpu.read_operand(bus, o0);
    let shift = (cpu.read_operand(bus, o1) & 0b111) as u32;

    let bits = size * 8;
    let mut keep_mask = if size > 1 {
        (1u64 << ((size - 1) * 8)) - 1
    } else {
        0
    };
    if flags & 1 != 0 {
        keep_mask = !keep_mask;
    }
    let keep = val & keep_mask;

    if shift == 0 {
        // The reference implementation leaves the value untouched and clears
        // the carry for a zero-width shift.
        cpu.regs.psw &= 0x7f;
        cpu.write_operand(bus, o0, val);
        return;
    }

    if flags & 4 != 0 {
        let m = 1u64 << (bits - 1);
        val = (val ^ m).wrapping_sub(m);
    }

    let mut psw = cpu.regs.psw & 0b0111_1111;
    if flags & 1 != 0 {
        psw |= (((val >> (shift - 1)) & 1) as u8) << 7;
        val >>= shift;
    } else {
        psw |= (((val >> (bits - shift as usize)) & 1) as u8) << 7;
        val <<= shift;
    }
    cpu.regs.psw = psw;

    if flags & 2 != 0 {
        val &= !keep_mask;
        val |= keep;
    }
    cpu.write_operand(bus, o0, val);
}

fn h_dsr(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    cpu.regs.dsr = cpu.read_operand(bus, o0) as u8;
    cpu.cur_dsr = cpu.regs.dsr;
    cpu.step(bus);
    cpu.cur_dsr = 0;
}

fn h_load(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let val = cpu.read_operand(bus, o1);
    let size = operand_size(&o0);
    cpu.psw_set_zs(size, val);
    cpu.write_operand(bus, o0, val);
}

fn h_store(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let val = cpu.read_operand(bus, o0);
    cpu.write_operand(bus, o1, val);
}

fn h_push(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    let size = operand_size(&o0);
    let step = if size == 1 { 2 } else { size };
    cpu.regs.sp = cpu.regs.sp.wrapping_sub(step as u16);
    let val = cpu.read_operand(bus, o0);
    bus.write_data(cpu.cur_dsr, cpu.regs.sp, size, val);
}

fn h_push_list(
    cpu: &mut Cpu,
    bus: &mut dyn Bus,
    _flags: u8,
    o0: Option<Operand>,
    _o1: Option<Operand>,
) {
    let list = cpu.read_operand(bus, op0(o0)) as u8;
    let el = (cpu.regs.psw & PSW_EL) as usize;
    if list & 0x2 != 0 {
        cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
        bus.write_data(cpu.cur_dsr, cpu.regs.sp, 1, cpu.regs.ecsr[el] as u64);
        cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
        bus.write_data(cpu.cur_dsr, cpu.regs.sp, 2, cpu.regs.elr[el] as u64);
    }
    if list & 0x4 != 0 {
        cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
        bus.write_data(cpu.cur_dsr, cpu.regs.sp, 1, cpu.regs.epsw[el] as u64);
    }
    if list & 0x8 != 0 {
        cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
        bus.write_data(cpu.cur_dsr, cpu.regs.sp, 1, cpu.regs.lcsr as u64);
        cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
        bus.write_data(cpu.cur_dsr, cpu.regs.sp, 2, cpu.regs.lr as u64);
    }
    if list & 0x1 != 0 {
        cpu.regs.sp = cpu.regs.sp.wrapping_sub(2);
        bus.write_data(cpu.cur_dsr, cpu.regs.sp, 2, cpu.regs.ea as u64);
    }
}

fn h_pop(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    let size = operand_size(&o0);
    let val = bus.read_data(cpu.cur_dsr, cpu.regs.sp, size);
    cpu.write_operand(bus, o0, val);
    let step = if size == 1 { 2 } else { size };
    cpu.regs.sp = cpu.regs.sp.wrapping_add(step as u16);
}

fn h_pop_list(
    cpu: &mut Cpu,
    bus: &mut dyn Bus,
    _flags: u8,
    o0: Option<Operand>,
    _o1: Option<Operand>,
) {
    let list = cpu.read_operand(bus, op0(o0)) as u8;
    if list & 0x1 != 0 {
        cpu.regs.ea = bus.read_data(cpu.cur_dsr, cpu.regs.sp, 2) as u16;
        cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
    }
    if list & 0x8 != 0 {
        cpu.regs.lr = bus.read_data(cpu.cur_dsr, cpu.regs.sp, 2) as u16;
        cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
        cpu.regs.lcsr = bus.read_data(cpu.cur_dsr, cpu.regs.sp, 1) as u8;
        cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
    }
    if list & 0x4 != 0 {
        cpu.regs.psw = bus.read_data(cpu.cur_dsr, cpu.regs.sp, 1) as u8;
        cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
    }
    if list & 0x2 != 0 {
        cpu.regs.pc = bus.read_data(cpu.cur_dsr, cpu.regs.sp, 2) as u16;
        cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
        cpu.regs.csr = bus.read_data(cpu.cur_dsr, cpu.regs.sp, 1) as u8;
        cpu.regs.sp = cpu.regs.sp.wrapping_add(2);
    }
}

fn h_lea(cpu: &mut Cpu, _bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    if let Operand::Mem { addr, .. } = op0(o0) {
        cpu.regs.ea = addr;
    }
}

fn h_daa(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    let mut val = cpu.read_operand(bus, o0);
    let c = (cpu.regs.psw & PSW_C) != 0;
    let hc = (cpu.regs.psw & PSW_HC) != 0;
    let mut psw = cpu.regs.psw & 0b1001_1011;
    if hc || (val & 0x0f) > 0x09 {
        val = val.wrapping_add(0x06);
        psw |= 1 << 2;
    }
    if c || (val & 0xf0) > 0x90 || (val & 0x100) != 0 {
        val = val.wrapping_add(0x60);
        psw |= 1 << 7;
    }
    val &= 0xff;
    psw |= ((val == 0) as u8) << 6;
    psw |= ((val >> 7) as u8) << 5;
    cpu.regs.psw = psw;
    cpu.write_operand(bus, o0, val);
}

fn h_das(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    let mut val = cpu.read_operand(bus, o0);
    let c = (cpu.regs.psw & PSW_C) != 0;
    let hc = (cpu.regs.psw & PSW_HC) != 0;
    let mut psw = cpu.regs.psw & 0b1001_1011;
    if hc || (val & 0x0f) > 0x09 {
        val = val.wrapping_sub(0x06);
        psw |= 1 << 2;
    }
    if c || (val & 0xf0) > 0x90 || (val & 0x100) != 0 {
        val = val.wrapping_sub(0x60);
        psw |= 1 << 7;
    }
    val &= 0xff;
    psw |= ((val == 0) as u8) << 6;
    psw |= ((val >> 7) as u8) << 5;
    cpu.regs.psw = psw;
    cpu.write_operand(bus, o0, val);
}

fn h_neg(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    let size = operand_size(&o0);
    let val = cpu.read_operand(bus, o0) & mask_for(size);
    let (res, flags) = sub_impl(0, val, size);
    cpu.set_flags(flags);
    cpu.write_operand(bus, o0, res);
}

fn h_bit(cpu: &mut Cpu, bus: &mut dyn Bus, flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let bit_idx = cpu.read_operand(bus, op1(o1)) & 0xff;
    let mut val = cpu.read_operand(bus, o0);
    let bit = (val >> bit_idx) & 1;
    cpu.regs.psw &= 0b1011_1111;
    let inv = !(bit as u8);
    cpu.regs.psw |= inv.wrapping_shl(6);
    match flags {
        1 => val |= 1u64 << bit_idx,
        2 => val &= !(1u64 << bit_idx),
        _ => return,
    }
    cpu.write_operand(bus, o0, val);
}

fn h_ei(cpu: &mut Cpu, _bus: &mut dyn Bus, _flags: u8, _o0: Option<Operand>, _o1: Option<Operand>) {
    cpu.regs.psw |= PSW_MIE;
}

fn h_di(cpu: &mut Cpu, _bus: &mut dyn Bus, _flags: u8, _o0: Option<Operand>, _o1: Option<Operand>) {
    cpu.regs.psw &= !PSW_MIE;
}

fn h_sc(cpu: &mut Cpu, _bus: &mut dyn Bus, _flags: u8, _o0: Option<Operand>, _o1: Option<Operand>) {
    cpu.regs.psw |= PSW_C;
}

fn h_rc(cpu: &mut Cpu, _bus: &mut dyn Bus, _flags: u8, _o0: Option<Operand>, _o1: Option<Operand>) {
    cpu.regs.psw &= !PSW_C;
}

fn h_cplc(
    cpu: &mut Cpu,
    _bus: &mut dyn Bus,
    _flags: u8,
    _o0: Option<Operand>,
    _o1: Option<Operand>,
) {
    cpu.regs.psw ^= PSW_C;
}

fn h_brel(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let cond = cpu.read_operand(bus, op0(o0)) as u8;
    let psw = cpu.regs.psw;
    let c = (psw & PSW_C) != 0;
    let z = (psw & PSW_Z) != 0;
    let s = (psw & PSW_S) != 0;
    let ov = (psw & PSW_OV) != 0;
    let take = match cond & 0x0f {
        0x0 => !c,
        0x1 => c,
        0x2 => !c && !z,
        0x3 => c || z,
        0x4 => !(ov ^ s),
        0x5 => ov ^ s,
        0x6 => !((ov ^ s) || z),
        0x7 => (ov ^ s) || z,
        0x8 => !z,
        0x9 => z,
        0xa => !ov,
        0xb => ov,
        0xc => !s,
        0xd => s,
        0xe => true,
        _ => false,
    };
    if take {
        let offset = cpu.read_operand(bus, op1(o1)) as i64;
        cpu.regs.pc = cpu.regs.pc.wrapping_add((offset << 1) as u16);
    }
}

fn h_extbw(
    cpu: &mut Cpu,
    bus: &mut dyn Bus,
    _flags: u8,
    _o0: Option<Operand>,
    o1: Option<Operand>,
) {
    let o1 = op1(o1);
    let size = operand_size(&o1);
    let bits = size * 4;
    let mut val = cpu.read_operand(bus, o1) & ((1u64 << bits) - 1);
    let m = 1u64 << (bits - 1);
    val = (val ^ m).wrapping_sub(m);
    cpu.psw_set_zs_on_size(size, val);
    cpu.write_operand(bus, o1, val);
}

fn h_swi(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, _o0: Option<Operand>, _o1: Option<Operand>) {
    // The reference core does not implement SWI either. Treat it as a
    // software breakpoint into the same vector as BRK, which is what the
    // hardware does for the debug interrupt.
    h_brk(cpu, bus, 0, None, None);
}

fn h_brk(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, _o0: Option<Operand>, _o1: Option<Operand>) {
    let el = (cpu.regs.psw & PSW_EL) as usize;
    if el <= 1 {
        cpu.regs.elr[2] = cpu.regs.pc;
        cpu.regs.ecsr[2] = cpu.regs.csr;
        cpu.regs.epsw[2] = cpu.regs.psw;
        cpu.regs.psw &= 0b1111_1100;
        cpu.regs.psw |= 0b0000_0010;
        cpu.regs.pc = bus.read_data(0, 0x0004, 2) as u16;
        cpu.regs.csr = 0;
    } else {
        cpu.reset(bus);
    }
}

fn h_b(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let csr = cpu.read_operand(bus, op0(o0)) as u8;
    let addr = if let Operand::Mem { addr, .. } = op1(o1) {
        addr
    } else {
        0
    };
    cpu.regs.csr = csr;
    cpu.regs.pc = addr;
}

fn h_bl(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    cpu.regs.lcsr = cpu.regs.csr;
    cpu.regs.lr = cpu.regs.pc;
    let csr = cpu.read_operand(bus, op0(o0)) as u8;
    let addr = if let Operand::Mem { addr, .. } = op1(o1) {
        addr
    } else {
        0
    };
    cpu.regs.csr = csr;
    cpu.regs.pc = addr;
}

fn h_mul(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let size = operand_size(&o0);
    let mask = (1u64 << (4 * size)) - 1;
    let v0 = cpu.read_operand(bus, o0) & mask;
    let v1 = cpu.read_operand(bus, o1) & mask;
    let res = (v0 * v1) & mask_for(size);
    cpu.regs.psw &= !PSW_Z;
    if res == 0 {
        cpu.regs.psw |= PSW_Z;
    }
    cpu.write_operand(bus, o0, res);
}

fn h_div(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, o1: Option<Operand>) {
    let o0 = op0(o0);
    let o1 = op1(o1);
    let v0 = cpu.read_operand(bus, o0);
    let v1 = cpu.read_operand(bus, o1);
    cpu.regs.psw &= 0b0011_1111;
    if v1 == 0 {
        cpu.regs.psw |= PSW_C;
        cpu.write_operand(bus, o0, 0);
        return;
    }
    let res = v0 / v1;
    if res == 0 {
        cpu.regs.psw |= PSW_Z;
    }
    cpu.write_operand(bus, o0, res);
    cpu.write_operand(bus, o1, v0 % v1);
}

fn h_inc(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    let val = cpu.read_operand(bus, o0);
    let (res, flags) = add_impl(val, 1, 1);
    cpu.set_flags_keep_carry(flags);
    cpu.write_operand(bus, o0, res);
}

fn h_dec(cpu: &mut Cpu, bus: &mut dyn Bus, _flags: u8, o0: Option<Operand>, _o1: Option<Operand>) {
    let o0 = op0(o0);
    let val = cpu.read_operand(bus, o0);
    let (res, flags) = sub_impl(val, 1, 1);
    cpu.set_flags_keep_carry(flags);
    cpu.write_operand(bus, o0, res);
}

fn h_rt(cpu: &mut Cpu, _bus: &mut dyn Bus, _flags: u8, _o0: Option<Operand>, _o1: Option<Operand>) {
    cpu.regs.csr = cpu.regs.lcsr;
    cpu.regs.pc = cpu.regs.lr;
}

fn h_rti(
    cpu: &mut Cpu,
    _bus: &mut dyn Bus,
    _flags: u8,
    _o0: Option<Operand>,
    _o1: Option<Operand>,
) {
    let el = (cpu.regs.psw & PSW_EL) as usize;
    cpu.regs.csr = cpu.regs.ecsr[el];
    cpu.regs.pc = cpu.regs.elr[el];
    cpu.regs.psw = cpu.regs.epsw[el];
}

fn h_nop(
    _cpu: &mut Cpu,
    _bus: &mut dyn Bus,
    _flags: u8,
    _o0: Option<Operand>,
    _o1: Option<Operand>,
) {
}

fn operand_size(op: &Operand) -> usize {
    match op {
        Operand::RegGp { size, .. } | Operand::RegCtrl { size, .. } | Operand::Mem { size, .. } => {
            *size
        }
        Operand::Imm { .. } => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBus {
        code: Vec<u8>,
        data: Vec<u8>,
    }

    impl TestBus {
        fn new(words: &[u16]) -> Self {
            let mut code = Vec::new();
            for w in words {
                code.extend_from_slice(&w.to_le_bytes());
            }
            code.resize(0x20000, 0);
            Self {
                code,
                data: vec![0; 0x10000],
            }
        }
    }

    impl Bus for TestBus {
        fn read_code(&mut self, csr: u8, offset: u16, size: usize) -> u64 {
            let base = ((csr as usize) << 16) | offset as usize;
            let mut v = 0;
            for i in 0..size {
                v |= (self.code.get(base + i).copied().unwrap_or(0) as u64) << (8 * i);
            }
            v
        }
        fn write_code(&mut self, _csr: u8, _offset: u16, _size: usize, _value: u64) {}
        fn read_data(&mut self, dsr: u8, offset: u16, size: usize) -> u64 {
            let base = ((dsr as usize) << 16) | offset as usize;
            let mut v = 0;
            for i in 0..size {
                v |= (self.data.get(base + i).copied().unwrap_or(0) as u64) << (8 * i);
            }
            v
        }
        fn write_data(&mut self, dsr: u8, offset: u16, size: usize, value: u64) {
            let base = ((dsr as usize) << 16) | offset as usize;
            for i in 0..size {
                if base + i < self.data.len() {
                    self.data[base + i] = (value >> (8 * i)) as u8;
                }
            }
        }
    }

    fn run(words: &[u16]) -> (Cpu, TestBus) {
        let mut bus = TestBus::new(words);
        let mut cpu = Cpu::new();
        cpu.regs.pc = 0;
        cpu.regs.csr = 0;
        cpu.step(&mut bus);
        (cpu, bus)
    }

    #[test]
    fn mov_immediate() {
        let (cpu, _) = run(&[0x0012]);
        assert_eq!(cpu.regs.gp[0], 0x12);
        assert_eq!(cpu.regs.pc, 2);
    }

    #[test]
    fn add_sets_carry_and_zero() {
        let mut bus = TestBus::new(&[0x1001]); // ADD R0, #1
        let mut cpu = Cpu::new();
        cpu.regs.gp[0] = 0xff;
        cpu.step(&mut bus);
        assert_eq!(cpu.regs.gp[0], 0x00);
        assert_eq!(cpu.regs.psw & PSW_C, PSW_C);
        assert_eq!(cpu.regs.psw & PSW_Z, PSW_Z);
    }

    #[test]
    fn sub_sets_borrow_and_sign() {
        let mut bus = TestBus::new(&[0x8018]); // SUB R0, R1
        let mut cpu = Cpu::new();
        cpu.regs.gp[0] = 1;
        cpu.regs.gp[1] = 2;
        cpu.step(&mut bus);
        assert_eq!(cpu.regs.gp[0], 0xff);
        assert_eq!(cpu.regs.psw & PSW_C, PSW_C);
        assert_eq!(cpu.regs.psw & PSW_S, PSW_S);
    }

    #[test]
    fn zero_width_shift_is_noop() {
        let mut bus = TestBus::new(&[0x801c]); // SRL R0, R1
        let mut cpu = Cpu::new();
        cpu.regs.gp[0] = 0x0b;
        cpu.regs.gp[1] = 0;
        cpu.step(&mut bus);
        assert_eq!(cpu.regs.gp[0], 0x0b);
        assert_eq!(cpu.regs.psw & PSW_C, 0);
    }

    #[test]
    fn tb_sets_carry_and_zero_like_reference() {
        let mut bus = TestBus::new(&[0xa201]); // TB R2.0
        let mut cpu = Cpu::new();
        cpu.regs.gp[2] = 0x00;
        cpu.step(&mut bus);
        assert_eq!(cpu.regs.psw & PSW_C, PSW_C);
        assert_eq!(cpu.regs.psw & PSW_Z, PSW_Z);
    }
}
