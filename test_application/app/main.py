from fastapi import FastAPI
from starlette.types import Scope, Receive, Send
from fastapi.responses import Response, JSONResponse
from fastapi.middleware.cors import CORSMiddleware

from . import basic
from . import ws
from . import files


class CustomFastAPI(FastAPI):
    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        print(scope)
        print(scope["state"])
        scope["state"].set_item("hello", "world")
        if self.root_path:
            scope["root_path"] = self.root_path
        await super().__call__(scope, receive, send)


app = CustomFastAPI()


app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


app.include_router(basic.router, tags=["Basic"], prefix="/api/basic")
app.include_router(files.router, tags=["Files"], prefix="/api/files")
app.include_router(ws.router, tags=["Websocket"], prefix="/api/chat")

@app.get("/")
async def root() -> Response:
    return Response()


@app.get("/health_check")
async def health_check() -> JSONResponse:
    return JSONResponse({"message": "looking good!"})
