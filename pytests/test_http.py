import requests


PORT = 8001


def test_healthy() -> None:
    response = requests.get(f"http://localhost:{PORT}/api/healthchecker")
    
    assert response.status_code == 200


def test_echo_json() -> None:
    data = {"Hi": "there"}
    response = requests.post(f"http://localhost:{PORT}/api/basic/echo_json", json=data)
    
    assert response.status_code == 200
    assert response.json() == data


def test_echo_text() -> None:
    data = "Hello"
    response = requests.get(f"http://localhost:{PORT}/api/basic/echo_text?data={data}")
    
    assert response.status_code == 200
    assert response.text == data


def test_headers_ok() -> None:
    response = requests.post(f"http://localhost:{PORT}/api/basic/echo_json", json={"hi": "server"})
    
    assert response.status_code == 200
    assert response.headers["content-type"] == "application/json"
    assert response.headers["Content-Length"] == "15"
    assert response.headers["Connection"] == "Keep-Alive"
    assert response.headers["Keep-Alive"] == "timeout=5"
