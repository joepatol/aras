import multiprocessing
from typing import Generator

import pytest

from .asgi_application.app import Application

from aras.aras import serve


@pytest.fixture(scope="session", autouse=True)
def running_server() -> Generator:
    p = multiprocessing.Process(target=serve, args=(Application(), (0, 0, 0, 0), 8080, "DEBUG"))
    p.start()
    yield
    p.terminate()
