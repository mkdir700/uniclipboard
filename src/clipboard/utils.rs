use anyhow::Result;
use clipboard_rs::common::RustImage;
use clipboard_rs::RustImageData;
use image::GenericImageView;
use image::{ImageBuffer, Rgba, RgbaImage};
use log::debug;
use png::Encoder;
use rayon::prelude::*;
use std::io::Cursor;

/// 并行转换图片为 png 格式
///
/// 注意：a 通道会被丢弃
pub fn parallel_convert_image(image: RustImageData) -> Result<Vec<u8>> {
    let img = image
        .to_rgba8()
        .map_err(|e| anyhow::anyhow!("Failed to convert image: {}", e))?;
    let (width, height) = image.get_size();

    // 如果图像很小，不进行并行处理
    if width * height < 1_000_000 {
        debug!("图像很小，不进行并行处理");
        return convert_image_simple(img, width, height);
    }

    // 并行处理大图像
    let chunk_size = height / rayon::current_num_threads().max(1) as u32 + 1;
    let chunks: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = (0..height)
        .step_by(chunk_size as usize)
        .collect::<Vec<u32>>()
        .par_iter()
        .map(|&start_y| {
            let end_y = (start_y + chunk_size).min(height);
            img.view(0, start_y, width, end_y - start_y).to_image()
        })
        .collect();

    // 重新组合图像
    let mut combined = ImageBuffer::new(width, height);
    for (i, chunk) in chunks.into_iter().enumerate() {
        let start_y = i as u32 * chunk_size;
        image::imageops::replace(&mut combined, &chunk, 0, start_y as i64);
    }

    // 编码为PNG
    convert_image_simple(combined, width, height)
}

/// 简单转换图片为 png 格式
///
/// 注意：a 通道会被丢弃
fn convert_image_simple(img: RgbaImage, width: u32, height: u32) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut encoder = Encoder::new(Cursor::new(&mut buffer), width, height);
        encoder.set_color(png::ColorType::Rgb);

        encoder.set_depth(png::BitDepth::Eight);
        // 设置压缩等级
        encoder.set_compression(png::Compression::Best);

        // 设置自适应过滤器
        // encoder.set_filter(FilterType::Sub);
        let mut writer = encoder
            .write_header()
            .map_err(|e| anyhow::anyhow!("Failed to write PNG header: {}", e))?;
        // 将RGBA转换为RGB
        let rgb_data: Vec<u8> = img.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect();
        writer
            .write_image_data(rgb_data.as_slice())
            .map_err(|e| anyhow::anyhow!("Failed to write PNG data: {}", e))?;
        // writer 在这里被丢弃，结束对 buffer 的借用
    }
    Ok(buffer)
}
