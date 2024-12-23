# A Rust ASGI Server

Work in progress!!

- Supports http 1.1
- Supports lifespan
- Support websockets

```python
import aras
from fastapi import FastAPI


app = FastAPI()


@app.get("/health_check")
async def root():
    return {"message": "looking good!"}


if __name__ == "__main__":
    aras.serve(app, log_level="INFO")
```

To do:

- Cancellation from docker quits python event loop (exiting probably should be done with channel)
- support extensions
- add debug logs
- Support streaming request/response