use pyo3::{
    exceptions::PyValueError, prelude::*, types::{IntoPyDict, PyDict, PyString}
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
        let msg_type = py_dict.get_item("type")?.unwrap().downcast::<PyString>().unwrap().to_str()?;

        match msg_type {
            "http.response.start" => {
                let status: u16 = py_dict.get_item("status")?.unwrap().extract()?;
                let headers = py_dict.get_item("headers")?.unwrap().extract::<Vec<(Vec<u8>, Vec<u8>)>>()?;
                Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseStart(HTTPResponseStartEvent::new(
                    status.to_owned(),
                    headers.to_owned(),
                ))))
            },
            "http.response.body" => {
                let body: Vec<u8> = py_dict.get_item("body")?.unwrap().extract()?;
                let more_body: bool = py_dict.get_item("more_body")?.unwrap().extract()?;
                Ok(PyASGIMessage::new(ASGIMessage::HTTPResponseBody(HTTPResonseBodyEvent::new(body, more_body))))
            }
            _ => Err(PyValueError::new_err(format!("Invalid message type '{}'", msg_type))),
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
            let maybe_awaitable = self
                .py_application
                .call1(
                    py,
                    (
                        PyScope::new(scope).into_py_dict(py),
                        PyReceive::new(receive),
                        PySend::new(send),
                    ),
                );
                Ok(pyo3_asyncio::into_future_with_locals(&self.task_locals, maybe_awaitable?.as_ref(py)).unwrap())
        });
        future.map_err(|e: PyErr| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?.await?;
        Ok(())
    }
}
