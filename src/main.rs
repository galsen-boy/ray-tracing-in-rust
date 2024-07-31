#![allow(unused_variables)]
mod raytracer;

extern crate minifb;
extern crate png;
extern crate rand;
extern crate threadpool;

use minifb::{Key, Window, WindowOptions};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
// To use encoder.set()
use rand::Rng;
use raytracer::camera::Camera;
use raytracer::color::Color;
use raytracer::point_light::PointLight;
use raytracer::scene::Scene;
use raytracer::sphere::Sphere;
use raytracer::textured_sphere::TexturedSphere;
use raytracer::vec3::Vec3;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Instant;

const WIDTH: usize = 880;
const HEIGHT: usize = 800;
const BOX_SIDE: usize = 96;
const MAX_ITERATION: u32 = 5;
const RAY_PER_PIXEL: u32 = 200;
const RANDOM_OFFSET_COUNT: usize = RAY_PER_PIXEL as usize * 100;

fn color(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | (b as u32)
}

fn main() {
    let mut window = Window::new(
        "Raytracer - ESC to exit",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });

    let origin = Vec3::new(0.0, 0.5, 0.0);
    let lookat = Vec3::new(0.0, 0.5, -1.0);

    let camera = Arc::new(Camera::new(
        origin,
        lookat,
        Vec3::new(0.0, 1.0, 0.0),
        WIDTH as f32 / HEIGHT as f32,
        90.0,
        0.05,
        2.0,
    ));

    let mut scene = Scene::new();

    /*scene.add_light(Box::new(DirectionalLight::new(
        Vec3::new(0.0, -1.0, -1.0),
        (1.0, 1.0, 1.0),
    )));*/

    scene.add_light(Box::new(PointLight::new(
        Vec3::new(0.0, 1.5, -1.0),
        (1.0, 1.0, 1.0),
        0.9,
        15.0,
    )));

    /*scene.add_light(Box::new(SpotLight::new(
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        },
        0.9,
        15.0,
        15.0,
        20.0,
    )));*/

    // Ground
    scene.add_object(Box::new(TexturedSphere::new(
        Vec3::new(0.0, -10000.0, -1.0),
        10000.0,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        },
        0.2,
        0.0,
    )));

    // Left - Black
    scene.add_object(Box::new(Sphere::new(
        Vec3::new(-1.5, 0.5, -1.0),
        0.5,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        },
        0.9,
        0.0,
    )));

    // Middle - Yellow
    scene.add_object(Box::new(Sphere::new(
        Vec3::new(0.0, 0.75, -2.5),
        0.75,
        Color {
            r: 1.0,
            g: 1.0,
            b: 0.0,
        },
        0.5,
        0.0,
    )));

    // Right - Red
    scene.add_object(Box::new(Sphere::new(
        Vec3::new(1.5, 0.5, -1.0),
        0.5,
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
        },
        0.2,
        0.0,
    )));

    let scene = Arc::new(scene);

    let mut rng = rand::XorShiftRng::new_unseeded();
    let mut random_offsets: Vec<f32> = vec![0.0; RANDOM_OFFSET_COUNT];
    if RAY_PER_PIXEL > 1 {
        for i in 0..RANDOM_OFFSET_COUNT {
            random_offsets[i] = rng.next_f32() * 2.0 - 1.0;
        }
    }

    let random_offsets = Arc::new(random_offsets);

    let box_count_x: usize = WIDTH / BOX_SIDE + if WIDTH % BOX_SIDE != 0 { 1 } else { 0 };
    let box_count_y: usize = HEIGHT / BOX_SIDE + if HEIGHT % BOX_SIDE != 0 { 1 } else { 0 };

    let boxes: Vec<usize> = (0..box_count_x * box_count_y).collect();

    let pool = threadpool::Builder::new()
        .thread_name(String::from("Raytracer"))
        .build();

    let (tx, rx) = channel();

    let start = Instant::now();

    for i in boxes.iter() {
        let x = i % box_count_x;
        let y = i / box_count_x;

        let min_x = x * BOX_SIDE;
        let min_y = y * BOX_SIDE;

        let max_x = (min_x + BOX_SIDE).min(WIDTH);
        let max_y = (min_y + BOX_SIDE).min(HEIGHT);

        let buffer_width = max_x - min_x;
        let buffer_height = max_y - min_y;

        let camera = camera.clone();
        let scene = scene.clone();
        let random_offsets = random_offsets.clone();
        let tx = tx.clone();

        pool.execute(move || {
            let mut buffer = vec![0u32; buffer_width * buffer_height];

            let mut random_offset = 0usize;

            let mut rng = rand::XorShiftRng::new_unseeded();

            for y in 0..buffer_height {
                let screen_y = (min_y + y) as f32;
                for x in 0..buffer_width {
                    let screen_x = (min_x + x) as f32;

                    let mut color_r = 0f32;
                    let mut color_g = 0f32;
                    let mut color_b = 0f32;
                    for i in 0..RAY_PER_PIXEL {
                        let factor_x =
                            (screen_x + random_offsets[random_offset + 0]) / WIDTH as f32;

                        let factor_y =
                            (screen_y + random_offsets[random_offset + 1]) / HEIGHT as f32;

                        random_offset += 2;
                        if random_offset >= random_offsets.len() {
                            random_offset = 0;
                        }

                        let ray = camera.get_ray(&mut rng, factor_x, factor_y);
                        let (_, _, trace_color) = scene.trace(&mut rng, ray, MAX_ITERATION, 0f32);

                        color_r += trace_color.r;
                        color_g += trace_color.g;
                        color_b += trace_color.b;
                    }

                    color_r /= RAY_PER_PIXEL as f32;
                    color_g /= RAY_PER_PIXEL as f32;
                    color_b /= RAY_PER_PIXEL as f32;

                    let u8_r = (color_r * 255.0).min(255.0) as u8;
                    let u8_g = (color_g * 255.0).min(255.0) as u8;
                    let u8_b = (color_b * 255.0).min(255.0) as u8;

                    buffer[(y * buffer_width + x) as usize] = color(u8_r, u8_g, u8_b);
                }
            }

            tx.send((x, y, buffer)).unwrap();
        });
    }

    let mut remaining = box_count_x * box_count_y;

    let mut screen_buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if remaining > 0 {
            for (box_x, box_y, box_buffer) in rx.try_iter() {
                let min_x = box_x * BOX_SIDE;
                let min_y = box_y * BOX_SIDE;

                let max_x = (min_x + BOX_SIDE).min(WIDTH);
                let max_y = (min_y + BOX_SIDE).min(HEIGHT);

                let buffer_width = max_x - min_x;
                let buffer_height = max_y - min_y;

                let mut start_y = min_y;
                for y in 0..buffer_height {
                    let mut start_x = min_x;
                    for x in 0..buffer_width {
                        screen_buffer[start_y * WIDTH + start_x] =
                            box_buffer[y * buffer_width + x];
                        start_x += 1;
                    }
                    start_y += 1;
                }

                window.update_with_buffer(&screen_buffer, WIDTH, HEIGHT).unwrap();

                remaining -= 1;
                if remaining == 0 {
                    let duration = start.elapsed();

                    println!("Rendering took {}s", duration.as_secs_f32());

                    save_as_png("raytracer.png", WIDTH as u32, HEIGHT as u32, &screen_buffer);
                }
            }
        }

        window.update();
    }
}

fn save_as_png(file_name: &str, width: u32, height: u32, buffer: &Vec<u32>) {
    let path = Path::new(file_name);
    let file = File::create(path).unwrap();
    let w = &mut BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, width, height); // Width is 2 pixels and height is 1.
    encoder.set_color(png::ColorType::RGB);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header().unwrap();

    let mut png_data = vec![0u8; 0];
    png_data.reserve_exact((width * height * 3) as usize);

    for value in buffer.iter() {
        let r = ((value & 0x00FF_0000) >> 16) as u8;
        let g = ((value & 0x0000_FF00) >> 8) as u8;
        let b = ((value & 0x0000_00FF) >> 0) as u8;

        png_data.push(r);
        png_data.push(g);
        png_data.push(b);
    }

    writer.write_image_data(&png_data).unwrap(); // Save
}
