use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use aras_core::{HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent, HTTPScope};
use pyo3::types::{PyDict, PyBytes, PyList, PyAny};

pub fn parse_py_http_response_start(py_dict: &PyDict) -> PyResult<HTTPResponseStartEvent> {
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
    Ok(HTTPResponseStartEvent::new(status, headers))
}

pub fn parse_py_http_response_body(py_dict: &PyDict) -> PyResult<HTTPResonseBodyEvent> {
    let body: Vec<u8> = py_dict
        .get_item("body")?
        .ok_or(PyValueError::new_err("Field 'body' is required"))?
        .extract()?;
    let more_body: bool = py_dict
        .get_item("more_body")?
        .ok_or(PyValueError::new_err("Field 'more_body' is required"))?
        .extract()?;
    Ok(HTTPResonseBodyEvent::new(body, more_body))
}

pub fn http_request_event_into_py(py: Python<'_>, event: HTTPRequestEvent) -> Py<PyAny> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", event.type_.into_py(py)).unwrap();
    python_result_dict.set_item("body", PyBytes::new(py, event.body.as_slice())).unwrap();
    python_result_dict.set_item("more_body", event.more_body.into_py(py)).unwrap();
    python_result_dict.into()
}

pub fn http_disconnect_event_into_py(py: Python<'_>, event: HTTPDisconnectEvent) -> Py<PyAny> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", event.type_.into_py(py)).unwrap();
    python_result_dict.into()
}

pub fn http_scope_into_py(py: Python<'_>, scope: HTTPScope) -> Py<PyAny> {
    let python_result_dict = PyDict::new(py);
    let asgi_dict = PyDict::new(py);
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
    python_result_dict.into()
}