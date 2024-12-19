import requests
from .conftest import AppContainerInfo

from pytests.utils.arrange import ASSETS_FOLDER


def test_upload_file(asgi_application: AppContainerInfo) -> None:
    with open(str(ASSETS_FOLDER / "basic_file.txt"), 'rb') as f1:
        with open(str(ASSETS_FOLDER / "test_file.txt")) as f2:
            response = requests.post(
                f"{asgi_application.uri}/api/files/files/",
                files=[('files', f1), ('files', f2)],
            )
    
    assert response.status_code == 200
    assert response.json() == {"file_sizes": [26, 4]}
