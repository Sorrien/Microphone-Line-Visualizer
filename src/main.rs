extern crate cpal;

use anyhow;
use cpal::{
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    StreamData, UnknownTypeInputBuffer,
};
use raylib::prelude::*;
use std::sync::{
    mpsc,
    mpsc::{Receiver, Sender},
};
use std::thread;

const MAX_SAMPLES: usize = 128;

fn main() -> Result<(), anyhow::Error> {
    let (tx, rx): (Sender<[f32; MAX_SAMPLES]>, Receiver<[f32; MAX_SAMPLES]>) = mpsc::channel();

    setup_audio(tx).expect("Audio setup failed");

    let screen_width = 1920;
    let screen_height = 1080;
    let (mut rl, thread) = raylib::init()
        .size(screen_width, screen_height)
        .title("Audio Visualizer")
        .build();

    rl.set_target_fps(30);
    let mut sound_values = [0.0; MAX_SAMPLES];

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);

        while let Ok(audio_buffer) = rx.try_recv() {
            sound_values = audio_buffer;
        }
        if sound_values.iter().any(|&x| x > 0.0) {
            update_lines(&mut d, &sound_values, screen_width, screen_height);
        }
    }

    Ok(())
}

fn update_lines(
    d: &mut RaylibDrawHandle,
    sound_values: &[f32; MAX_SAMPLES],
    screen_width: i32,
    screen_height: i32,
) {
    let half_screen_height = screen_height as f32 / 2.0;
    let num_samples = sound_values.len() as f32;
    let sample_screen_width = screen_width as f32 / num_samples;
    for (index, input_data) in sound_values.iter().enumerate() {
        let x_position = index as f32 * sample_screen_width;
        let scaled_input = (*input_data).abs() * 1000.0;

        d.draw_line_v(
            Vector2::new(x_position, half_screen_height - scaled_input),
            Vector2::new(x_position, (scaled_input * 2.0) + half_screen_height),
            Color::WHITE,
        );
    }
}

fn setup_audio(tx: Sender<[f32; MAX_SAMPLES]>) -> Result<(), anyhow::Error> {
    // Use the default host for working with audio devices.
    let host = cpal::default_host();

    let event_loop = host.event_loop();

    thread::spawn(move || {
        loop {
            // Setup the default input device and stream with the default input config.
            let device = host
                .default_input_device()
                .expect("Failed to get default input device");
            println!("Default input device: {}", device.name().expect("failed to get device name"));
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

                let mut result: [f32; MAX_SAMPLES] = [0.0; MAX_SAMPLES];
                let mut index = 0;
                match stream_data {
                    StreamData::Input {
                        buffer: UnknownTypeInputBuffer::U16(buffer),
                    } => {
                        for elem in buffer.iter() {
                            if index < MAX_SAMPLES {
                                result[index] = *elem as f32;
                                index = index + 1;
                            }
                        }
                    }
                    StreamData::Input {
                        buffer: UnknownTypeInputBuffer::I16(buffer),
                    } => {
                        for elem in buffer.iter() {
                            if index < MAX_SAMPLES {
                                result[index] = *elem as f32;
                                index = index + 1;
                            }
                        }
                    }
                    StreamData::Input {
                        buffer: UnknownTypeInputBuffer::F32(buffer),
                    } => {
                        for elem in buffer.iter() {
                            if index < MAX_SAMPLES {
                                result[index] = *elem;
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
