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

- Python test suite
- Handling of errors in the server -> 500? Is it done by hyper?
- Rust tests
- Cancellation from docker quits python event loop (exiting probably should be done with channel)
- support extensions
- performance test
- add debug logs
- Support streaming responses -> max size when collect body
- Chunked data