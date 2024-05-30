from typing import Any, Awaitable, Callable, TypeAlias, Protocol, Literal

Send: TypeAlias = Callable[[dict[str, Any]], Awaitable[None]]
Receive: TypeAlias = Callable[[], Awaitable[dict[str, Any]]]
Scope: TypeAlias = dict[str, Any]

LogLevel = Literal["DEBUG", "INFO", "WARN", "TRACE", "OFF", "ERROR"]

class ASGIApplication(Protocol):
    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None: ...

def serve(application: ASGIApplication, addr: tuple[int, int, int, int], port: int, log_level: LogLevel) -> None: ...
