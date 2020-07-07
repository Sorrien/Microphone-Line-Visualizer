extern crate cpal;

use anyhow;
use cpal::{
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    StreamData, UnknownTypeInputBuffer,
};
use raylib::prelude::*;
use std::sync::mpsc::{
    self,
    Receiver, Sender,
};
use std::thread;

const MAX_SAMPLES: usize = 1000;
const RENDERED_SAMPLES: usize = 128;

fn main() -> Result<(), anyhow::Error> {
    let (tx, rx): (Sender<SoundState>, Receiver<SoundState>) = mpsc::channel();

    setup_audio(tx).expect("Audio setup failed");

    let screen_width = 1920;
    let screen_height = 1080;
    let (mut rl, thread) = raylib::init()
        .size(screen_width, screen_height)
        .title("Audio Visualizer")
        .build();

    rl.set_target_fps(60);
    let mut sound_state = SoundState {
        sound_values: [0.0; MAX_SAMPLES],
        sample_size: 0,
    };

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);

        while let Ok(audio_buffer) = rx.try_recv() {
            sound_state = audio_buffer;
        }

        if sound_state.sound_values.iter().any(|&x| x > 0.0) {
            update_lines(&mut d, &sound_state, screen_width, screen_height);
        }
    }

    Ok(())
}

fn update_lines(
    d: &mut RaylibDrawHandle,
    sound_state: &SoundState,
    screen_width: i32,
    screen_height: i32,
) {
    let rendered_values = fit_samples(&sound_state.sound_values, &sound_state.sample_size);
    let half_screen_height = screen_height as f32 / 2.0;
    let sample_screen_width = screen_width as f32 / RENDERED_SAMPLES as f32;
    for (index, input_data) in rendered_values.iter().enumerate() {
        let x_position = index as f32 * sample_screen_width;
        let scaled_input = (*input_data).abs() * 1000.0;

        d.draw_line_v(
            Vector2::new(x_position, half_screen_height - scaled_input),
            Vector2::new(x_position, (scaled_input * 2.0) + half_screen_height),
            Color::WHITE,
        );
    }
}

fn fit_samples(sound_values: &[f32; MAX_SAMPLES], sample_size: &usize) -> [f32; RENDERED_SAMPLES] {
    let mut rendered_values: [f32; RENDERED_SAMPLES] = [0.0; RENDERED_SAMPLES];
    let sample_chunk_size = sample_size / RENDERED_SAMPLES;
    for index in 0..RENDERED_SAMPLES {
        let iter = sound_values.iter().skip(sample_chunk_size * index).take(sample_chunk_size);
        rendered_values[index] = iter.sum::<f32>() / sample_chunk_size as f32;
    }

    rendered_values
}

fn setup_audio(tx: Sender<SoundState>) -> Result<(), anyhow::Error> {
    // Use the default host for working with audio devices.
    let host = cpal::default_host();

    let event_loop = host.event_loop();

    thread::spawn(move || {
        loop {           
            // Setup the default input device and stream with the default input config.
            let device = host
                .default_input_device()
                .expect("Failed to get default input device");
            println!(
                "Default input device: {}",
                device.name().expect("failed to get device name")
            );
            let mut supported_formats_range = device
                .supported_input_formats()
                .expect("error while querying formats");
            let format = supported_formats_range
                .next()
                .expect("no supported format?!")
                .with_max_sample_rate();
            eprintln!("input format was {:?}", format);

            let stream_id = event_loop
                .build_input_stream(&device, &format)
                .expect("failed to build input stream");

            event_loop
                .play_stream(stream_id)
                .expect("failed to play_stream");
            event_loop.run(move |stream_id, stream_result| {
                let stream_data = match stream_result {
                    Ok(data) => data,
                    Err(err) => {
                        eprintln!("an error occurred on stream {:?}: {}", stream_id, err);
                        return;
                    }
                };

                let mut result = SoundState {
                    sound_values: [0.0; MAX_SAMPLES],
                    sample_size: 0,
                };

                let mut index = 0;
                match stream_data {
                    StreamData::Input {
                        buffer: UnknownTypeInputBuffer::U16(buffer),
                    } => {
                        result.sample_size = buffer.len();
                        for elem in buffer.iter() {
                            if index < MAX_SAMPLES {
                                result.sound_values[index] = *elem as f32;
                                index = index + 1;
                            }
                        }
                    }
                    StreamData::Input {
                        buffer: UnknownTypeInputBuffer::I16(buffer),
                    } => {
                        result.sample_size = buffer.len();
                        for elem in buffer.iter() {
                            if index < MAX_SAMPLES {
                                result.sound_values[index] = *elem as f32;
                                index = index + 1;
                            }
                        }
                    }
                    StreamData::Input {
                        buffer: UnknownTypeInputBuffer::F32(buffer),
                    } => {
                        result.sample_size = buffer.len();
                        for elem in buffer.iter() {
                            if index < MAX_SAMPLES {
                                result.sound_values[index] = *elem;
                                index = index + 1;
                            }
                        }
                    }
                    _ => (),
                }

                tx.send(result).expect("Failed to send audio buffer data");
            });
        }
    });
    Ok(())
}

struct SoundState {
    sound_values: [f32; MAX_SAMPLES],
    sample_size: usize,
}
