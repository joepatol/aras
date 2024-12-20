import requests
from .conftest import AppContainerInfo


def test_create_note(asgi_application: AppContainerInfo) -> None:
    data = {
        "title": "Test Note", 
        "content": "This is a test note", 
        "published": False, 
        "createdAt": "2021-01-01T00:00:00Z", 
        "updatedAt": "2021-01-01T00:00:00Z",
        "category": "test",
    }
    response = requests.post(f"{asgi_application.uri}/api/notes", json=data)
    
    assert response.status_code == 201
    
    note_data = response.json()["note"]
    assert note_data["title"] == data["title"]
    assert note_data["content"] == data["content"]
    assert note_data["id"] is not None
    assert note_data["createdAt"] is not None
    assert note_data["updatedAt"] is not None
    assert note_data["category"] == data["category"]
    

def test_patch_note(asgi_application: AppContainerInfo) -> None:
    data = {
        "title": "Test Note", 
        "content": "This is a test note", 
        "published": False, 
        "createdAt": "2021-01-01T00:00:00Z", 
        "updatedAt": "2021-01-01T00:00:00Z",
        "category": "test",
    }
    response = requests.post(f"{asgi_application.uri}/api/notes", json=data)
    assert response.status_code == 201
    
    note_id = response.json()["note"]["id"]
    
    data = {"title": "Updated Title", "content": "Updated Content"}
    
    response = requests.patch(f"{asgi_application.uri}/api/notes/{note_id}", json=data)
    assert response.status_code == 200
