import requests


def test_hello_world() -> None:
    response = requests.get("http://localhost:8000")
    
    assert response.status_code == 200
    assert response.json() == {'Hello': 'world'}


def test_echo_json() -> None:
    data = {"Hi": "there"}
    response = requests.get("http://localhost:8000/echo", json=data)
    
    assert response.status_code == 200
    assert response.json() == data


def test_echo_text() -> None:
    data = "Hello"
    response = requests.get("http://localhost:8000/echo", data=data)
    
    assert response.status_code == 200
    assert response.text == data


def test_invalid_json_send_returns_500() -> None:
    data = {"Hi": "there"}
    response = requests.get("http://localhost:8000/no_error_handling", data=data)
    
    assert response.status_code == 500
    assert response.content == b'Internal server error'


def test_headers_ok() -> None:
    response = requests.get("http://localhost:8000/echo", data="hi")
    
    assert response.status_code == 200
    assert response.headers["content-type"] == "text/plain"
    assert response.headers["Content-Length"] == "2"
    assert response.headers["Connection"] == "Keep-Alive"
    assert response.headers["Keep-Alive"] == "timeout=5"
