use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

mod convert;
mod wrappers;

use wrappers::PyASGIAppWrapper;

fn terminate_python_event_loop(py: Python, event_loop: Py<PyAny>) -> PyResult<()> {
    let event_loop_stop_fn = event_loop.getattr(py, "stop")?;
    event_loop.call_method1(py, "call_soon_threadsafe", (event_loop_stop_fn,))?;
    loop {
        let running: bool = event_loop.call_method0(py, "is_running")?.extract(py)?;
        if running == false {
            break;
        };
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    event_loop.call_method0(py, "close")?;
    Ok(())
}

fn run_python_event_loop(event_loop: &PyAny) {
    let running_loop = (*event_loop).call_method0("run_forever");
    if running_loop.is_err() {
        println!("Python event loop quit unexpectedly");
    };
}

#[pyfunction]
fn serve(py: Python, application: Py<PyAny>, addr: [u8; 4], port: u16) -> PyResult<()> {
    // asyncio setup
    let asyncio = py.import("asyncio")?;
    let event_loop = asyncio.call_method0("new_event_loop")?;
    asyncio.call_method1("set_event_loop", (event_loop,))?;
    let event_loop_clone = event_loop.into_py(py).clone_ref(py);

    // TaskLocals stores a reference to the event loop, which can be used to run Python coroutines
    let task_locals = pyo3_asyncio::TaskLocals::new(&event_loop).copy_context(py)?;
    
    // Run Rust event loop with the server in a separate thread
    std::thread::spawn(move || {
        Python::with_gil(|py| {
            pyo3_asyncio::tokio::run(py, async move {
                let asgi_application = Arc::new(PyASGIAppWrapper::new(application, task_locals));
                aras_core::serve(asgi_application, addr, port)
                    .await
                    .map_err(|e| PyRuntimeError::new_err(format!("Error starting server; {}", e.to_string())))?;
                Ok(())
            })?;
            // When the server is done, stop Python's event loop as well
            println!("Stopping Python event loop");
            match terminate_python_event_loop(py, event_loop_clone) {
                Ok(_) => Ok(()),
                Err(e) => Err(PyRuntimeError::new_err(format!(
                    "Failed to stop Python event loop. {}",
                    e
                ))),
            }
        }).unwrap();
    });

    // Python's event loop runs in the main thread
    run_python_event_loop(event_loop);

    Ok(())
}

#[pymodule]
fn aras(_: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    Ok(())
}
