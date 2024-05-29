use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

mod wrappers;
mod convert;

use wrappers::PyASGIAppWrapper;

#[pyfunction]
fn serve(py: Python, application: Py<PyAny>, addr: [u8; 4], port: u16) -> PyResult<()> {
    // asyncio setup
    let asyncio = py.import("asyncio")?;
    let event_loop = asyncio.call_method0("new_event_loop")?;
    asyncio.call_method1("set_event_loop", (event_loop,))?;

    // TaskLocals stores a reference to the event loop, which can be used to run Python coroutines
    let task_locals = pyo3_asyncio::TaskLocals::new(&event_loop).copy_context(py)?;

    let app_clone = application.clone_ref(py);

    // Run Rust event loop with the server in a separate thread
    std::thread::spawn(move || {
        Python::with_gil(|py| {
            pyo3_asyncio::tokio::run(py, async move {
                aras_core::serve(Arc::new(PyASGIAppWrapper::new(app_clone, task_locals)), addr, port)
                    .await
                    .map_err(|e| PyRuntimeError::new_err(format!("Error starting server; {}", e.to_string())))?;
                Ok(())
            }).map_err(|e| format!("Failed to start server; {}", e)).unwrap();
        });
    });

    // Python's event loop runs in the main thread
    let running_loop = (*event_loop).call_method0("run_forever");
    // TODO: fix having to ctrl + c twice
    if running_loop.is_err() {
        println!("Python event loop stopped");
    };

    Ok(())
}

#[pymodule]
fn aras(_: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    Ok(())
}
