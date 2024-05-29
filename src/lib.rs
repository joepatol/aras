use std::process::abort;
use std::sync::Arc;

use pyo3::prelude::*;

mod wrappers;

use wrappers::PyASGIAppWrapper;

#[pyfunction]
fn serve(py: Python, application: Py<PyAny>) -> PyResult<()> {
    // asyncio setup
    let asyncio = py.import("asyncio")?;
    let event_loop = asyncio.call_method0("new_event_loop")?;
    asyncio.call_method1("set_event_loop", (event_loop,))?;

    // TaskLocals stores a reference to the event loop, which can be used to run Python coroutines
    let task_locals = pyo3_asyncio::TaskLocals::new(event_loop).copy_context(py)?;

    // Run Rust event loop with server in separate thread
    std::thread::spawn(move || {
        Python::with_gil(|py| {
            pyo3_asyncio::tokio::run(py, async move {
                aras_core::serve(Arc::new(PyASGIAppWrapper::new(application, task_locals)))
                    .await
                    .unwrap();
                Ok(())
            })
            .unwrap();
        });
    });

    // Python's event loop runs in the main thread
    let running_loop = (*event_loop).call_method0("run_forever");
    if running_loop.is_err() {
        println!("CTRL+C Exit");
        abort();
    }
    Ok(())
}

#[pymodule]
fn aras(_: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    Ok(())
}
