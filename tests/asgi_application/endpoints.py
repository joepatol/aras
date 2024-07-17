from typing import Any, Literal, Callable, Awaitable

SupportedMethod = Literal["GET", "POST", "DELETE"]
BodyT = dict[str, Any]
ResponseT = dict[str, Any]
EndpointFunc = Callable[[str, BodyT], Awaitable[ResponseT]]

ENDPOINTS: dict[SupportedMethod, dict[str, EndpointFunc]] = {
    "GET": {},
    "POST": {},
    "DELETE": {},
}


async def find_and_call_endpoint(method: SupportedMethod, path: str, query: str, data: BodyT) -> ResponseT:
    try:
        func = ENDPOINTS[method][path]
    except KeyError:
        raise KeyError(f"Endpoint {method} {path} not found")

    return await func(query, data)


def endpoint(method: SupportedMethod, path: str) -> Callable[[EndpointFunc], EndpointFunc]:
    def decorator(__func: EndpointFunc) -> EndpointFunc:
         ENDPOINTS[method][path] = __func
         return __func
    return decorator


@endpoint("GET", "/echo")
async def echo(_: str, data: BodyT) -> ResponseT:
    return data


@endpoint("GET", "/")
async def root(_: str, __: BodyT) -> ResponseT:
    return {"Hello": "world"}


@endpoint("GET", "/no_error_handling")
async def no_errors_handled(_: str, __: BodyT) -> ResponseT:
    return {"msg": "we don't handle errors here"}
