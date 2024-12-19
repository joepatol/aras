use std::sync::Arc;

use log::{debug, error};
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyDict, PyMapping, PyString},
};
use pyo3_async_runtimes;

use aras_core::{
    self, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartupComplete, LifespanStartupFailed,
};
use aras_core::{ASGICallable, ASGIMessage, Error, ReceiveFn, Result, Scope, SendFn};

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
                LifespanStartupFailed::new(convert::parse_lifespan_failed_message(&py_mapping)),
            ))),
            "lifespan.shutdown.complete" => Ok(PyASGIMessage::new(ASGIMessage::ShutdownComplete(
                LifespanShutdownComplete::new(),
            ))),
            "lifespan.shutdown.failed" => Ok(PyASGIMessage::new(ASGIMessage::ShutdownFailed(
                LifespanShutdownFailed::new(convert::parse_lifespan_failed_message(&py_mapping)),
            ))),
            "websocket.accept" => Ok(PyASGIMessage::new(ASGIMessage::WebsocketAccept(
                convert::parse_websocket_accept(&py_mapping)?
            ))),
            "websocket.send" => Ok(PyASGIMessage::new(ASGIMessage::WebsocketSend(
                convert::parse_websocket_send(&py_mapping)?
            ))),
            "websocket.close" => Ok(PyASGIMessage::new(ASGIMessage::WebsocketClose(
                convert::parse_websocket_close(&py_mapping)?
            ))),
            _ => {
                error!("Invalid ASGI message received from application!");
                Err(PyValueError::new_err(format!("Invalid message type '{}'", msg_type)))
            }
        }
    }
}

impl<'py> IntoPyObject<'py> for PyASGIMessage {
    type Target = PyDict;

    type Output = Bound<'py, Self::Target>;

    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> std::result::Result<Self::Output, Self::Error> {
        match self.0 {
            ASGIMessage::HTTPRequest(event) => convert::http_request_event_into_py(py, event),
            ASGIMessage::HTTPDisconnect(event) => convert::http_disconnect_event_into_py(py, event),
            ASGIMessage::Startup(event) => convert::lifespan_startup_into_py(py, event),
            ASGIMessage::Shutdown(event) => convert::lifespan_shutdown_into_py(py, event),
            ASGIMessage::WebsocketConnect(event) => convert::websocket_connect_into_py(py, event),
            ASGIMessage::WebsocketReceive(event) => convert::websocket_receive_into_py(py, event),
            ASGIMessage::WebsocketDisconnect(event) => convert::websocket_disconnect_into_py(py, event),
            _ => {
                error!("Invalid ASGI message sent from server");
                Err(PyErr::new::<PyRuntimeError, _>(format!(
                    "Invalid message sent from server to application. {:?}",
                    self.0
                )))
            }
        }
    }
}

struct PyScope(Scope);

impl PyScope {
    pub fn new(scope: Scope) -> Self {
        Self { 0: scope }
    }
}

impl<'py> IntoPyObject<'py> for PyScope {
    type Target = PyDict;

    type Output = Bound<'py, Self::Target>;

    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> std::result::Result<Self::Output, Self::Error> {
        debug!("Sending scope: {}", self.0);
        match self.0 {
            Scope::HTTP(scope) => convert::http_scope_into_py(py, scope),
            Scope::Lifespan(scope) => convert::lifespan_scope_into_py(py, scope),
            Scope::Websocket(scope) => convert::websocket_scope_into_py(py, scope),
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
        let converted_message: PyASGIMessage = Python::with_gil(|py: Python| message.extract(py))?;
        debug!("Send: {}", message);
        (self.send)(converted_message.0)
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
    async fn __call__(&self) -> PyResult<Py<PyDict>> {
        let received = (self.receive)()
            .await
            .map_err(|e| PyRuntimeError::new_err(format!("Error in 'receive': {e}")))?;
        debug!("Receive: {}", received);
        Python::with_gil(|py| {
            PyASGIMessage::new(received)
                .into_pyobject(py)
                .and_then(|v| Ok(v.unbind()))
        })
    }
}
#[derive(Clone)]
pub struct PyASGIAppWrapper {
    py_application: Arc<Py<PyAny>>,
    task_locals: Arc<pyo3_async_runtimes::TaskLocals>,
}

impl PyASGIAppWrapper {
    pub fn new(py_application: Py<PyAny>, task_locals: pyo3_async_runtimes::TaskLocals) -> Self {
        Self {
            py_application: Arc::new(py_application),
            task_locals: Arc::new(task_locals),
        }
    }
}

impl ASGICallable for PyASGIAppWrapper {
    async fn call(&self, scope: Scope, receive: ReceiveFn, send: SendFn) -> Result<()> {
        let future = Python::with_gil(|py| {
            let maybe_awaitable = self.py_application.call1(
                py,
                (
                    PyScope::new(scope).into_pyobject(py)?,
                    PyReceive::new(receive),
                    PySend::new(send),
                ),
            );

            Ok(pyo3_async_runtimes::into_future_with_locals(
                &self.task_locals,
                maybe_awaitable?.bind(py).to_owned(),
            )?)
        });
        future
            .map_err(|e: PyErr| Error::custom(e.to_string()))?
            .await
            .map_err(|e: PyErr| Error::custom(e.to_string()))?;

        Ok(())
    }
}
