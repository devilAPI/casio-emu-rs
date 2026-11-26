mod gui;

use casio_emu_rs::cpu::{Bus, Cpu};
use casio_emu_rs::keys;
use casio_emu_rs::machine::Machine;
use casio_emu_rs::model::{self, Model};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut rom_path = PathBuf::from("fx991es.rom.bin");
    let mut rom_explicit = false;
    let mut ram_path: Option<PathBuf> = None;
    let mut model_id: Option<String> = None;
    let mut headless: Option<u64> = None;
    let mut dump_path: Option<PathBuf> = None;
    let mut key_script: Vec<String> = Vec::new();
    let mut trace_steps: Option<u64> = None;
    let mut trace_hash_steps: Option<u64> = None;
    let mut presses: Vec<(u64, String)> = Vec::new();
    let mut releases: Vec<u64> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rom" => {
                i += 1;
                rom_path = PathBuf::from(args.get(i).expect("--rom needs a path"));
                rom_explicit = true;
            }
            "--ram" => {
                i += 1;
                ram_path = Some(PathBuf::from(args.get(i).expect("--ram needs a path")));
            }
            "--model" => {
                i += 1;
                model_id = Some(args.get(i).expect("--model needs an id").to_string());
            }
            "--list-models" => {
                println!("Supported calculator models:");
                for m in model::ALL {
                    println!("  {:<16} {}", m.id, m.name);
                }
                return;
            }
            "--headless" => {
                i += 1;
                headless = Some(
                    args.get(i)
                        .expect("--headless needs a step count")
                        .parse()
                        .expect("invalid step count"),
                );
            }
            "--dump" => {
                i += 1;
                dump_path = Some(PathBuf::from(args.get(i).expect("--dump needs a path")));
            }
            "--press" => {
                i += 1;
                let spec = args.get(i).expect("--press needs STEP:KEY");
                let (at, label) = spec.split_once(':').expect("--press format is STEP:KEY");
                presses.push((at.parse().expect("invalid step"), label.to_string()));
            }
            "--trace" => {
                i += 1;
                trace_steps = Some(
                    args.get(i)
                        .expect("--trace needs a step count")
                        .parse()
                        .expect("invalid step count"),
                );
            }
            "--trace-hash" => {
                i += 1;
                trace_hash_steps = Some(
                    args.get(i)
                        .expect("--trace-hash needs a step count")
                        .parse()
                        .expect("invalid step count"),
                );
            }
            "--release" => {
                i += 1;
                releases.push(
                    args.get(i)
                        .expect("--release needs a step")
                        .parse()
                        .expect("invalid step"),
                );
            }
            "--keys" => {
                i += 1;
                key_script = args
                    .get(i)
                    .expect("--keys needs a comma separated list")
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                print_help();
                std::process::exit(2);
            }
        }
        i += 1;
    }

    // With an explicit model but no explicit ROM, use roms/<id>.rom.bin.
    // Otherwise fall back to the legacy default in the current directory.
    if !rom_explicit {
        if let Some(id) = model_id.as_deref() {
            let candidate = PathBuf::from("roms").join(
                model::by_id(id)
                    .map(|m| m.rom_filename())
                    .unwrap_or_else(|| format!("{id}.rom.bin")),
            );
            if candidate.exists() {
                rom_path = candidate;
            }
        } else if !Path::new("fx991es.rom.bin").exists() {
            let candidate = PathBuf::from("roms/fx991es-plus-c.rom.bin");
            if candidate.exists() {
                rom_path = candidate;
            }
        }
    }

    let rom = match fs::read(&rom_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("cannot read ROM '{}': {e}", rom_path.display());
            eprintln!();
            eprintln!("You need a ROM dump from your own licensed copy of the");
            eprintln!("CASIO fx-991ES PLUS C Emulator Ver. 4.00.  See tools/extract_rom.py");
            eprintln!("and the README for instructions.  Place the dump at fx991es.rom.bin");
            std::process::exit(1);
        }
    };

    // An explicit --model always wins; otherwise identify the ROM by the
    // self-test signature stored at 0x1FFF4.  Unknown images keep the original
    // fx-991ES PLUS C behaviour so existing setups continue to work.
    let model = if let Some(id) = model_id.as_deref() {
        match model::by_id(id) {
            Some(m) => m,
            None => {
                eprintln!("unknown model '{id}'. Run --list-models to see valid ids.");
                std::process::exit(2);
            }
        }
    } else {
        match model::detect(&rom) {
            Some(m) => {
                eprintln!("detected ROM model: {}", m.name);
                m
            }
            None => model::default(),
        }
    };

    if rom.len() < model.rom_size {
        eprintln!(
            "warning: ROM is {} bytes; {} expects {} bytes",
            rom.len(),
            model.name,
            model.rom_size
        );
    }

    if !model.execution_supported() {
        eprintln!(
            "note: {} is recognised and its ROM can be loaded, but this build does not emulate \
             its board; firmware execution is not supported.",
            model.name
        );
    }

    // Optional raw RAM snapshot (see Machine::load_ram).  An absent or
    // unreadable file leaves the machine's freshly zeroed RAM untouched.
    let ram = ram_path.as_deref().and_then(|path| match fs::read(path) {
        Ok(data) => Some(data),
        Err(e) => {
            eprintln!("cannot read RAM snapshot '{}': {e}", path.display());
            None
        }
    });
    let ram = ram.as_deref();

    if let Some(steps) = trace_hash_steps {
        run_trace_hash(rom, model, steps, ram);
    } else if let Some(steps) = trace_steps {
        run_trace(rom, model, steps, ram);
    } else if let Some(steps) = headless {
        run_headless(
            rom,
            model,
            steps,
            dump_path,
            &key_script,
            &presses,
            &releases,
            ram,
        );
    } else {
        gui::run_gui(rom, model, ram);
    }
}

