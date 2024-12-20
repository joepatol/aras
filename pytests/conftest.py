from typing import Generator
from dataclasses import dataclass

import pytest
from testcontainers.core.container import DockerContainer  # type: ignore
from testcontainers.core.waiting_utils import wait_for_logs  # type: ignore


@dataclass
class AppContainerInfo:
    host_ip: str
    port: int
    
    @property
    def uri(self) -> str:
        return f"http://127.0.0.1:{self.port}"
    
    @property
    def ws_uri(self) -> str:
        return f"ws://127.0.0.1:{self.port}"


@pytest.fixture(scope="session")
def asgi_application() -> Generator[AppContainerInfo, None, None]:
    with DockerContainer("aras:latest").with_exposed_ports(8080) as container:
        _ = wait_for_logs(container, "Application startup complete")
        yield AppContainerInfo(
            host_ip=container.get_container_host_ip(),
            port=container.get_exposed_port(8080),
        )
