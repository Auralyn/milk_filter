use dialoguer::{Input, Select};
use image::ImageBuffer;
use image::{open, RgbImage, Rgb, GenericImageView, imageops::FilterType};
use palette::color_difference::Wcag21RelativeContrast;
use palette::luma::Luma;
use palette::Srgb;
use imageproc::filter::gaussian_blur_f32;
use rand::Rng;
use std::path::{Path, PathBuf};
use std::time::Duration;
use rfd::FileDialog;
use std::process::Command;
use std::env;
use indicatif::ProgressBar;

const MILK_COLORS: [(palette::rgb::Rgb, Luma); 3] = [        
    (Srgb::new(0.67, 0.2, 0.2), Luma::new(0.85)),
    (Srgb::new(0.32, 0.15, 0.24), Luma::new(0.4)),
    (Srgb::new(0.05, 0.05, 0.08), Luma::new(0.33)),];

fn main() {
    // Pick an image file using a file dialog
    let img_path = pick_file().expect("No file selected");
    
    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));


    let img: image::ImageBuffer<Rgb<u8>, Vec<u8>> = open(&Path::new(&img_path)).expect("Failed to open image").to_rgb8();

    //let img = resize_image(&img, 256);
    //let img = gaussian_blur_f32(&img, 1.0);

    let img = contrast_stretch_by_luminance(&img);

    bar.finish();

    let items = vec!["Milk Filter", "Random Filter"];

    let selection = Select::new()
        .with_prompt("Which Filter would you like to use?")
        .items(&items)
        .interact()
        .unwrap();

    println!("You chose: {}", items[selection]);

    match items[selection] {
        "Milk Filter" => {
            let bar = ProgressBar::new(1);
            let colors = MILK_COLORS.to_vec();
            image_creating(&img, &img_path, 0, &bar, &colors);
            bar.finish();
        },
        _ => {
            let generated_images: u64 = Input::new()
                .with_prompt("How Many Images would you like to generate?")
                .interact_text()
                .unwrap();
            let color_amount: u64 = Input::new()
                .with_prompt("How many Colors would you like to use?")
                .interact_text()
                .unwrap();
            let mut range: f32 = Input::new()
                .with_prompt("How close should colors be to another? [0.01 - 0.99]")
                .interact_text()
                .unwrap();
            range = range % 0.999;
            let bar = ProgressBar::new(generated_images);

            for x in 0..generated_images {
                let colors = gen_colors(color_amount, range);
                image_creating(&img, &img_path, x, &bar, &colors);
            }
            bar.finish();
        }
    }

    
}

fn image_creating(img: &image::ImageBuffer<Rgb<u8>, Vec<u8>>, img_path: &String, x: u64, bar: &ProgressBar, colors: &[(Srgb, Luma)]) {
    // Reduce the image colors based on luminance
    let img = reduce_colors_by_luminance(&img, &colors);
    
        // Construct the output file path
    let output_path = construct_output_path(&img_path);

    // Save the result
    img.save(format!("{}{}{}", &output_path[0..output_path.len() - 4], x, ".png").as_str()).expect("Failed to save image");

    // Open the saved image
    open_image(format!("{}{}{}", &output_path[0..output_path.len() - 4], x, ".png").as_str());
    bar.inc(1);
}

fn gen_colors(color_amount: u64, range: f32) -> Vec<(palette::rgb::Rgb, Luma)> {
    //(Srgb::new(1.0, 0.0, 0.0), Luma::new(0.2126))

    let mut colors: Vec<(palette::rgb::Rgb, Luma)> = Vec::new();

    let mut rng = rand::thread_rng();
    
    for x in 0..color_amount {
        let value = rng.gen_range(0.0..1.0);

        let min_val = if value < (0.0 + range) { 0.0 } else { value - range };
        let max_val = if value > (1.0 - range) { 1.0 } else { value + range };

        let r = rng.gen_range(min_val..max_val);
        let g = rng.gen_range(min_val..max_val);
        let b = rng.gen_range(min_val..max_val);
        let l = (r + g + b) / 3.0;
        colors.push((Srgb::new(r, g, b), Luma::new(l)))
    }
    
    // Find min and max luminance values
    let min_luma = colors.iter().map(|&(_, luma)| luma.luma).fold(f32::INFINITY, f32::min);
    let max_luma = colors.iter().map(|&(_, luma)| luma.luma).fold(f32::NEG_INFINITY, f32::max);

    // Apply contrast stretching to luminance values
    for &mut (_, ref mut luma) in colors.iter_mut() {
        luma.luma = (luma.luma - min_luma) / (max_luma - min_luma);
    }

    colors
}

