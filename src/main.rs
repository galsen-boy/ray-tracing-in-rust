#![allow(unused_variables)]
mod raytracer;

extern crate minifb;
extern crate png;
extern crate rand;
extern crate threadpool;

use minifb::{Key, Window, WindowOptions};
use std::fs::File;
use std::io::{self, Write};
// To use encoder.set()
use rand::Rng;
use raytracer::camera::Camera;
use raytracer::color::Color;
use raytracer::point_light::PointLight;
use raytracer::scene::Scene;
use raytracer::sphere::Sphere;
use raytracer::cylinder::Cylinder;
use raytracer::cube::Cube;
use raytracer::textured_sphere::TexturedSphere;
use raytracer::vec3::Vec3;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Instant;

const WIDTH: usize = 880;
const HEIGHT: usize = 800;
const BOX_SIDE: usize = 96;
const MAX_ITERATION: u32 = 3;
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

    let origin = Vec3::new(0.0, 0.5, 1.0);
    let lookat = Vec3::new(0.0, 0.5, -1.0);

    let camera = Arc::new(Camera::new(
        origin,
        lookat,
        Vec3::new(0.0, 0.5, 3.5),
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
    scene.add_object(Box::new(Cube::new(
        Vec3::new(0.0, 0.75, -2.5),
        Vec3::new(0.75, 0.75, 0.75), // demi-taille du cube
        Color {
            r: 1.0,
            g: 1.0,
            b: 0.0,
        },
        0.5,
        0.0,
    )));


    // Right - Red
    scene.add_object(Box::new(Cylinder::new(
        Vec3::new(1.5, 0.5, -1.0),
        0.5,
        1.5,
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


                    save_buffer_as_ppm(&screen_buffer, WIDTH, HEIGHT, "raytracer.ppm").unwrap();

                }
            }
        }

        window.update();
    }
}

fn save_buffer_as_ppm(buffer: &Vec<u32>, width: usize, height: usize, output_path: &str) -> io::Result<()> {
    // Ouvrir le fichier de sortie
    let mut file = File::create(output_path)?;

    // Écrire l'en-tête PPM
    writeln!(file, "P6")?;
    writeln!(file, "{} {}", width, height)?;
    writeln!(file, "255")?;

    // Écrire les données de l'image
    for &pixel in buffer.iter() {
        let red = ((pixel >> 16) & 0xFF) as u8;  // 16-bit shift pour le rouge
        let green = ((pixel >> 8) & 0xFF) as u8; // 8-bit shift pour le vert
        let blue = (pixel & 0xFF) as u8;         // pas de shift pour le bleu
        file.write_all(&[red, green, blue])?;
    }

    Ok(())
}