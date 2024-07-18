use log::debug;
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyDict, PyMapping, PyString},
};
use pyo3_asyncio_0_21 as pyo3_asyncio;

use aras_core::{
    self, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartupComplete, LifespanStartupFailed,
};
use aras_core::{ASGIApplication, ASGIMessage, Error, ReceiveFn, Result, Scope, SendFn};

use super::convert;

#[derive(Debug)]
pub struct PyASGIMessage(ASGIMessage);

impl PyASGIMessage {
    fn new(msg: ASGIMessage) -> Self {
        Self { 0: msg }
    }
}

impl<'source> FromPyObject<'source> for PyASGIMessage {
    fn extract_bound(ob: &Bound<'source, PyAny>) -> PyResult<Self> {
        let py_mapping: Bound<PyMapping> = ob.downcast()?.to_owned();
        let msg_type = py_mapping.get_item("type")?.downcast::<PyString>()?.to_string();

        match msg_type.as_str() {
            "http.response.start" => Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseStart(
                convert::parse_py_http_response_start(&py_mapping)?,
            ))),
            "http.response.body" => Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseBody(
                convert::parse_py_http_response_body(&py_mapping)?,
            ))),
            "lifespan.startup.complete" => Ok(PyASGIMessage::new(ASGIMessage::StartupComplete(
                LifespanStartupComplete::new(),
            ))),
            "lifespan.startup.failed" => Ok(PyASGIMessage::new(ASGIMessage::StartupFailed(
                LifespanStartupFailed::new(convert::parse_lifespan_failed_message(&py_mapping)?),
            ))),
            "lifespan.shutdown.complete" => Ok(PyASGIMessage::new(ASGIMessage::ShutdownComplete(
                LifespanShutdownComplete::new(),
            ))),
            "lifespan.shutdown.failed" => Ok(PyASGIMessage::new(ASGIMessage::ShutdownFailed(
                LifespanShutdownFailed::new(convert::parse_lifespan_failed_message(&py_mapping)?),
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
            Scope::Lifespan(scope) => convert::lifespan_scope_into_py(py, scope),
            _ => panic!("Not implemented"),
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
    async fn __call__(&self, message: Py<PyDict>) -> PyResult<()> {
        let converted_message: PyResult<PyASGIMessage> = Python::with_gil(|py: Python| message.extract(py));
        (self.send)(converted_message?.0)
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("Error in 'send': {}", e)))?;
        Ok(())
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
    async fn __call__(&self) -> PyResult<Py<PyAny>> {
        let received = (self.receive)()
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("Error in 'receive': {e}")))?;
        debug!("{:?}", received);
        let s = Python::with_gil(|py| PyResult::Ok(PyASGIMessage::new(received).into_py(py)));
        s
    }
}

#[pyclass]
#[derive(Clone)]
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

            debug!("ASGIApplication.__call__ result: {:?}", maybe_awaitable);

            // Until pyo3 implements full support for async we need to use
            // pyo3_asyncio. Migrate when possible as the pyo3 implementation
            // provides performance benefits
            Ok(pyo3_asyncio::into_future_with_locals(
                &self.task_locals,
                maybe_awaitable?.bind(py).to_owned(),
            )?)
        });
        future
            .map_err(|e: PyErr| {
                Error::custom(e.to_string())
            })?
            .await
            .map_err(|e: PyErr| {
                Error::custom(e.to_string())
            })?;

        Ok(())
    }
}
