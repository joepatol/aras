import aras
from app.main import app as asgi_application


if __name__ == "__main__":
    aras.serve(asgi_application)
