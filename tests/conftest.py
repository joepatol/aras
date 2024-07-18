import multiprocessing
from typing import Generator

import pytest

from .applications import BasicApplication

from aras.aras import serve


@pytest.fixture(scope="session", autouse=True)
def server_with_basic_app() -> Generator:
    p = multiprocessing.Process(target=serve, args=(BasicApplication(), (0, 0, 0, 0), 8080, "DEBUG"))
    p.start()
    yield
    p.terminate()
