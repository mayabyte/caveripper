use pyo3::{prelude::*, types::{PyDict, PyInt, PyString}, exceptions::PyValueError};

use crate::{layout::Layout, parse_seed, assets::AssetManager, errors::{SublevelError, AssetError}};

#[pymodule]
fn caveripper(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generate, m)?)?;
    Ok(())
}

#[pyfunction]
#[pyo3(text_signature = "(seed, sublevel, /)")]
fn generate<'a> (py: Python<'a>, #[pyo3(from_py_with="py_convert_seed")] seed: u32, sublevel: &str) -> PyResult<&'a PyDict> {
    AssetManager::init_global("assets", ".").expect("Couldn't initialize asset manager! Are the assets/ and resources/ directories present?");
    let caveinfo = AssetManager::get_caveinfo(&sublevel.try_into()?)?;
    let layout = Layout::generate(seed, caveinfo);
    let layout_json = serde_json::to_string(&layout).expect("Couldn't convert layout to JSON");

    let json = PyModule::import(py, "json")?;
    json.getattr("loads")?.call1((layout_json,))?.extract()
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

impl From<SublevelError> for PyErr {
    fn from(err: SublevelError) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}

impl From<AssetError> for PyErr {
    fn from(err: AssetError) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}
