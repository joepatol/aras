from typing import Any
import asyncio

from fastapi import APIRouter
from fastapi.responses import JSONResponse, PlainTextResponse

router = APIRouter()


@router.get("/echo_text")
async def echo(data: str) -> PlainTextResponse:
    return PlainTextResponse(data)


@router.post("/echo_json")
async def echo_json(data: dict[str, Any]) -> JSONResponse:
    return JSONResponse(data)


@router.get("/long_task")
async def long_task() -> JSONResponse:
    await asyncio.sleep(20.0)
    
    return JSONResponse({"task": "done"})
