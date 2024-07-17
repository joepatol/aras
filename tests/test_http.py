import requests


def test_hello_world() -> None:
    response = requests.get("http://localhost:8080")
    
    assert response.json() == {'Hello': 'world'}


def test_echo() -> None:
    data = {"Hi": "there"}
    response = requests.get("http://localhost:8080/echo", json=data)
    
    assert response.json() == data


def test_invalid_json_send_returns_500() -> None:
    data = {"Hi": "there"}
    response = requests.get("http://localhost:8080/no_error_handling", data=data)
    
    assert response.status_code == 500
    assert response.content == b'Internal server error'
