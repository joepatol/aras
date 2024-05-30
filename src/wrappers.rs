use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyDict, PyString},
};

use aras_core::{
    self, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartupComplete, LifespanStartupFailed,
};
use aras_core::{ASGIApplication, ASGIMessage, ReceiveFn, Result, Scope, SendFn};

use super::convert;

pub struct PyASGIMessage(ASGIMessage);

impl PyASGIMessage {
    fn new(msg: ASGIMessage) -> Self {
        Self { 0: msg }
    }
}

impl<'source> FromPyObject<'source> for PyASGIMessage {
    fn extract(ob: &'source PyAny) -> PyResult<Self> {
        let py_dict: &PyDict = ob.downcast()?;
        let msg_type = py_dict
            .get_item("type")?
            .ok_or(PyValueError::new_err("Field 'type' is required"))?
            .downcast::<PyString>()?
            .to_str()?;

        match msg_type {
            "http.response.start" => Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseStart(
                convert::parse_py_http_response_start(py_dict)?,
            ))),
            "http.response.body" => Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseBody(
                convert::parse_py_http_response_body(py_dict)?,
            ))),
            "lifespan.startup.complete" => Ok(PyASGIMessage::new(ASGIMessage::StartupComplete(
                LifespanStartupComplete::new(),
            ))),
            "lifespan.startup.failed" => Ok(PyASGIMessage::new(ASGIMessage::StartupFailed(
                LifespanStartupFailed::new(convert::parse_lifespan_failed_message(py_dict)?),
            ))),
            "lifespan.shutdown.complete" => Ok(PyASGIMessage::new(ASGIMessage::ShutdownComplete(
                LifespanShutdownComplete::new(),
            ))),
            "lifespan.shutdown.failed" => Ok(PyASGIMessage::new(ASGIMessage::ShutdownFailed(
                LifespanShutdownFailed::new(convert::parse_lifespan_failed_message(py_dict)?),
            ))),
            _ => Err(PyValueError::new_err(format!("Invalid message type '{}'", msg_type))),
        }
    }
}

impl IntoPy<Py<PyAny>> for PyASGIMessage {
    fn into_py(self, py: Python<'_>) -> Py<PyAny> {
        match self.0 {
            ASGIMessage::HTTPRequest(event) => convert::http_request_event_into_py(py, event),
            ASGIMessage::HTTPDisconnect(event) => convert::http_disconnect_event_into_py(py, event),
            ASGIMessage::Startup(event) => convert::lifespan_startup_into_py(py, event),
            ASGIMessage::Shutdown(event) => convert::lifespan_shutdown_into_py(py, event),
            _ => PyRuntimeError::new_err(format!("Invalid message sent from server to application. {:?}", self.0))
                .into_py(py),
        }
    }
}

struct PyScope(Scope);

impl PyScope {
    pub fn new(scope: Scope) -> Self {
        Self { 0: scope }
    }
}

impl IntoPy<Py<PyAny>> for PyScope {
    fn into_py(self, py: Python<'_>) -> Py<PyAny> {
        match self.0 {
            Scope::HTTP(scope) => convert::http_scope_into_py(py, scope),
            Scope::Lifespan(scope) => convert::lifetime_scope_into_py(py, scope),
        }
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
    fn __call__(&self, message: &PyDict) -> PyResult<Py<PyAny>> {
        let rust_msg = PyASGIMessage::extract(&message)?;
        let sclone = self.send.clone();
        Python::with_gil(|py| {
            let awaitable = pyo3_asyncio::tokio::future_into_py(py, async move {
                println!("App called send");
                println!("Sending: {:?}", rust_msg.0);
                PyResult::Ok(
                    (sclone)(rust_msg.0)
                        .await
                        .map_err(|e| PyRuntimeError::new_err(format!("Error in 'send': {}", e)))?,
                )
            });
            match awaitable {
                Ok(aw) => Ok(aw.into_py(py)),
                Err(e) => Err(e),
            }
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
    fn __call__(&self) -> PyResult<Py<PyAny>> {
        let rclone = self.receive.clone();
        Python::with_gil(|py| {
            let awaitable = pyo3_asyncio::tokio::future_into_py(py, async move {
                println!("App called receive");
                let rust_out = (rclone)()
                    .await
                    .map_err(|e| PyRuntimeError::new_err(format!("Error in 'receive': {}", e)))?;
                println!("Got: {:?}", &rust_out);
                let py_message = PyASGIMessage::new(rust_out);
                PyResult::Ok(py_message)
            });
            match awaitable {
                Ok(aw) => Ok(aw.into_py(py)),
                Err(e) => Err(e),
            }
        })
    }
}

#[pyclass]
pub struct PyASGIAppWrapper {
    py_application: Py<PyAny>,
    task_locals: pyo3_asyncio::TaskLocals,
}

impl PyASGIAppWrapper {
    pub fn new(py_application: Py<PyAny>, task_locals: pyo3_asyncio::TaskLocals) -> Self {
        Self {
            py_application,
            task_locals,
        }
    }
}

impl ASGIApplication for PyASGIAppWrapper {
    async fn call(&self, scope: Scope, receive: ReceiveFn, send: SendFn) -> Result<()> {
        let future = Python::with_gil(|py| {
            let maybe_awaitable = self.py_application.call1(
                py,
                (
                    PyScope::new(scope).into_py(py),
                    PyReceive::new(receive),
                    PySend::new(send),
                ),
            );
            Ok(pyo3_asyncio::into_future_with_locals(
                &self.task_locals,
                maybe_awaitable?.as_ref(py),
            )?)
        });
        future
            .map_err(|e: PyErr| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            .await?;
        Ok(())
    }
}
