from typing import Any, Literal, Awaitable, Callable, Protocol
import json
from dataclasses import dataclass

from aras import Receive, Send

class ResponseType(Protocol):
    def dump(self) -> bytes:
        ...


@dataclass
class JSONResponse:
    content: dict[str, Any]
    
    def dump(self) -> bytes:
        return json.dumps(self.content).encode()


@dataclass
class PlainTextResponse:
    content: str
    
    def dump(self) -> bytes:
        return self.content.encode()
    

BodyT = dict[str, Any] | str
HTTPMethod = Literal["GET", "POST", "DELETE", "PATCH", "PUT"]
EndpointFunc = Callable[[str, BodyT], Awaitable[ResponseType]]
EndPointsDict = dict[HTTPMethod, dict[str, EndpointFunc]]


def endpoint(method: HTTPMethod, path: str, store: EndPointsDict) -> Callable[[EndpointFunc], EndpointFunc]:
    def decorator(__func: EndpointFunc) -> EndpointFunc:
        if method not in store:
            store[method] = {}
        store[method][path] = __func
        return __func
    return decorator


async def find_and_call_endpoint(endpoints: EndPointsDict, method: HTTPMethod, path: str, query: str, data: BodyT) -> ResponseType:
    try:
        func = endpoints[method][path]
    except KeyError:
        raise KeyError(f"Endpoint {method} {path} not found")

    return await func(query, data)


async def read_http_body(receive: Receive) -> bytes:
    body = bytes()
    
    while True:
        rec = await receive()
        
        if rec["type"] == "http.disconnect":
            break
        
        assert rec["type"] == "http.request", "Invalid ASGI message, expected http body"
        
        body += rec["body"]
        if rec["more_body"] is False:
            break

    return body


async def send_http_response(send: Send, data: bytes) -> None:
    await send({
        "type": "http.response.start",
        "status": 200,
        "headers": [("Content-Type".encode(), "application/json".encode())],
    })
    
    await send({
        "type": "http.response.body",
        "body": data,
        "more_body": False,
    })


def get_header_value(key: str, headers: list[tuple[str, str]]) -> str | None:
    for h in headers:
        if h[0].lower() == key.lower():
            return h[1]
    return None


def parse_json_content(body: bytes) -> dict[str, Any]:
    body_str = body.decode()
    if body_str == "":
        return {}
    return json.loads(body.decode())