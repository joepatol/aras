from fastapi import APIRouter
from fastapi.responses import StreamingResponse

router = APIRouter()


async def fake_video_streamer():
    for i in range(10):
        yield b"some fake video bytes"


@router.get("/")
async def main():
    return StreamingResponse(fake_video_streamer())
