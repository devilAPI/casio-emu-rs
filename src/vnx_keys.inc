
/// ClassWiz (fx-570VN X / fx-580VN X) keypad, derived from
/// `user202729/CasioEmu stable/models/fx580vnx/model.lua`.
/// Codes there are `KO<<4 | KI`; this emulator stores KI/KO as active-high
/// bit masks, so `ki = 1 << (code & 0xF)` and `ko = 1 << (code >> 4)`.
///
/// `label` is the name printed on the fx-580VN X keycaps (the reference map),
/// used for on-screen display only. Labels are ASCII because the GUI draws
/// them with the 8x8 `font8x8` bitmap font; printed symbols are transliterated
/// where no ASCII spelling exists (e.g. `x10` for the x10^x key, `S<>D` for
/// S<->D, `Integ` for the integral key, `dms` for degrees/minutes/seconds).
pub const VNX_GRID_W: u16 = 41;
pub const VNX_GRID_H: u16 = 52;
pub const VNX_KEYS: &[CalcKey] = &[
    CalcKey { label: "7", ki: 0x04, ko: 0x01, x0:  0, y0: 28, x1:  6, y1: 32 },
    CalcKey { label: "8", ki: 0x04, ko: 0x02, x0:  7, y0: 28, x1: 14, y1: 32 },
    CalcKey { label: "9", ki: 0x04, ko: 0x04, x0: 15, y0: 28, x1: 21, y1: 32 },
    CalcKey { label: "DEL", ki: 0x04, ko: 0x08, x0: 22, y0: 28, x1: 29, y1: 32 },
    CalcKey { label: "AC", ki: 0x04, ko: 0x10, x0: 29, y0: 28, x1: 36, y1: 32 },
    CalcKey { label: "4", ki: 0x02, ko: 0x01, x0:  0, y0: 34, x1:  6, y1: 38 },
    CalcKey { label: "5", ki: 0x02, ko: 0x02, x0:  7, y0: 34, x1: 14, y1: 38 },
    CalcKey { label: "6", ki: 0x02, ko: 0x04, x0: 15, y0: 34, x1: 21, y1: 38 },
    CalcKey { label: "X", ki: 0x02, ko: 0x08, x0: 22, y0: 34, x1: 29, y1: 38 },
    CalcKey { label: "/", ki: 0x02, ko: 0x10, x0: 29, y0: 34, x1: 36, y1: 38 },
    CalcKey { label: "1", ki: 0x01, ko: 0x01, x0:  0, y0: 40, x1:  6, y1: 44 },
    CalcKey { label: "2", ki: 0x01, ko: 0x02, x0:  7, y0: 40, x1: 14, y1: 44 },
    CalcKey { label: "3", ki: 0x01, ko: 0x04, x0: 15, y0: 40, x1: 21, y1: 44 },
    CalcKey { label: "+", ki: 0x01, ko: 0x08, x0: 22, y0: 40, x1: 29, y1: 44 },
    CalcKey { label: "-", ki: 0x01, ko: 0x10, x0: 29, y0: 40, x1: 36, y1: 44 },
    CalcKey { label: "0", ki: 0x10, ko: 0x40, x0:  0, y0: 46, x1:  6, y1: 50 },
    CalcKey { label: ".", ki: 0x08, ko: 0x40, x0:  7, y0: 46, x1: 14, y1: 50 },
    CalcKey { label: "x10", ki: 0x04, ko: 0x40, x0: 15, y0: 46, x1: 21, y1: 50 },
    CalcKey { label: "Ans", ki: 0x02, ko: 0x40, x0: 22, y0: 46, x1: 29, y1: 50 },
    CalcKey { label: "=", ki: 0x01, ko: 0x40, x0: 29, y0: 46, x1: 36, y1: 50 },
    // Function block (fx-580VN X printing).
    CalcKey { label: "a/b", ki: 0x20, ko: 0x01, x0:  0, y0: 12, x1:  5, y1: 15 },
    CalcKey { label: "sqrt", ki: 0x20, ko: 0x02, x0:  6, y0: 12, x1: 11, y1: 15 },
    CalcKey { label: "x2", ki: 0x20, ko: 0x04, x0: 12, y0: 12, x1: 17, y1: 15 },
    CalcKey { label: "x^", ki: 0x20, ko: 0x08, x0: 18, y0: 12, x1: 23, y1: 15 },
    CalcKey { label: "log", ki: 0x20, ko: 0x10, x0: 24, y0: 12, x1: 29, y1: 15 },
    CalcKey { label: "ln", ki: 0x20, ko: 0x20, x0: 30, y0: 12, x1: 35, y1: 15 },
    CalcKey { label: "(-)", ki: 0x10, ko: 0x01, x0:  0, y0: 17, x1:  5, y1: 20 },
    CalcKey { label: "dms", ki: 0x10, ko: 0x02, x0:  6, y0: 17, x1: 11, y1: 20 },
    CalcKey { label: "x-1", ki: 0x10, ko: 0x04, x0: 12, y0: 17, x1: 17, y1: 20 },
    CalcKey { label: "sin", ki: 0x10, ko: 0x08, x0: 18, y0: 17, x1: 23, y1: 20 },
    CalcKey { label: "cos", ki: 0x10, ko: 0x10, x0: 24, y0: 17, x1: 29, y1: 20 },
    CalcKey { label: "tan", ki: 0x10, ko: 0x20, x0: 30, y0: 17, x1: 35, y1: 20 },
    CalcKey { label: "STO", ki: 0x08, ko: 0x01, x0:  0, y0: 22, x1:  5, y1: 25 },
    CalcKey { label: "ENG", ki: 0x08, ko: 0x02, x0:  6, y0: 22, x1: 11, y1: 25 },
    CalcKey { label: "(", ki: 0x08, ko: 0x04, x0: 12, y0: 22, x1: 17, y1: 25 },
    CalcKey { label: ")", ki: 0x08, ko: 0x08, x0: 18, y0: 22, x1: 23, y1: 25 },
    CalcKey { label: "S<>D", ki: 0x08, ko: 0x10, x0: 24, y0: 22, x1: 29, y1: 25 },
    CalcKey { label: "M+", ki: 0x08, ko: 0x20, x0: 30, y0: 22, x1: 35, y1: 25 },
    // Shift/function row above the numeric pad.
    CalcKey { label: "OPTN", ki: 0x40, ko: 0x01, x0:  0, y0:  7, x1:  5, y1: 10 },
    CalcKey { label: "CALC", ki: 0x40, ko: 0x02, x0:  6, y0:  7, x1: 11, y1: 10 },
    CalcKey { label: "Integ", ki: 0x40, ko: 0x10, x0: 24, y0:  7, x1: 29, y1: 10 },
    CalcKey { label: "x", ki: 0x40, ko: 0x20, x0: 30, y0:  7, x1: 35, y1: 10 },
    // Top row.
    CalcKey { label: "SHIFT", ki: 0x80, ko: 0x01, x0:  0, y0:  0, x1:  4, y1:  4 },
    CalcKey { label: "ALPHA", ki: 0x80, ko: 0x02, x0:  5, y0:  0, x1: 10, y1:  4 },
    CalcKey { label: "MENU", ki: 0x80, ko: 0x10, x0: 26, y0:  0, x1: 30, y1:  4 },
    CalcKey { label: "ON", ki: 0x00, ko: 0x00, x0: 31, y0:  0, x1: 36, y1:  4 },
    CalcKey { label: "LEFT", ki: 0x40, ko: 0x04, x0: 11, y0:  3, x1: 15, y1:  6 },
    CalcKey { label: "RIGHT", ki: 0x80, ko: 0x08, x0: 21, y0:  3, x1: 25, y1:  6 },
    CalcKey { label: "UP", ki: 0x80, ko: 0x04, x0: 15, y0:  0, x1: 21, y1:  4 },
    CalcKey { label: "DOWN", ki: 0x40, ko: 0x08, x0: 15, y0:  5, x1: 21, y1:  9 },
];
