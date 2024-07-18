import multiprocessing
from typing import Generator

import pytest

from .applications import BasicApplication, InvalidLifeSpanEventApp

from aras.aras import serve


# @pytest.fixture(scope="session", autouse=True)
# def server_with_basic_app() -> Generator:
#     p = multiprocessing.Process(target=serve, args=(BasicApplication(), (0, 0, 0, 0), 8000, "DEBUG"))
#     p.start()
#     yield
#     p.terminate()


@pytest.fixture
def invalid_lifespan_app() -> Generator:
    p = multiprocessing.Process(target=serve, args=(InvalidLifeSpanEventApp(), (0, 0, 0, 0), 8001, "DEBUG"))
    p.start()
    yield
    p.terminate()
