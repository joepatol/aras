use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList, PyMapping, PyNone};

use aras_core::{ASGIScope, HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent, HTTPScope, LifespanScope, LifespanShutdown, LifespanStartup};

pub fn parse_py_http_response_start(py_map: &Bound<PyMapping>) -> PyResult<HTTPResponseStartEvent> {
    let status: u16 = py_map
        .get_item("status")?
        .extract()?;
    let headers = py_map
        .get_item("headers")
        .and_then(|v| v.extract::<Vec<(Vec<u8>, Vec<u8>)>>())
        .unwrap_or(Vec::new());
    Ok(HTTPResponseStartEvent::new(status, headers))
}

pub fn parse_py_http_response_body(py_map: &Bound<PyMapping>) -> PyResult<HTTPResonseBodyEvent> {
    let body: Vec<u8> = py_map
        .get_item("body")?
        .extract()?;
    let more_body = py_map
        .get_item("more_body")
        .and_then(|v| v.extract::<bool>())
        .unwrap_or(false);
    Ok(HTTPResonseBodyEvent::new(body, more_body))
}

pub fn parse_lifespan_failed_message(py_map: &Bound<PyMapping>) -> PyResult<String> {
    py_map.get_item("message").and_then(|v| v.extract())
}

pub fn http_request_event_into_py(py: Python<'_>, event: HTTPRequestEvent) -> Py<PyAny> {
    let python_result_dict = PyDict::new_bound(py);
    python_result_dict.set_item("type", event.type_.into_py(py)).unwrap();
    python_result_dict.set_item("body", PyBytes::new_bound(py, event.body.as_slice())).unwrap();
    python_result_dict.set_item("more_body", event.more_body.into_py(py)).unwrap();
    python_result_dict.into()
}

pub fn http_disconnect_event_into_py(py: Python<'_>, event: HTTPDisconnectEvent) -> Py<PyAny> {
    let python_result_dict = PyDict::new_bound(py);
    python_result_dict.set_item("type", event.type_.into_py(py)).unwrap();
    python_result_dict.into()
}

fn asgi_scope_into_py(py: Python<'_>, scope: ASGIScope) -> Py<PyAny> {
    let asgi_dict = PyDict::new_bound(py);
    asgi_dict.set_item("version", scope.version.into_py(py)).unwrap();
    asgi_dict.set_item("spec_version", String::from(scope.spec_version).into_py(py)).unwrap();
    asgi_dict.into()
}

pub fn http_scope_into_py(py: Python<'_>, scope: HTTPScope) -> Py<PyAny> {
    let python_result_dict = PyDict::new_bound(py);
    python_result_dict.set_item("type", scope.type_.into_py(py)).unwrap();
    python_result_dict.set_item("asgi", asgi_scope_into_py(py, scope.asgi)).unwrap();
    python_result_dict.set_item("http_version", String::from(scope.http_version).into_py(py)).unwrap();
    python_result_dict.set_item("method", scope.method.into_py(py)).unwrap();
    python_result_dict.set_item("scheme", scope.scheme.into_py(py)).unwrap();
    python_result_dict.set_item("path", scope.path.into_py(py)).unwrap();
    python_result_dict.set_item("raw_path", PyBytes::new_bound(py, &scope.raw_path)).unwrap();
    python_result_dict.set_item("query_string", PyBytes::new_bound(py, &scope.query_string)).unwrap();
    python_result_dict.set_item("root_path", scope.root_path.into_py(py)).unwrap();
    let py_bytes_headers: Vec<(Bound<PyBytes>, Bound<PyBytes>)> = scope.headers
        .into_iter()
        .map(|(k, v)| (PyBytes::new_bound(py, k.as_slice()), PyBytes::new_bound(py, v.as_slice())))
        .collect();
    python_result_dict.set_item("headers", py_bytes_headers.into_py(py)).unwrap();
    let py_client = match scope.client {
        Some(s) => PyList::new_bound(py, vec![s.0.into_py(py), s.1.into_py(py)]).to_object(py),
        None => PyNone::get_bound(py).to_object(py),
    };
    python_result_dict.set_item("client", py_client).unwrap();
    let py_server = match scope.server {
        Some(s) => PyList::new_bound(py, vec![s.0.into_py(py), s.1.into_py(py)]).to_object(py),
        None => PyNone::get_bound(py).to_object(py),
    };
    python_result_dict.set_item("server", py_server).unwrap();
    python_result_dict.into()
}

pub fn lifespan_scope_into_py(py: Python<'_>, scope: LifespanScope) -> Py<PyAny> {
    let python_result_dict = PyDict::new_bound(py);
    python_result_dict.set_item("type", scope.type_.into_py(py)).unwrap();
    python_result_dict.set_item("asgi", asgi_scope_into_py(py, scope.asgi)).unwrap();
    python_result_dict.into()
}

pub fn lifespan_startup_into_py(py: Python<'_>, event: LifespanStartup) -> Py<PyAny> {
    let python_result_dict = PyDict::new_bound(py);
    python_result_dict.set_item("type", event.type_.into_py(py)).unwrap();
    python_result_dict.into()
}

pub fn lifespan_shutdown_into_py(py: Python<'_>, event: LifespanShutdown) -> Py<PyAny> {
    let python_result_dict = PyDict::new_bound(py);
    python_result_dict.set_item("type", event.type_.into_py(py)).unwrap();
    python_result_dict.into()
}
