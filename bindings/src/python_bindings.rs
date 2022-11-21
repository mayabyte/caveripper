use pyo3::{prelude::*, types::{PyDict, PyInt, PyString}, exceptions::{PyValueError, PyException, PyIOError}};
use ::caveripper::{layout::Layout, parse_seed, assets::AssetManager, errors::SublevelError, render::{render_layout, LayoutRenderOptions, save_image}};


#[pymodule]
fn caveripper(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generate, m)?)?;
    m.add_function(wrap_pyfunction!(caveinfo, m)?)?;
    Ok(())
}

#[pyfunction("/", seed, sublevel, render)]
#[pyo3(text_signature = "(/, seed, sublevel, render)")]
fn generate<'a>(py: Python<'a>, #[pyo3(from_py_with="py_convert_seed")] seed: u32, sublevel: &str, render: Option<bool>) -> PyResult<&'a PyDict> {
    AssetManager::init_global("assets", ".")
        .expect("Couldn't initialize asset manager! Are the assets/ and resources/ directories present?");

    let sublevel = sublevel.try_into()
        .map_err(|e: SublevelError| PyValueError::new_err(e.to_string()))?;
    let caveinfo = AssetManager::get_caveinfo(&sublevel)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let layout = Layout::generate(seed, caveinfo);

    if render.unwrap_or(false) {
        let img = render_layout(&layout, LayoutRenderOptions::default())
            .map_err(|e| PyException::new_err(e.to_string()))?;
        save_image(&img, format!("{}_{:#010X}.png", sublevel.short_name(), seed))
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
    }

    let layout_json = serde_json::to_string(&layout).expect("Couldn't convert layout to JSON");
    let json = PyModule::import(py, "json")?;
    json.getattr("loads")?.call1((layout_json,))?.extract()
}

#[pyfunction]
#[pyo3(text_signature = "(sublevel, /)")]
fn caveinfo<'a>(py: Python<'a>, sublevel: &str) -> PyResult<&'a PyDict> {
    AssetManager::init_global("assets", ".")
        .expect("Couldn't initialize asset manager! Are the assets/ and resources/ directories present?");

    let caveinfo = AssetManager::get_caveinfo(
        &sublevel.try_into().map_err(|e: SublevelError| PyValueError::new_err(e.to_string()))?
    ).map_err(|e| PyValueError::new_err(e.to_string()))?;

    let json = PyModule::import(py, "json")?;
    json.getattr("loads")?.call1(
        (serde_json::to_string(&caveinfo).expect("Couldn't convert caveinfo to JSON"),)
    )?.extract()
}

fn py_convert_seed(input: &PyAny) -> PyResult<u32> {
    if let Ok(seed) = input.downcast::<PyInt>() {
        seed.extract()
    }
    else if let Ok(seed_str) = input.downcast::<PyString>() {
        parse_seed(seed_str.extract()?).map_err(|e| PyValueError::new_err(e.to_string()))
    }
    else {
        Err(PyValueError::new_err("Provided value cannot be interpreted as a Caveripper seed."))
    }
}
