use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyDict, PyList, PyString},
};

use aras_core::{self, ASGIMessage, HTTPResonseBodyEvent, HTTPResponseStartEvent};
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
        let msg_type = py_dict
            .get_item("type")?
            .ok_or(PyValueError::new_err("Field 'type' is required"))?
            .downcast::<PyString>()?
            .to_str()?;

        match msg_type {
            "http.response.start" => {
                let status: u16 = py_dict
                    .get_item("status")?
                    .ok_or(PyValueError::new_err("Field 'status' is required"))?
                    .extract()?;
                let headers = py_dict
                    .get_item("headers")?
                    .ok_or(PyValueError::new_err(
                        "Field 'headers' is required",
                    ))?
                    .extract::<Vec<(Vec<u8>, Vec<u8>)>>()?;
                Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseStart(
                    HTTPResponseStartEvent::new(status.to_owned(), headers.to_owned()),
                )))
            }
            "http.response.body" => {
                let body: Vec<u8> = py_dict
                    .get_item("body")?
                    .ok_or(PyValueError::new_err("Field 'body' is required"))?
                    .extract()?;
                let more_body: bool = py_dict
                    .get_item("more_body")?
                    .ok_or(PyValueError::new_err("Field 'more_body' is required"))?
                    .extract()?;
                Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseBody(
                    HTTPResonseBodyEvent::new(body, more_body),
                )))
            }
            _ => Err(PyValueError::new_err(format!("Invalid message type '{}'", msg_type))),
        }
    }
}

impl IntoPy<Py<PyAny>> for PyASGIMessage {
    fn into_py(self, py: Python<'_>) -> Py<PyAny> {
        let python_result_dict = PyDict::new(py);

        match self.0 {
            ASGIMessage::HTTPRequest(msg) => {
                python_result_dict.set_item("type", msg.type_.into_py(py)).unwrap();
                python_result_dict.set_item("body", PyBytes::new(py, msg.body.as_slice())).unwrap();
                python_result_dict.set_item("more_body", msg.more_body.into_py(py)).unwrap();
            },
            ASGIMessage::HTTPDisconnect(msg) => {
                python_result_dict.set_item("type", msg.type_.into_py(py)).unwrap();
            },
            _ => panic!("Invalid message from server to Python"),
        };
        python_result_dict.into()
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
        let python_result_dict = PyDict::new(py);
        let asgi_dict = PyDict::new(py);

        match self.0 {
            Scope::HTTP(scope) => {
                python_result_dict.set_item("type", scope.type_.into_py(py)).unwrap();
                asgi_dict.set_item("version", scope.asgi.version.into_py(py)).unwrap();
                asgi_dict.set_item("spec_version", String::from(scope.asgi.spec_version).into_py(py)).unwrap();
                python_result_dict.set_item("asgi", asgi_dict).unwrap();
                python_result_dict.set_item("http_version", String::from(scope.http_version).into_py(py)).unwrap();
                python_result_dict.set_item("method", scope.method.into_py(py)).unwrap();
                python_result_dict.set_item("scheme", scope.scheme.into_py(py)).unwrap();
                python_result_dict.set_item("path", scope.path.into_py(py)).unwrap();
                python_result_dict.set_item("raw_path", PyBytes::new(py, &scope.raw_path)).unwrap();
                python_result_dict.set_item("query_string", PyBytes::new(py, &scope.query_string)).unwrap();
                python_result_dict.set_item("root_path", scope.root_path.into_py(py)).unwrap();
                let py_bytes_headers: Vec<(&PyBytes, &PyBytes)> = scope.headers
                    .into_iter()
                    .map(|(k, v)| (PyBytes::new(py, k.as_slice()), PyBytes::new(py, v.as_slice())))
                    .collect();
                python_result_dict.set_item("headers", py_bytes_headers.into_py(py)).unwrap();
                let py_client = PyList::new(py, vec![scope.client.0.into_py(py), scope.client.1.into_py(py)]);
                python_result_dict.set_item("client", py_client).unwrap();
                let py_server = PyList::new(py, vec![scope.server.0.into_py(py), scope.server.1.into_py(py)]);
                python_result_dict.set_item("server", py_server).unwrap();
            }
        };

        python_result_dict.into()
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
        let rust_msg = PyASGIMessage::extract(&message).unwrap();
        let sclone = self.send.clone();
        Python::with_gil(|py| {
            let awaitable =
                pyo3_asyncio::tokio::future_into_py(
                    py,
                    async move { PyResult::Ok((sclone)(rust_msg.0).await.unwrap()) },
                );
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
                let rust_out = (rclone)()
                    .await
                    .map_err(|e| PyRuntimeError::new_err(format!("Error in 'receive': {}", e)))?;
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
            Ok(pyo3_asyncio::into_future_with_locals(&self.task_locals, maybe_awaitable?.as_ref(py))?)
        });
        future
            .map_err(|e: PyErr| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            .await?;
        Ok(())
    }
}