fn pick_file() -> Option<String> {
    FileDialog::new()
        .add_filter("Image files", &["png", "jpg", "jpeg"])
        .pick_file()
        .map(|path| path.to_string_lossy().to_string())
}

fn resize_image(img: &RgbImage, max_dimension: u32) -> RgbImage {
    let (width, height) = img.dimensions();
    let (new_width, new_height) = if width > height {
        (max_dimension, (height as f32 * max_dimension as f32 / width as f32).round() as u32)
    } else {
        ((width as f32 * max_dimension as f32 / height as f32).round() as u32, max_dimension)
    };
    image::imageops::resize(img, new_width, new_height, FilterType::Lanczos3)
}

fn reduce_colors_by_luminance(img: &RgbImage, colors: &[(Srgb, Luma)]) -> RgbImage {
    let mut result = RgbImage::new(img.width(), img.height());

    let bar = ProgressBar::new((img.width() * img.height()).into());

    for (x, y, pixel) in img.enumerate_pixels() {
        let rgb = Srgb::new(pixel[0] as f32 / 255.0, pixel[1] as f32 / 255.0, pixel[2] as f32 / 255.0);
        let luminance = rgb.into_linear().relative_luminance();
        let closest_color = find_closest_luminance_color(luminance.into(), colors);
        result.put_pixel(x, y, Rgb([(closest_color.red * 255.0) as u8, (closest_color.green * 255.0) as u8, (closest_color.blue * 255.0) as u8]));
        bar.inc(1);
    }
    bar.finish();

    result
}

fn find_closest_luminance_color(luminance: f32, colors: &[(Srgb, Luma)]) -> Srgb {
    colors.iter()
        .min_by(|&&(_, luma_a), &&(_, luma_b)| {
            let x: f32 = luma_a.into_linear().relative_luminance().try_into().unwrap();
            let y: f32 = luma_b.into_linear().relative_luminance().try_into().unwrap();
            let a_dist: f32 = (luminance - x).abs();
            let b_dist: f32 = (luminance - y).abs();
            a_dist.partial_cmp(&b_dist).unwrap()
        })
        .map(|&(color, _)| color)
        .unwrap()
}



fn construct_output_path(input_path: &str) -> String {
    let path = Path::new(input_path);
    let file_name = path.file_name().unwrap().to_string_lossy();
    let output_file_name = format!("milk_{}", file_name);

    // Get the current executable's directory
    let exe_path = env::current_exe().expect("Failed to get current executable path");
    let exe_dir = exe_path.parent().expect("Failed to get executable directory");

    // Construct the output path
    let output_path = exe_dir.join(output_file_name);
    output_path.to_string_lossy().to_string()
}

fn open_image(output_path: &str) {
    // Open the image using the default viewer
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .arg("/C")
            .arg(output_path)
            .spawn()
            .expect("Failed to open image");
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(output_path)
            .spawn()
            .expect("Failed to open image");
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(output_path)
            .spawn()
            .expect("Failed to open image");
    }
}

fn contrast_stretch_by_luminance(img: &RgbImage) -> RgbImage {
    let (width, height) = img.dimensions();
    let mut luminances = Vec::with_capacity((width * height) as usize);

    // Extract luminance values
    for pixel in img.pixels() {
        let rgb = Srgb::new(pixel[0] as f32 / 255.0, pixel[1] as f32 / 255.0, pixel[2] as f32 / 255.0);
        let luminance = rgb.into_linear().relative_luminance().luma;
        luminances.push(luminance);
    }

    // Find min and max luminance
    let min_luma = *luminances.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
    let max_luma = *luminances.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();

    // Create contrast-stretched image
    let mut stretched_img = ImageBuffer::new(width, height);

    for (x, y, pixel) in img.enumerate_pixels() {
        let rgb = Srgb::new(pixel[0] as f32 / 255.0, pixel[1] as f32 / 255.0, pixel[2] as f32 / 255.0);
        let luminance = rgb.into_linear().relative_luminance().luma;
        let stretched_luma = (luminance - min_luma) / (max_luma - min_luma);

        // Adjust the RGB values based on the stretched luminance
        let scale = stretched_luma / luminance;
        let new_rgb = Srgb::new(
            (rgb.red * scale).min(1.0).max(0.0),
            (rgb.green * scale).min(1.0).max(0.0),
            (rgb.blue * scale).min(1.0).max(0.0),
        );

        stretched_img.put_pixel(
            x,
            y,
            Rgb([
                (new_rgb.red * 255.0) as u8,
                (new_rgb.green * 255.0) as u8,
                (new_rgb.blue * 255.0) as u8,
            ]),
        );
    }

    stretched_img
}