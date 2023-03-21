extern crate cpal;

use anyhow;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use raylib::prelude::*;
use ringbuf::HeapRb;

const MAX_SAMPLES: usize = 1000;
const RENDERED_SAMPLES: usize = 128;

fn main() -> Result<(), anyhow::Error> {
    let rb = HeapRb::new(MAX_SAMPLES);
    let (mut producer, mut consumer) = rb.split();

    let host = cpal::default_host();

    // Setup the default input device and stream with the default input config.
    let device = host
        .default_input_device()
        .expect("Failed to get default input device");
    println!(
        "Default input device: {}",
        device.name().expect("failed to get device name")
    );
    let mut supported_formats_range = device
        .supported_input_configs()
        .expect("error while querying formats");
    let format = supported_formats_range
        .next()
        .expect("no supported format?!")
        .with_max_sample_rate();
    eprintln!("input format was {:?}", format);

    let config: cpal::StreamConfig = device.default_input_config().unwrap().into();

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        let mut output_fell_behind = false;
        for &sample in data {
            if producer.push(sample).is_err() {
                output_fell_behind = true;
            }
        }
        if output_fell_behind {
            eprintln!("output stream fell behind: try increasing latency");
        }
    };

    let input_stream = device
        .build_input_stream(&config, input_data_fn, err_fn, None)
        .unwrap();
    input_stream.play().expect("failed to play input stream!");

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

        let mut samples = [0.0; MAX_SAMPLES];
        let sample_len = consumer.pop_slice(&mut samples);

        if sample_len > 0 {
            sound_state = SoundState {
                sound_values: samples,
                sample_size: sample_len,
            };
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
    let max_sample = get_max_f32(rendered_values);

    //if max_sample > 0.002 {
    let amplifier = get_amplifier(half_screen_height, max_sample);

    for (index, input_data) in rendered_values.iter().enumerate() {
        let x_position = index as f32 * sample_screen_width;
        let scaled_input = (*input_data).abs() * amplifier;

        d.draw_line_v(
            Vector2::new(x_position, half_screen_height - scaled_input),
            Vector2::new(x_position, scaled_input + half_screen_height),
            Color::WHITE,
        );
    }
    //}
}

fn get_amplifier(max_height: f32, max_sample: f32) -> f32 {
    let mut amplifier = (max_height * 0.5) / max_sample;
    //let min_modifier = 100.0;
    //if modifier < min_modifier {
    //    modifier = min_modifier;
    //}
    let max_amplifier = 10000.0;
    if amplifier > max_amplifier {
        amplifier = max_amplifier;
    }
    amplifier
}

fn get_max_f32(rendered_values: [f32; RENDERED_SAMPLES]) -> f32 {
    let mut max_sample = 0.0;
    for (_index, input_data) in rendered_values.iter().enumerate() {
        if max_sample < *input_data {
            max_sample = *input_data;
        }
    }
    max_sample
}

fn fit_samples(sound_values: &[f32; MAX_SAMPLES], sample_size: &usize) -> [f32; RENDERED_SAMPLES] {
    let mut rendered_values: [f32; RENDERED_SAMPLES] = [0.0; RENDERED_SAMPLES];
    let sample_chunk_size = sample_size / RENDERED_SAMPLES;
    for index in 0..RENDERED_SAMPLES {
        let iter = sound_values
            .iter()
            .skip(sample_chunk_size * index)
            .take(sample_chunk_size);
        rendered_values[index] = iter.sum::<f32>() / sample_chunk_size as f32;
    }

    rendered_values
}

struct SoundState {
    sound_values: [f32; MAX_SAMPLES],
    sample_size: usize,
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
