# A Rust ASGI Server

Work in progress!!

- Supports http 1.1
- Supports lifespan

```python
import aras
from fastapi import FastAPI


app = FastAPI()


@app.get("/api/healthchecker")
def root():
    return {"message": "Hello world"}


if __name__ == "__main__":
    aras.serve(app, log_level="INFO")
```

To do:

- Python test suite
- Rust tests
- websockets
- performance test
- support for `state` in `Scope`
- support http trailers