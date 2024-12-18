import requests
from .conftest import AppContainerInfo


def test_healthy(asgi_application: AppContainerInfo) -> None:
    response = requests.get(f"{asgi_application.uri}/health_check")
    
    assert response.status_code == 200


def test_not_found(asgi_application: AppContainerInfo) -> None:
    response = requests.get(f"{asgi_application.uri}")
    
    assert response.status_code == 404
    

def test_echo_json(asgi_application: AppContainerInfo) -> None:
    data = {"Hi": "there"}
    response = requests.post(f"{asgi_application.uri}/api/basic/echo_json", json=data)
    
    assert response.status_code == 200
    assert response.json() == data


def test_echo_text(asgi_application: AppContainerInfo) -> None:
    data = "Hello"
    response = requests.get(f"{asgi_application.uri}/api/basic/echo_text?data={data}")
    
    assert response.status_code == 200
    assert response.text == data


def test_headers_ok(asgi_application: AppContainerInfo) -> None:
    response = requests.post(f"{asgi_application.uri}/api/basic/echo_json", json={"hi": "server"})
    
    assert response.status_code == 200
    assert response.headers["content-type"] == "application/json"
    assert response.headers["Content-Length"] == "15"


def test_additional_headers_ok(asgi_application: AppContainerInfo) -> None:
    response = requests.get(f"{asgi_application.uri}/api/basic/more_headers")
    
    assert response.status_code == 200
    assert response.headers["the"] == "header"
