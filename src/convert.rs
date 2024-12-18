use pyo3::types::{PyBytes, PyDict, PyList, PyMapping, PyNone};
use pyo3::{prelude::*, IntoPyObjectExt};

use aras_core::{
    ASGIScope, HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent, HTTPScope,
    LifespanScope, LifespanShutdown, LifespanStartup,
};

pub fn parse_py_http_response_start(py_map: &Bound<PyMapping>) -> PyResult<HTTPResponseStartEvent> {
    let status: u16 = py_map.get_item("status")?.extract()?;
    let headers = py_map
        .get_item("headers")
        .and_then(|v| v.extract::<Vec<(Vec<u8>, Vec<u8>)>>())
        .unwrap_or(Vec::new());
    let trailers = py_map
        .get_item("trailers")
        .and_then(|v| v.extract::<bool>())
        .unwrap_or(false);
    Ok(HTTPResponseStartEvent::new(status, headers, trailers))
}

pub fn parse_py_http_response_body(py_map: &Bound<PyMapping>) -> PyResult<HTTPResonseBodyEvent> {
    let body: Vec<u8> = py_map.get_item("body")?.extract()?;
    let more_body = py_map
        .get_item("more_body")
        .and_then(|v| v.extract::<bool>())
        .unwrap_or(false);
    Ok(HTTPResonseBodyEvent::new(body, more_body))
}

pub fn parse_lifespan_failed_message(py_map: &Bound<PyMapping>) -> String {
    py_map
        .get_item("message")
        .and_then(|v| v.extract())
        .unwrap_or(String::from(""))
}

pub fn http_request_event_into_py<'py>(py: Python<'py>, event: HTTPRequestEvent) -> PyResult<Bound<'py, PyDict>> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", event.type_.into_pyobject(py)?)?;
    python_result_dict.set_item("body", PyBytes::new(py, event.body.as_slice()))?;
    python_result_dict.set_item("more_body", event.more_body.into_pyobject(py)?)?;
    Ok(python_result_dict)
}

pub fn http_disconnect_event_into_py<'py>(py: Python<'py>, event: HTTPDisconnectEvent) -> PyResult<Bound<'py, PyDict>> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", event.type_.into_pyobject(py)?)?;
    Ok(python_result_dict)
}

fn asgi_scope_into_py<'py>(py: Python<'py>, scope: ASGIScope) -> PyResult<Bound<'py, PyDict>> {
    let asgi_dict = PyDict::new(py);
    asgi_dict.set_item("version", scope.version.into_pyobject(py)?)?;
    asgi_dict.set_item("spec_version", String::from(scope.spec_version).into_pyobject(py)?)?;
    Ok(asgi_dict)
}

pub fn http_scope_into_py<'py>(py: Python<'py>, scope: HTTPScope) -> PyResult<Bound<'py, PyDict>> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", scope.type_.into_pyobject(py)?)?;
    python_result_dict.set_item("asgi", asgi_scope_into_py(py, scope.asgi)?)?;
    python_result_dict.set_item("http_version", String::from(scope.http_version).into_pyobject(py)?)?;
    python_result_dict.set_item("method", scope.method.into_pyobject(py)?)?;
    python_result_dict.set_item("scheme", scope.scheme.into_pyobject(py)?)?;
    python_result_dict.set_item("path", scope.path.into_pyobject(py)?)?;
    python_result_dict.set_item("raw_path", PyBytes::new(py, &scope.raw_path))?;
    python_result_dict.set_item("query_string", PyBytes::new(py, &scope.query_string))?;
    python_result_dict.set_item("root_path", scope.root_path.into_pyobject(py)?)?;
    let py_bytes_headers: Vec<(Bound<PyBytes>, Bound<PyBytes>)> = scope
        .headers
        .into_iter()
        .map(|(k, v)| (PyBytes::new(py, k.as_slice()), PyBytes::new(py, v.as_slice())))
        .collect();
    python_result_dict.set_item("headers", py_bytes_headers.into_pyobject(py)?)?;
    let py_client = match scope.client {
        Some(s) => PyList::new(py, vec![s.0.into_py_any(py)?, s.1.into_py_any(py)?])?.into_py_any(py),
        None => PyNone::get(py).into_py_any(py),
    };
    python_result_dict.set_item("client", py_client?)?;
    let py_server = match scope.server {
        Some(s) => PyList::new(py, vec![s.0.into_py_any(py)?, s.1.into_py_any(py)?])?.into_py_any(py),
        None => PyNone::get(py).into_py_any(py),
    };
    python_result_dict.set_item("server", py_server?)?;
    Ok(python_result_dict)
}

pub fn lifespan_scope_into_py<'py>(py: Python<'py>, scope: LifespanScope) -> PyResult<Bound<'py, PyDict>> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", scope.type_.into_pyobject(py)?)?;
    python_result_dict.set_item("asgi", asgi_scope_into_py(py, scope.asgi)?)?;
    Ok(python_result_dict)
}

pub fn lifespan_startup_into_py<'py>(py: Python<'py>, event: LifespanStartup) -> PyResult<Bound<'py, PyDict>> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", event.type_.into_pyobject(py)?)?;
    Ok(python_result_dict)
}

pub fn lifespan_shutdown_into_py<'py>(py: Python<'py>, event: LifespanShutdown) -> PyResult<Bound<'py, PyDict>> {
    let python_result_dict = PyDict::new(py);
    python_result_dict.set_item("type", event.type_.into_pyobject(py)?)?;
    Ok(python_result_dict)
}
