use std::process::abort;
use std::sync::Arc;

use pyo3::{
    prelude::*,
    types::{IntoPyDict, PyDict, PyString},
};

use aras_core::{self, ASGIMessage};
use aras_core::{ASGIApplication, ReceiveFn, Result, Scope, SendFn};

struct PyASGIMessage(ASGIMessage);

impl PyASGIMessage {
    fn new(msg: ASGIMessage) -> Self {
        Self { 0: msg }
    }
}

impl<'source> FromPyObject<'source> for PyASGIMessage {
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        let py_dict: &PyDict = ob.downcast()?;
        let py_msg_type = py_dict.get_item("type")?.unwrap();
        let msg_type: &str = py_msg_type.downcast::<PyString>().unwrap().to_str()?;

        let py_msg = py_dict.get_item("message")?.unwrap();
        let msg: &str = py_msg.downcast::<PyString>().unwrap().to_str()?;

        match msg_type {
            "http.response" => Ok(PyASGIMessage::new(ASGIMessage::HTTPResponse(String::from(msg)))),
            _ => panic!("Invalid response type"),
        }
    }
}

impl IntoPy<pyo3::Py<pyo3::PyAny>> for PyASGIMessage {
    fn into_py(self, py: Python<'_>) -> pyo3::Py<pyo3::PyAny> {
        let serialized_data = serde_json::to_string(&self.0).unwrap();
        let py_json = py.import("json").unwrap();
        let py_dict = py_json.call_method1("loads", (serialized_data,)).unwrap();
        py_dict.into()
    }
}

struct PyScope(Scope);

impl PyScope {
    pub fn new(scope: Scope) -> Self {
        Self { 0: scope }
    }
}

impl IntoPyDict for PyScope {
    fn into_py_dict(self, py: Python<'_>) -> &PyDict {
        let serialized_data = serde_json::to_string(&self.0).unwrap();
        let py_json = py.import("json").unwrap();
        let py_dict = py_json.call_method1("loads", (serialized_data,)).unwrap();
        py_dict.downcast().unwrap()
    }
}

#[pyclass]
struct PySend {
    send: SendFn,
}

impl PySend {
    pub fn new(send: SendFn) -> Self {
        Self { send }
    }
}

#[pymethods]
impl PySend {
    fn __call__(&self, message: &PyDict) -> Py<PyAny> {
        let rust_msg = PyASGIMessage::extract(&message).unwrap();
        let sclone = self.send.clone();
        Python::with_gil(|py| {
            let f = pyo3_asyncio::tokio::future_into_py(py, async move {
                PyResult::Ok((sclone)(rust_msg.0).await.unwrap())
            })
            .unwrap();
            f.into_py(py)
        })
    }
}

#[pyclass]
struct PyReceive {
    receive: ReceiveFn,
}

impl PyReceive {
    pub fn new(receive: ReceiveFn) -> Self {
        Self { receive }
    }
}

#[pymethods]
impl PyReceive {
    fn __call__(&self) -> Py<PyAny> {
        let rclone = self.receive.clone();
        Python::with_gil(|py| {
            let f = pyo3_asyncio::tokio::future_into_py(py, async move {
                let rust_out = (rclone)().await.unwrap();
                let py_message = PyASGIMessage::new(rust_out);
                PyResult::Ok(py_message)
            })
            .unwrap();
            f.into_py(py)
        })
    }
}

#[pyclass]
struct PyASGIAppWrapper {
    py_application: Py<PyAny>,
    task_locals: pyo3_asyncio::TaskLocals,
}

impl PyASGIAppWrapper {
    fn new(py_application: Py<PyAny>, task_locals: pyo3_asyncio::TaskLocals) -> Self {
        Self {
            py_application,
            task_locals,
        }
    }
}

impl ASGIApplication for PyASGIAppWrapper {
    async fn call(&self, scope: Scope, receive: ReceiveFn, send: SendFn) -> Result<()> {
        let future = Python::with_gil(|py| {
            let awaitable = self
                .py_application
                .call1(
                    py,
                    (
                        PyScope::new(scope).into_py_dict(py),
                        PyReceive::new(receive),
                        PySend::new(send),
                    ),
                )
                .unwrap();
            pyo3_asyncio::into_future_with_locals(&self.task_locals, awaitable.as_ref(py)).unwrap()
        });
        future.await.unwrap();
        Ok(())
    }
}

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
