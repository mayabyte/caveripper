mod assets;
mod utils;

use std::sync::OnceLock;

use assets::WebAssetManager;
use caveripper::{
    assets::AssetManager,
    layout::Layout,
    query::{Query, StructuralQuery},
    render::{render_layout, LayoutRenderOptions, RenderHelper},
    sublevel::Sublevel,
};
use js_sys::{
    Date,
    Math::{floor, pow, random},
};
use utils::set_panic_hook;
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::ImageData;

static GLOBAL_MGR: OnceLock<WebAssetManager> = OnceLock::new();

#[inline]
fn mgr() -> &'static WebAssetManager {
    GLOBAL_MGR.get_or_init(WebAssetManager::new)
}

#[wasm_bindgen]
pub struct Image {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
pub fn cavegen(sublevel: &str, seed: u32) -> Result<Image, JsValue> {
    set_panic_hook();

    let sublevel = Sublevel::try_from_str(sublevel, mgr()).expect("Failed to parse sublevel");
    let caveinfo = mgr().load_caveinfo(&sublevel).expect("Failed to load caveinfo");
    let layout = Layout::generate(seed, caveinfo);
    Ok(render(layout))
}

fn render(layout: Layout) -> Image {
    let image = render_layout(&layout, &RenderHelper::new(mgr()), LayoutRenderOptions::default()).expect("Failed to render");

    let width = image.width();
    let height = image.height();
    Image {
        data: image.into_raw(),
        width,
        height,
    }
}

#[wasm_bindgen]
pub fn query(query: &str) -> Result<Image, JsValue> {
    let query = StructuralQuery::try_parse(query, mgr()).map_err(|_| JsValue::NULL)?;
    let sublevel = &query.clauses[0].sublevel;
    let caveinfo = mgr().load_caveinfo(&sublevel).expect("Failed to load caveinfo");

    let start_time = Date::new_0().get_seconds();
    loop {
        if Date::new_0().get_seconds() - start_time > 10 {
            break Err(JsValue::NULL);
        }

        let seed = floor(random() * pow(2.0, 32.0)) as u32;
        if query.matches(seed, mgr()) {
            let layout = Layout::generate(seed, &caveinfo);
            break Ok(render(layout));
        }
    }
}

#[wasm_bindgen]
pub fn draw_to_canvas(image: Image, canvas_id: String) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().expect("Could not get document");

    let canvas = document
        .get_element_by_id(&canvas_id)
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    canvas.set_height(image.height);
    canvas.set_width(image.width);
    let context = canvas.get_context("2d")?.unwrap().dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    context.clear_rect(0.0, 0.0, 2048.0, 2048.0);

    let image_data_temp = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&image.data), image.width, image.height)?;
    context.put_image_data(&image_data_temp, 0.0, 0.0)?;
    Ok(())
}
