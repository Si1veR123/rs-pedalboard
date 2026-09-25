I explored the repo thoroughly (no README/LICENSE/CI/docs currently exist). Here's what I verified, followed by a proposed README layout and the content to put in it.
## What the project actually is (verified from code)
| Aspect | Finding |
|---|---|
| Crate | `rs-pedalboard` v0.1.0, edition 2021, lib + 3 bins |
| Bins | `pedalboard-processor` (`src/bin/processor/bin.rs`, needs `processor`), `pedalboard-client` (`src/bin/client/bin.rs`, needs `client`), `pedalboard-info` (no features) |
| Architecture | Two processes over **TCP 127.0.0.1:29475**, newline-delimited text commands, JSON payloads (`serde_json`) for pedalboards/pedals |
| Processor | Headless real-time engine: `cpal` (WASAPI/ASIO on Windows; Linux = ALSA devices + bundled **JACK server**), `ringbuf` + `crossbeam` + `smol` |
| Client | `eframe`/`egui` GUI, fullscreen by default (touch-first), wgpu renderer by default, `glow` feature as alternative (Raspberry Pi fix in code), vendored `custom_egui_dnd` fork |
| Pedals | 22 built-ins + VST2/VST3 hosting, NAM, IR loader (list below) |
| Persistence | `~/rs_pedalboard/` → `pedalboards.json`, `client_settings.json`, `processor_settings.json`, MIDI save, `Recordings/`; logs `pedalboard-client.log` / `pedalboard-processor.log` in CWD |
| Tests | Unit tests in 21 files (`#[test]` in dsp_algorithms, pedals, pedalboard, plugin, ui helpers) → `cargo test`; **no CI workflows** |
| Missing | no LICENSE, no docs/screenshots, no CI, no rust-toolchain file |
---
# Proposed README layout
### 1. Title block
- Name + one-line pitch, e.g. *"rs-pedalboard — a client/server guitar effects pedalboard and amp-sim rig for Windows and Linux."*
- Badges: build status (only if you add CI), license, Rust version. **Note:** with no CI today, either add a `.github/workflows/ci.yml` first or skip the status badge rather than shipping a dead badge.
- Screenshot/hero image placeholder (none exist yet — decide whether to capture Stage/Utilities screens).
### 2. "What is this?" (3–5 lines)
- Runs your whole rig on a PC/laptop/Raspberry Pi: audio I/O, signal chain, footswitches, all controlled from a touchscreen UI.
- Explicitly state the **two-process model** up front, because it explains almost every instruction later: `pedalboard-processor` owns the audio device and DSP; `pedalboard-client` is the GUI and talks to it over localhost TCP (port 29475). They can run on the same machine, and the client can auto-launch the processor.
### 3. Features
Group into short bullets (or a `<details>` block if long):
- **Signal chain**: ordered pedals per pedalboard, drag-and-drop reordering, per-pedal knobs, footswitch active bypass.
- **Pedal library**: 22 built-in effects (see §10), plus VST2, VST3, Neural Amp Modeler and impulse-response cabs.
- **Multi-board workflow**: "stage" = several pedalboards with one active; Library = saved boards; Songs = saved ordered lists of boards for a setlist (play/next/prev).
- **Utilities**: chromatic tuner (YIN pitch detection, configurable min/max freq + periods), metronome (BPM/volume), recorder (32-bit float mono WAV of the output, optional "clean" capture), volume monitor, master in/out volume, mute.
- **Gear/MIDI**: MIDI control of parameters — absolute, relative and inc/dec style, auto-connect ports, per-pedalboard parameter mappings, and "sensible parameter" mapping so generic MIDI knobs/footswitches control the right things.
- **Monitoring**: clipping detection, xrun counter, CPU/RAM readout on the Stage screen.
- **Audio quality options**: block-size choice, internal ring-buffer latency, up to N×2 upsampling passes, sample-rate handling/resampling (rubato), optional volume normalization (off/manual/automatic with decay).
- **Persistence**: everything JSON in `~/rs_pedalboard/`; client can start/kill the processor.
- **UI**: fullscreen, touch-friendly, zoom buttons for touchscreens, optional on-screen virtual keyboard (`virtual_keyboard` feature).
### 4. Screenshots / demo
- Decide: a small table of 4 images (Stage, pedal knobs, Utilities, Settings) or omit. Add `docs/` (or `assets/`) folder convention and document it, since none exists.
### 5. Architecture (diagram + tables)
- ASCII or Mermaid diagram: `client (egui) ⇄ TCP :29475 ⇄ processor (cpal stream) → ring buffer → pedalboard set → pedal chain → output`.
- Table mapping crate layout → responsibility:
  - `src/pedals/` — `PedalTrait` pedals (`enum_dispatch`, `Pedal` enum, `PedalDiscriminants::new_pedal()/display_name()`) + parameter model
  - `src/dsp_algorithms/` — biquad, eq, frequency analysis, impulse response, moving bandpass, oscillator, phaser, resampler, variable delay (+ phase-vibrato), YIN
  - `src/pedalboard.rs` / `src/pedalboard_set.rs` — chain + active board, sensible-parameter mapping for MIDI/knobs
  - `src/plugin/` — VST2/VST3 hosting
  - `src/processor_api.rs` — offline processing helpers (`process_audio`, `process_audio_file(_and_save)`, `load_wav`)
  - `src/socket_helper.rs` — newline-framed command framing shared by both sides
  - `custom_egui_dnd/` — vendored `egui_dnd` 0.12 fork (custom knob drag sensitivity, etc.) — say *why* it's vendored
