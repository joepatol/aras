import requests
from .conftest import AppContainerInfo


def test_healthy(asgi_application: AppContainerInfo) -> None:
    response = requests.get(f"{asgi_application.uri}/health_check")
    
    assert response.status_code == 200


def test_not_found(asgi_application: AppContainerInfo) -> None:
    response = requests.get(f"{asgi_application.uri}/does_not_exist")
    
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


def test_app_raises_error(asgi_application: AppContainerInfo) -> None:
    response = requests.get(f"{asgi_application.uri}/api/basic/error")
    
    assert response.status_code == 500
    assert response.text == "Internal Server Error"


def test_state_is_persisted(asgi_application: AppContainerInfo) -> None:
    data = {"key": "value"}
    response = requests.patch(f"{asgi_application.uri}/api/basic/state", json=data)
    
    assert response.status_code == 204
    
    response = requests.get(f"{asgi_application.uri}/api/basic/state")
    
    assert response.status_code == 200
    assert response.text == "{'key': 'value'}"
