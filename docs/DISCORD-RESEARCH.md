# Research from the "Casio Calculator Hacking" Discord

Notes gathered from the **Casio Calculator Hacking** Discord server
(guild `839749858687844372`) in September–November 2026, plus the linked
community wiki and `roms.db` from the emulator the server recommends. Only
that server was searched. Sensitive flashing details are deliberately omitted;
see the wiki's "Restricted material" rules.

## Channels examined

| channel | value |
|---|---|
| `#info` | pinned knowledge base: hardware, ROP, ROM versions, emulator links |
| `#emulation` | emulator recommendations, fx-9910CW package |
| `#rom-dumping` | methodology for real-hardware dumps |
| `#rom-analysis` | mostly ROP video-player work |
| `#emulator-roms` | mostly ROM-import chatter |
| `#downloads` | list of available CasioEmuMsvc `.package` models |

Discord loads messages lazily; the browser tool has no scroll action, so older
messages in the busier channels could not all be reached. The `#info` channel
was fully loaded and is the densest source.

## Hardware per generation

| generation | MCU | notes |
|---|---|---|
| ES, fx-5800P, ES PLUS `GY-4__` | Oki ML610901 / ML610901B | mask ROM or PROM, not reflashable |
| ES PLUS `GY4__X`, `LY`, `CY`, revised MS/MS2/FC2 | Oki/Lapis ML620904 (unconfirmed) | |
| ClassWiz X / EX | Lapis ML620Q906 (nX-U16/100), confirmed via EASE debugger | 256 KiB EEPROM/flash, 8 KiB RAM at `0xD000–0xFFFF` |
| ClassWiz II / CW | Lapis ML620Q909, confirmed via EASE debugger | |
| SOLAR II | small ML620906 or possibly ML610905 | |
| original MS | unknown | |

This confirms the board split used in `src/model.rs`: `EsPlus`, `ClassWiz`
(HW id 4) and `ClassWizII` (HW id 5). Note the wiki calls the EX generation
**nX-U16/100**; our core's "large memory model" decoding covers it.

## Address format (LARGE memory model)

A 32-bit stack value is written as `cc bb xa xx`; the address is `a:bbccH`,
where `a` is the code segment (CSR) and `bbccH` the PC. On ES PLUS versions
`GY4__X`, `LY-7__`/`LY7__X` and `CY2__X`, code segments 0 and 1 are mirrored
into all other code segments — relevant to our `csr_mask` handling.

## ROP starting positions (useful for hackstring-driven ROM reads)

- ES ROM 018: the 49th character.
- ES PLUS: `initial_SP - 0x8DB8` gives (start position - 1).
- ClassWiz X: in 100an/110an/10+100an mode (110 bytes before `an`), the 35th character.
- On entry, ER0 usually points to the cache area and ER2 to the input area.

## ROM dumping methodology (`#rom-dumping`)

For ES PLUS, the practical route is: dump the official emulator ROM first,
then find gadgets by trial and error until they reproduce the same effect as
the emulator, and compute offsets from that. Basic overflow is considered
essential for ROM dumping. This matches our use of emulator-derived ROMs for
the mailbox models and real-hardware dumps for `fx570es-plus`/`fx82esplusa`.

## Version/checksum cross-check (ClassWiz X)

The wiki gives the checksum algorithm (subtract 16-bit words from 0:0000 to
0:FC00, 1:0000–FFFF, 2:0000–FFFF, 3:0000–FFF6) and a table. It confirms the
identities of the dumps already in `roms/`:

| signature in our ROM | checksum bytes | model |
|---|---|---|
| `CY-230E` | `3C39` | fx-82EX |
| `CY-235F` | `8F18` | fx-991EX |
| `CY-251F` | `AD05` | fx-991DE X |
| `CY-298A` | `BB26` | fx-580VN X |
| `71CASG` / `EY-023B` | `8113` | fx-991CN CW / fx-880BTG |

## `roms.db` (CasioEmuMsvc)

`https://github.com/telecomadm1145/CasioEmuMsvc/blob/stable/CasioEmuMsvc/roms.db`
maps model names to self-test signatures, e.g. `fx-570ES PLUS GY454X`,
`fx-82ES PLUS LY713X`, `fx-991ES PLUS LY727X`, `fx-880BTG EY-023`, and long
lists of `CY-2xx` / `EY-0xx` ClassWiz codes. This is a useful source for
extending `model::detect()` beyond the models currently registered.

Note: its `CY` numbering differs by one from the wiki's table for several EX
models (e.g. it lists `fx-991EX CY-236` where the wiki says `CY-235`), so
treat it as a cross-check, not an authority.

## Emulator tooling referenced in the server

- **CasioEmuMsvc** — https://github.com/telecomadm1145/CasioEmuMsvc/releases
  (GPL-3.0, nX-U8/100 and nX-U16/100, supports ES PLUS / EX / CW / TI).
  Uses `.package`, `model.lua`, `interface.png`, `config.bin`, `rom.bin`.
- **Cw2tools** — https://github.com/telecomadm1145/CasioEmuMsvc/tree/stable/cw2tools_en
  for editing CW ii strings, menus and fonts.
- A CW ii keyboard patcher to run emulator ROMs on real hardware with a custom
  192×63 shutdown logo (referenced in `#info`, not linked here).
- Web emulators: ClassPad.net, `emu.ca510.com` / `e.ca510.com` / `e.20142022.xyz`.
- `#downloads` lists available packages including `fx991cncwA`, `fx880btgB`,
  `fx991cwA`, `fx9910cwA`, `fx991exF`, `fx580vnxA`, `fx82exE`, `fx991dexF`.

## Relevance to this project

- The two remaining blank-ROM cases (`fx9910cw`, `fx991dex-vere`) may be
  version/revision issues; `#downloads` suggests distinct packages for
  `fx9910cwA` and `fx991dexF`, so a different ROM image may boot where ours
  does not.
- Adding more `Model` descriptors using the `roms.db` signature table is the
  cheapest next expansion; no new CPU work is needed for ES PLUS / EX / CW.
- ROM images themselves are copyrighted by CASIO and are never committed here.
