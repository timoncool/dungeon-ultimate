//! Живая проверка движка картинок: рисует кадр и, если передан референс, редактирует его.
//!
//! ```text
//! cargo run -p du-image --release --example scene -- \
//!     --models models/image --runtime models/runtime/sd \
//!     --prompt "a lone knight at a flooded crypt" --out scene.png
//! ```
//! С референсом добавить `--ref прошлый.png`, тогда включается edit-LoRA и пресет `krea2_edit`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use du_image::{Engine, GenParams, LoraSpec, ModelConfig, RawImage};

struct Args {
    models: PathBuf,
    runtime: PathBuf,
    prompt: String,
    out: PathBuf,
    reference: Option<PathBuf>,
    steps: i32,
    size: (i32, i32),
    seed: i64,
}

fn parse_args() -> Args {
    let mut args = Args {
        models: PathBuf::from("models/image"),
        runtime: PathBuf::from("models/runtime/sd"),
        prompt: "a lone knight in dented plate armor at the mouth of a flooded stone crypt, \
                 torchlight raking across wet walls, cinematic concept art"
            .into(),
        out: PathBuf::from("scene.png"),
        reference: None,
        steps: 8,
        size: (1024, 1024),
        seed: 42,
    };
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < raw.len() {
        let value = raw.get(index + 1).cloned().unwrap_or_default();
        match raw[index].as_str() {
            "--models" => args.models = PathBuf::from(value),
            "--runtime" => args.runtime = PathBuf::from(value),
            "--prompt" => args.prompt = value,
            "--out" => args.out = PathBuf::from(value),
            "--ref" => args.reference = Some(PathBuf::from(value)),
            "--steps" => args.steps = value.parse().unwrap_or(8),
            "--seed" => args.seed = value.parse().unwrap_or(42),
            "--size" => {
                if let Some((w, h)) = value.split_once('x') {
                    args.size = (w.parse().unwrap_or(1024), h.parse().unwrap_or(1024));
                }
            }
            other => eprintln!("неизвестный аргумент: {other}"),
        }
        index += 2;
    }
    args
}

fn read_png(path: &Path) -> Result<RawImage, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("не открыть {path:?}: {e}"))?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().map_err(|e| format!("не прочитать PNG: {e}"))?;
    let mut buffer = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buffer).map_err(|e| format!("не декодировать PNG: {e}"))?;
    buffer.truncate(info.buffer_size());
    // Движок ждёт три канала, поэтому альфу отбрасываем.
    let data = match info.color_type {
        png::ColorType::Rgb => buffer,
        png::ColorType::Rgba => buffer.chunks_exact(4).flat_map(|px| px[..3].to_vec()).collect(),
        other => return Err(format!("не поддержанный цвет PNG: {other:?}")),
    };
    Ok(RawImage { width: info.width, height: info.height, channels: 3, data })
}

fn write_png(path: &Path, image: &RawImage) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("не создать {path:?}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(if image.channels == 4 { png::ColorType::Rgba } else { png::ColorType::Rgb });
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| format!("не записать заголовок: {e}"))?
        .write_image_data(&image.data)
        .map_err(|e| format!("не записать пиксели: {e}"))
}

fn main() -> Result<(), String> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = parse_args();

    let engine = Engine::load(&args.runtime).map_err(|e| e.to_string())?;
    println!("движок {} ({})", engine.version(), engine.commit());
    for device in engine.devices() {
        println!("  устройство: {}", device.replace('\t', " — "));
    }

    let config = ModelConfig::krea2(
        args.models.join("krea2-turbo-Q4_K_M.gguf"),
        args.models.join("qwen3-vl-4b-abliterated-Q4_K_M.gguf"),
        args.models.join("wan_2.1_vae.safetensors"),
    );
    let started = Instant::now();
    let mut context = engine.new_context(&config).map_err(|e| e.to_string())?;
    println!("модель загружена за {:.1}с", started.elapsed().as_secs_f32());

    let mut params = GenParams {
        prompt: args.prompt.clone(),
        width: args.size.0,
        height: args.size.1,
        steps: args.steps,
        seed: args.seed,
        ..Default::default()
    };
    // Референс включает режим правки: та же модель плюс edit-LoRA, без второго набора весов.
    if let Some(path) = args.reference.as_ref() {
        params.ref_images = vec![read_png(path)?];
        params.ref_image_args = Some("preset=krea2_edit".into());
        params.loras = vec![LoraSpec {
            path: args.models.join("krea2-identity-edit-r128.safetensors"),
            multiplier: 1.0,
        }];
        println!("режим правки по референсу {path:?}");
    }

    let started = Instant::now();
    let images = context
        .generate(
            &params,
            Some(Box::new(|step, steps, time| {
                println!("  шаг {step}/{steps} ({time:.2}с)");
            })),
        )
        .map_err(|e| e.to_string())?;
    let elapsed = started.elapsed().as_secs_f32();

    let image = images.first().ok_or("движок не вернул ни одного кадра")?;
    write_png(&args.out, image)?;
    println!(
        "готово: {}×{}, {} каналов, {:.1}с → {}",
        image.width,
        image.height,
        image.channels,
        elapsed,
        args.out.display()
    );
    Ok(())
}
