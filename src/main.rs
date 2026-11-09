use c8::{HEIGHT, RAM_SIZE, WIDTH};
use minifb::{Key, Scale, Window, WindowOptions};
use rodio::{source::SineWave, OutputStream, Sink, Source};
use std::{env, fs, time::Duration};

mod c8;

fn main() {
    println!("CHIP-8 Interpreter/Emulator");

    // Usage: chip8 <rom_file> [instructions_per_frame]
    let args: Vec<String> = env::args().collect();
    let file_name = args
        .get(1)
        .expect("Usage: cargo run --release -- <rom_file> [instructions_per_frame]");
    let cycles_per_frame: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    let rom = fs::read(file_name).unwrap_or_else(|e| panic!("Couldn't load {}: {}", file_name, e));
    let mut c8 = c8::C8::new();
    c8.load_ram(&rom);

    // Audio is optional: if there is no output device, run silently.
    // `_stream` must stay alive or the sound stops.
    let audio = OutputStream::try_default().ok().and_then(|(stream, handle)| {
        let sink = Sink::try_new(&handle).ok()?;
        sink.append(SineWave::new(440.0).amplify(0.2));
        sink.pause();
        Some((stream, sink))
    });
    if audio.is_none() {
        println!("No audio device found, running without sound.");
    }

    let mut window = Window::new(
        &format!("CHIP-8: {}", file_name),
        WIDTH,
        HEIGHT,
        WindowOptions {
            scale: Scale::X16,
            ..WindowOptions::default()
        },
    )
    .expect("Unable to open a Window");

    // One loop iteration = one 60 Hz frame
    window.limit_update_rate(Some(Duration::from_micros(16_667)));

    while window.is_open() && !window.is_key_down(Key::Escape) && (c8.pc as usize) < RAM_SIZE {
        // Keypad: 1 2 3 4 / Q W E R / A S D F / Z X C V
        let mut key_press = [false; 16];
        for key in window.get_keys() {
            match key {
                Key::Key1 => key_press[0x1] = true,
                Key::Key2 => key_press[0x2] = true,
                Key::Key3 => key_press[0x3] = true,
                Key::Key4 => key_press[0xC] = true,
                Key::Q => key_press[0x4] = true,
                Key::W => key_press[0x5] = true,
                Key::E => key_press[0x6] = true,
                Key::R => key_press[0xD] = true,
                Key::A => key_press[0x7] = true,
                Key::S => key_press[0x8] = true,
                Key::D => key_press[0x9] = true,
                Key::F => key_press[0xE] = true,
                Key::Z => key_press[0xA] = true,
                Key::X => key_press[0x0] = true,
                Key::C => key_press[0xB] = true,
                Key::V => key_press[0xF] = true,
                _ => {}
            }
        }

        for _ in 0..cycles_per_frame {
            c8.run(&key_press);
        }
        c8.tick_timers();

        if let Some((_, sink)) = &audio {
            if c8.st > 0 && sink.is_paused() {
                sink.play();
            } else if c8.st == 0 && !sink.is_paused() {
                sink.pause();
            }
        }

        window
            .update_with_buffer(&c8.display, WIDTH, HEIGHT)
            .unwrap();
    }
}