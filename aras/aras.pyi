from .aras_types import ASGIApplication, LogLevel

def serve(application: ASGIApplication, addr: tuple[int, int, int, int], port: int, log_level: LogLevel) -> None: ...