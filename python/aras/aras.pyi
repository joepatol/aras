from typing import Any, Awaitable, Callable, TypeAlias, Protocol

Send: TypeAlias = Callable[[bytes], Awaitable[None]]
Receive: TypeAlias = Callable[[], Awaitable[bytes | None]]
Scope: TypeAlias = dict[str, Any]

class ASGIApplication(Protocol):
    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None: ...

def serve(application: ASGIApplication) -> None: ...
