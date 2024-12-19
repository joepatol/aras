from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from . import basic
from . import ws


app = FastAPI()


# app.add_middleware(
#     CORSMiddleware,
#     allow_origins=["*"],
#     allow_credentials=True,
#     allow_methods=["*"],
#     allow_headers=["*"],
# )


app.include_router(basic.router, tags=["Basic"], prefix="/api/basic")
app.include_router(ws.router, tags=["Websocket"], prefix="/api/chat")

@app.get("/health_check")
async def root():
    return {"message": "looking good!"}
