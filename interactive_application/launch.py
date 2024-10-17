import aras
from app.main import InteractiveApplication as app


if __name__ == "__main__":
    aras.serve(app(), log_level="INFO")