fn run_trace(rom: Vec<u8>, model: Model, steps: u64, ram: Option<&[u8]>) {
    let mut machine = Machine::with_model(rom, model);
    if let Some(ram) = ram {
        machine.load_ram(ram);
    }
    let mut cpu = Cpu::new();
    cpu.reset(&mut machine);
    for i in 0..steps {
        let word = machine.read_code(cpu.regs.csr, cpu.regs.pc, 2) as u16;
        print!(
            "{} {:04x} {:04x} {:02x} {:02x}",
            i, cpu.regs.pc, word, cpu.regs.csr, cpu.regs.psw
        );
        for b in cpu.regs.gp.iter() {
            print!(" {:02x}", b);
        }
        println!(" {:04x} {:04x}", cpu.regs.sp, cpu.regs.ea);
        machine.tick(&mut cpu);
    }
}

fn run_trace_hash(rom: Vec<u8>, model: Model, steps: u64, ram: Option<&[u8]>) {
    let mut machine = Machine::with_model(rom, model);
    if let Some(ram) = ram {
        machine.load_ram(ram);
    }
    let mut cpu = Cpu::new();
    cpu.reset(&mut machine);
    let mut h = FNV_OFFSET;
    for i in 0..steps {
        if i % 1000 == 0 {
            hash_u64(&mut h, machine.checksum());
            println!("{} {:016x}", i, h);
        }
        let r = &cpu.regs;
        hash_u16(&mut h, r.pc);
        hash_u8(&mut h, r.csr);
        hash_u8(&mut h, r.lcsr);
        for b in r.ecsr {
            hash_u8(&mut h, b);
        }
        hash_u16(&mut h, r.lr);
        for v in r.elr {
            hash_u16(&mut h, v);
        }
        hash_u8(&mut h, r.psw);
        for b in r.epsw {
            hash_u8(&mut h, b);
        }
        hash_u16(&mut h, r.sp);
        hash_u16(&mut h, r.ea);
        hash_u8(&mut h, r.dsr);
        for b in r.gp {
            hash_u8(&mut h, b);
        }
        machine.tick(&mut cpu);
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn hash_u8(h: &mut u64, b: u8) {
    *h ^= b as u64;
    *h = h.wrapping_mul(FNV_PRIME);
}

fn hash_u16(h: &mut u64, v: u16) {
    hash_u8(h, (v & 0xff) as u8);
    hash_u8(h, (v >> 8) as u8);
}

fn hash_u64(h: &mut u64, v: u64) {
    for i in 0..8 {
        hash_u8(h, ((v >> (8 * i)) & 0xff) as u8);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_headless(
    rom: Vec<u8>,
    model: Model,
    steps: u64,
    dump_path: Option<PathBuf>,
    script: &[String],
    presses: &[(u64, String)],
    releases: &[u64],
    ram: Option<&[u8]>,
) {
    let mut machine = Machine::with_model(rom, model);
    if let Some(ram) = ram {
        machine.load_ram(ram);
    }
    let mut cpu = Cpu::new();
    cpu.reset(&mut machine);

    let key_at = |n: usize| -> Option<(u8, u8)> {
        script
            .get(n)
            .and_then(|s| keys::lookup(model.keys, s.as_str()))
    };

    let mut held = false;
    let mut key_index = 0usize;
    for step in 0..steps {
        for (at, label) in presses {
            if *at == step {
                if let Some((ki, ko)) = keys::lookup(model.keys, label) {
                    machine.press(ki, ko);
                }
            }
        }
        if releases.contains(&step) {
            machine.release();
        }
        if !script.is_empty() {
            let window = steps / (script.len() as u64 + 1);
            if window > 0 && step > 0 && step % window == 0 && key_index < script.len() {
                if held {
                    machine.release();
                    held = false;
                } else {
                    if let Some((ki, ko)) = key_at(key_index) {
                        machine.press(ki, ko);
                        held = true;
                    }
                    key_index += 1;
                }
            }
        }
        machine.tick(&mut cpu);
    }

    let lcd = machine.render();
    let (lw, lh) = (model.lcd_width, model.lcd_height);
    let mut text = String::new();
    for y in 0..lh {
        for x in 0..lw {
            text.push(if lcd[y * lw + x] != 0 { '#' } else { '.' });
        }
        text.push('\n');
    }
    println!(
        "pc={:04x} csr={:02x} psw={:02x} cycles={}",
        cpu.regs.pc, cpu.regs.csr, cpu.regs.psw, cpu.cycles
    );
    print!("{text}");
    if let Some(path) = dump_path {
        let mut pgm = format!("P1\n{} {}\n", lw, lh);
        for y in 0..lh {
            for x in 0..lw {
                pgm.push_str(if lcd[y * lw + x] != 0 { "1 " } else { "0 " });
            }
            pgm.push('\n');
        }
        if let Err(e) = fs::write(&path, pgm) {
            eprintln!("cannot write {}: {e}", path.display());
        }
    }
}

fn print_help() {
    println!("casio-emu-rs — Casio nX-U8/100 calculator emulator in Rust");
    println!();
    println!("USAGE:");
    println!(
        "    casio-emu-rs [--rom PATH] [--ram PATH] [--model ID] [--headless STEPS] [OPTIONS]"
    );
    println!();
    println!("OPTIONS:");
    println!("    --rom PATH       ROM dump (default: fx991es.rom.bin)");
    println!("    --ram PATH       raw RAM snapshot to load at startup (optional)");
    println!(
        "    --model ID       model: {}",
        model::ALL
            .iter()
            .map(|m| m.id)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("    --list-models    list supported models and exit");
    println!("    --trace-hash N   run N instructions and print a rolling state hash");
    println!("    --headless N     run N instructions and print the LCD, no window");
    println!("    --press N:KEY    press KEY at instruction N (repeatable)");
    println!("    --release N      release the held key at instruction N (repeatable)");
    println!("    --keys LIST      comma-separated keys to press in headless mode");
    println!("    --dump PATH      write the LCD as a PBM (P1) file");
    println!();
    println!("The ROM must be dumped from your own licensed copy of the");
    println!("CASIO fx-991ES PLUS C Emulator Ver. 4.00; see tools/extract_rom.py.");
}
