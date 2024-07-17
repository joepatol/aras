from typing import Any
import json

from aras import Receive, Send, Scope

from .endpoints import find_and_call_endpoint

class Application:
    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        match scope["type"]:
            case "http":
                await handle_http_protocol(scope, receive, send)
            case "lifespan":
                await handle_lifespan_protocol(scope, receive, send)
            case "websocket":
                raise NotImplementedError("Websocket protocol not supported")
            case _:
                raise RuntimeError(
                    "Invalid http scope type received from server. Must be one of "
                    f"'http', 'lifespan', 'websocket'. Got: {scope['type']}"
                )


async def read_http_body(receive: Receive) -> bytes:
    body = bytes()
    
    while True:
        rec = await receive()
        print("received:", rec)
        
        if rec["type"] == "http.disconnect":
            break
        
        assert rec["type"] == "http.request", "Invalid ASGI message, expected http body"
        
        body += rec["body"]
        if rec["more_body"] is False:
            break
    print("exit recv loop")
    return body


async def send_http_response(send: Send, data: dict[str, Any]) -> None:
    await send({
        "type": "http.response.start",
        "status": 200,
        "headers": [("Content-Type".encode(), "application/json".encode())],
    })
    
    await send({
        "type": "http.response.body",
        "body": json.dumps(data).encode(),
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


async def handle_http_protocol(scope: Scope, receive: Receive, send: Send) -> None:
    print(scope)
    body = await read_http_body(receive)
    print("Got body:", body)
    parsed_headers = [(h[0].decode(), h[1].decode()) for h in scope["headers"]]
    print(parsed_headers)
    content_type = get_header_value("content-type", parsed_headers) or "application/json"
    print(content_type)
    assert content_type == "application/json", "Unsupported content type"
    body_data = parse_json_content(body)
    
    response = await find_and_call_endpoint(scope["method"].upper(), scope["path"], scope["query_string"], body_data)
    await send_http_response(send, response)
    

async def handle_lifespan_protocol(scope: Scope, receive: Receive, send: Send) -> None:
    print(scope)
    rec = await receive()
    if rec["type"] == "lifespan.startup":
        await send({
            "type": "lifespan.startup.complete",
        })
        rec = await receive()
    if rec["type"] == "lifespan.shutdown":
        await send({
            "type": "lifespan.shutdown.complete"
        })
    else:
        raise RuntimeError(f"Unexpected lifespan message received: '{rec['type']}'")