- Short "signal flow" description: `PROCESSING_BUFFER_SIZE = 1024` blocks; pedals get `set_config(buffer_size, sample_rate)`; parameters are `Float | Bool | Int | String | Oscillator` keyed by name in `HashMap`; a `ParameterPath { pedalboard_id, pedal_id, parameter_name }` addresses any parameter across all boards.
### 6. Requirements / platform support matrix
- Rust toolchain (verified working locally: rustc 1.98.1) — state an MSRV only if you actually test one.
- Table: **Windows** WASAPI + ASIO (default feature `asio`), **Linux** ALSA devices + bundled JACK server (JACK is the only host), **macOS unsupported** (`SupportedHost` is only defined for `linux`/`windows` → compile error).
- ASIO build caveat: `cpal/asio` pulls `asio-sys`, which **downloads the Steinberg ASIO SDK at build time** (or honours `CPAL_ASIO_DIR`); note `--no-default-features` to skip it.
- Linux system deps (ALSA/JACK dev libs) — verify exact package names for your distro before claiming them.
### 7. Build & run (quickstart)
- Feature-flag table from `Cargo.toml`: `default = ["asio"]`, `processor`, `client`, `glow`, `windowed`, `virtual_keyboard`, `log_full_commands`.
- Key point: the bins have `required-features`, so a plain `cargo build` produces **only `pedalboard-info`**. Document:
  - Windows: `cargo build --release --features client,processor`
  - Linux / no-ASIO: `cargo build --release --no-default-features --features client,processor`
  - `cargo run --release --features client,processor --bin pedalboard-client`
