#[allow(dead_code)]
use std::sync::Arc;

use log::{debug, error};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3_asyncio_0_21 as pyo3_asyncio;
use simplelog::*;

mod convert;
mod wrappers;

use wrappers::PyASGIAppWrapper;

fn terminate_python_event_loop(py: Python, event_loop: Py<PyAny>) -> PyResult<()> {
    let event_loop_stop_fn = event_loop.getattr(py, "stop")?;
    event_loop.call_method1(py, "call_soon_threadsafe", (event_loop_stop_fn,))?;
    Ok(())
}

fn run_python_event_loop(event_loop: Bound<PyAny>) {
    let running_loop = (event_loop).call_method0("run_forever");
    if running_loop.is_err() {
        error!("Python event loop quit unexpectedly");
    };
}

fn new_python_event_loop(py: Python) -> PyResult<Bound<PyAny>> {
    let module = match py.import_bound("uvloop") {
        Ok(evl) => {
            debug!("Found Python uvloop installed");
            Ok(evl)
        },
        Err(_) => {
            debug!("Uvloop not installed, using asyncio");
            py.import_bound("asyncio")
        }
    }?;

    module.call_method0("new_event_loop")
}

fn get_log_level_filter(log_level: &str) -> LevelFilter {
    match log_level {
        "DEBUG" => LevelFilter::Debug,
        "INFO" => LevelFilter::Info,
        "ERROR" => LevelFilter::Error,
        "OFF" => LevelFilter::Off,
        "TRACE" => LevelFilter::Trace,
        "WARN" => LevelFilter::Warn,
        _ => LevelFilter::Info,
    }
}

#[pyfunction]
fn serve(py: Python, application: Py<PyAny>, addr: [u8; 4], port: u16, log_level: &str) -> PyResult<()> {
    SimpleLogger::init(get_log_level_filter(log_level), Config::default())
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to start logger. {}", e)))?;

    // asyncio setup
    let asyncio = py.import_bound("asyncio")?;
    let event_loop = new_python_event_loop(py)?;
    let event_loop_clone = event_loop.clone().into();
    asyncio.call_method1("set_event_loop", (&event_loop,))?;

    // TaskLocals stores a reference to the event loop, which can be used to run Python coroutines
    let task_locals = pyo3_asyncio::TaskLocals::new(event_loop.clone().into()).copy_context(py)?;

    // Run Rust event loop with the server in a separate thread
    let server_task = std::thread::spawn(move || {
        let server_result = Python::with_gil(|py| {
            pyo3_asyncio::tokio::run(py, async move {
                let asgi_application = Arc::new(PyASGIAppWrapper::new(application, task_locals));
                aras_core::serve(asgi_application, addr, port, None)
                    .await
                    .map_err(|e| PyRuntimeError::new_err(format!("Error starting server; {}", e.to_string())))
            })
        });

        // When the server is done, stop Python's event loop as well
        debug!("Terminate Python event loop");
        if let Err(e) = Python::with_gil(|py| terminate_python_event_loop(py, event_loop_clone)) {
            return Err(e);
        };

        server_result
    });

    // Python's event loop runs in the main thread
    run_python_event_loop(event_loop);
    server_task
        .join()
        .map_err(|e| PyRuntimeError::new_err(format!("{e:?}")))??;
    Ok(())
}

#[pymodule]
fn aras(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(serve, m)?)?;
    Ok(())
}
