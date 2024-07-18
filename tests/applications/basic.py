from aras import Receive, Send, Scope

from .helpers import (
    read_http_body,
    get_header_value,
    send_http_response,
    parse_json_content,
    find_and_call_endpoint,
    endpoint,
    HTTPMethod,
    EndpointFunc,
    BodyT,
    ResponseType,
    JSONResponse,
    PlainTextResponse,
    HTMLResponse,
)


ENDPOINTS: dict[HTTPMethod, dict[str, EndpointFunc]] = {}


@endpoint("GET", "/echo", store=ENDPOINTS)
async def echo(query: str, data: BodyT) -> ResponseType:
    if isinstance(data, dict):
        return JSONResponse(data)
    elif isinstance(data, str):
        return PlainTextResponse(data)
    

@endpoint("GET", "/html", store=ENDPOINTS)
async def html(query: str, data: BodyT) -> HTMLResponse:
    return HTMLResponse("<h1>Hello World</h1>")


@endpoint("GET", "/", store=ENDPOINTS)
async def root(query: str, data: BodyT) -> ResponseType:
    return JSONResponse({"Hello": "world"})


@endpoint("GET", "/no_error_handling", store=ENDPOINTS)
async def no_errors_handled(query: str, data: BodyT) -> ResponseType:
    return JSONResponse({"msg": "we don't handle errors here"})


class BasicApplication:
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


async def handle_http_protocol(scope: Scope, receive: Receive, send: Send) -> None:
    
    body = await read_http_body(receive)
    
    headers: list[tuple[bytes, bytes]] = scope["headers"]
    parsed_headers = [(h[0].decode(), h[1].decode()) for h in headers]
    
    content_type = get_header_value("content-type", parsed_headers) or "text/plain"
    
    body_data: BodyT
    match content_type:
        case "application/json":
            body_data = parse_json_content(body)
        case "text/plain":
            body_data = body.decode()
        case other:
            raise ValueError(f"Unsupported content type {other}")
    
    method: HTTPMethod = scope["method"]
    response = await find_and_call_endpoint(ENDPOINTS, method, scope["path"], scope["query_string"], body_data)
    
    await send_http_response(send, response)
    

async def handle_lifespan_protocol(scope: Scope, receive: Receive, send: Send) -> None:
    
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