- Processor auto-launch: the client looks for the `RSPEDALBOARD_PROCESSOR` env var (a full path) and otherwise `which("pedalboard_processor")`. **Gotcha to document prominently:** the search name uses an underscore while the built binary is `pedalboard-processor(.exe)`, so either set the env var or ship/rename the binary — otherwise users hit "processor executable not found".
- Manual/headless start of the processor, and the `--no-processor` client flag when you run the processor yourself.
- Logging: `RUST_LOG` (console defaults to `info`, file layer `debug`), log file names/locations.
### 8. First run / usage walkthrough
Step-by-step narrative that mirrors the code: choose host + input/output device + sample rate + buffer size in Settings → client starts processor → Stage screen: add pedals from the menu, drag to reorder, turn knobs, click knobs to make them the "active parameter", toggle footswitches → save board to Library → build Songs from boards → Utilities: tuner, metronome, recorder → Settings: MIDI port/device mapping, NAM/IR/VST2 folder configuration, volume normalization, kill/start processor.
- Mention loading NAM models, IRs and VST2/VST3 plugins, and that `pedalboard-info` dumps every pedal's default parameter set as JSON (useful for tooling/debugging).
### 9. Configuration & data files
- Table of paths: `~/rs_pedalboard/pedalboards.json`, `client_settings.json`, `processor_settings.json`, MIDI settings, `Recordings/`, plus the two `.log` files in the working directory.
- How to reset: delete the JSON files and/or run the processor with `--ignore-save` (verified flag).
### 10. Pedals & plugins reference
Table of the 22 built-ins with the exact `display_name()` strings: Volume, Fuzz, Pitch Shift, Chorus, Flanger, Delay, Graphic EQ, Simple EQ, Neural Amp Modeler, Impulse Response, Noise Gate, VST2 Plugin, VST3 Plugin, Reverb, Vibrato, Tremolo, Auto Wah, Wah, Compressor, Overdrive, Phaser, Distortion. Add a column for "based on / notes" only where you're confident (e.g. overdrive/distortion modelled after TS/DS-1 per commit history — verify per pedal before writing claims).
### 11. CLI reference
Two tables from `clap`:
- Processor: `-h/--host`, `-f/--frames-per-period`, `-b/--buffer-latency`, `--periods-per-buffer`, `--tuner-min-freq`, `--tuner-max-freq`, `--tuner-periods`, `-i/--input-device`, `-o/--output-device`, `--preferred-sample-rate`, `--upsample-passes`, `--ignore-save`, `--recording-dir`. Include the platform defaults (Windows 512 frames / 7.5 ms; Linux 256 frames / 5.0 ms).
- Client: `--no-processor`.
### 12. Using it as a library (optional but valuable)
- `pedalboard-info` exists because the crate is usable programmatically: `Pedalboard`, `PedalboardSet`, `PedalTrait` (to implement a custom pedal), `processor_api::process_audio_file_and_save`, `set_use_vst2_global_host`, `set_nam_save_path` / `set_ir_save_path` / `set_vst2_save_path` / `set_vst3_save_path`.
### 13. Performance & latency notes
- Explain the knobs users will ask about: `frames_per_period` = `2^buffer_size`, internal ring buffer size = `buffer_size*2 + latency_ms`, `upsample_passes` multiplier, where xruns/clipping surface in the UI.
### 14. Troubleshooting / FAQ
- Processor executable not found (the underscore gotcha + env var).
- "Not connected to processor" screen (Utilities shows it explicitly).
- No devices listed / ASIO driver unavailable; ASIO is Windows-only.
- Xruns/clipping → increase buffer size/latency, watch CPU/RAM readout.
- Linux: JACK/ALSA setup, devices chosen from a terminal menu when not passed as flags (device select menus on stdin), and that the processor starts the JACK server.
- macOS unsupported.
- Where the logs are.
### 15. Status / roadmap / contributing
- Be honest about state: VST3 support landed recently, some pedal UIs are in progress, no CI yet, single client at a time (the processor handles one connection).
- Contributing: `cargo fmt`, `cargo clippy`, `cargo test`; describe commit-message convention visible in the log (`bugfix: …`, `fix …`, `add …`).
### 16. License + acknowledgements
- **Decision needed:** there's no LICENSE file. Given the VST3-SDK-dependent `vst3` crate and the Steinberg ASIO SDK used at build time, GPL-3.0 is the usual pragmatic choice, but confirm before stating it — don't write "MIT" casually. If undecided, say "license TBD".
- Credits: `egui`/`eframe`, `cpal`, `midir`, `rubato`, `realfft`, `hound`, `freeverb`, `signalsmith-stretch`, NAM (`neural-amp-modeler-core-bindgen` + `Si1veR123` forks), and the vendored `egui_dnd` fork (upstream Lucas Meurer, MIT).
---
## Ordering advice and conventions
- Put **quickstart before architecture** for a project like this; keep the two-process explanation in the first 15 lines so the quickstart makes sense.
- Use tables for features/flags/CLI/files; push long pedal lists into `<details>` if the README grows past ~250 lines.
- Add a table of contents only if you exceed ~10 sections (GitHub auto-outlines anyway).
- Consider splitting the deep dives into `docs/` and keeping README as a landing page + quickstart.
## Things to verify/decide before you write
1. **License** (and whether you'll add a `LICENSE` file).
2. Whether you'll add CI (needed for a build badge) and the exact build matrix (Windows ASIO vs Windows WASAPI vs Linux).
3. Whether you want screenshots (none in the repo today) and where they live (`docs/`).
4. Whether the crate is meant to be publishable/consumable as a library (affects §12 plus `Cargo.toml` metadata: `description`, `license`, `repository`, `readme`).
5. Whether to rename the processor binary or the `PROCESSOR_EXE_NAME` constant so the auto-launch works out of the box — whichever you pick determines how §7 is worded.
6. Confirm per-pedal "modelled after" claims before publishing them.