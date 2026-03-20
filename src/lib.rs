use pyo3::prelude::*;

fn hello_from_bin_impl() -> &'static str {
    "Hello from veloversi!"
}

#[pyfunction(name = "hello_from_bin")]
fn hello_from_bin_py() -> &'static str {
    hello_from_bin_impl()
}

#[pymodule]
fn _core(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(hello_from_bin_py, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::hello_from_bin_impl;

    #[test]
    fn smoke_returns_expected_message() {
        assert_eq!(hello_from_bin_impl(), "Hello from veloversi!");
    }
}
