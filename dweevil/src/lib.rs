mod assets;
mod utils;

use assets::WebAssetManager;
use caveripper::{
    assets::AssetManager,
    layout::Layout,
    render::{render_layout, LayoutRenderOptions, RenderHelper},
    sublevel::Sublevel,
};
use image::{ImageBuffer, Rgba};
use utils::set_panic_hook;
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::ImageData;

#[wasm_bindgen]
pub struct Image {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
pub fn cavegen(sublevel: &str, seed: u32) -> Result<Image, JsValue> {
    set_panic_hook();

    let mgr = WebAssetManager::new();
    let sublevel = Sublevel::try_from_str(sublevel, &mgr).expect("Failed to load sublevel");
    let caveinfo = mgr.load_caveinfo(&sublevel).expect("Failed to load caveinfo");
    let layout = Layout::generate(seed, caveinfo);
    let image = render_layout(&layout, &RenderHelper::new(&mgr), LayoutRenderOptions::default()).expect("Failed to render");

    let width = image.width();
    let height = image.height();
    Ok(Image {
        data: image.into_raw(),
        width,
        height,
    })
}

#[wasm_bindgen]
pub fn draw_to_canvas(image: Image, canvas_id: String) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");
    let canvas = document
        .get_element_by_id(&canvas_id)
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    let context = canvas.get_context("2d")?.unwrap().dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    context.clear_rect(0.0, 0.0, 2048.0, 2048.0);

    let image_data_temp = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&image.data), image.width, image.height)?;
    context.put_image_data(&image_data_temp, 0.0, 0.0)?;
    Ok(())
}
