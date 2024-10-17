from typing import Any

import questionary
from aras import Scope, Receive, Send


OPTIONS = [
    "Call Receive function",
    "Send | lifespan.startup.complete",
    "Send | lifespan.startup.failed",
    "Send | lifespan.shutdown.complete",
    "Send | lifespan.shutdown.failed",
    "Send | http.response.start",
    "Send | http.response.body",
]


class InteractiveApplication:
    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        print(f"ASGI Application called\nReceived following scope:\n{scope}\n")
        action = await questionary.select("What do you want to do next?", choices=OPTIONS).ask_async()
        
        msg: dict[str, Any]
        while True:
            if action == "Call Receive function":
                rec = await receive()
                print(f"Received:\n{rec}\n")
            elif action == "Send | lifespan.startup.complete":
                msg = {
                    "type": "lifespan.startup.complete"
                }
                await send(msg)
            elif action == "Send | lifespan.startup.failed":
                msg = {
                    "type": "lifespan.startup.failed",
                    "message": "oops",
                }
                await send(msg)
            elif action == "Send | lifespan.shutdown.complete":
                msg = {
                    "type": "lifespan.shutdown.complete"
                }
                await send(msg)
                break
            elif action == "Send | lifespan.shutdown.failed":
                msg = {
                    "type": "lifespan.shutdown.failed"
                }
                await send(msg)
                break
            elif action == "Send | http.response.start":
                status_code: str = await questionary.text("What's the status code?", default="200").ask_async()
                headers = []
                while True:
                    header_key: str = await questionary.text("Send a header, CANCEL to stop").ask_async()
                    if header_key == "CANCEL":
                        break
                    header_value: str = await questionary.text("Which value does this header have?").ask_async()
                    headers.append((header_key.encode(), header_value.encode()))
                msg = {
                    "type": "http.response.start",
                    "status": int(status_code),
                    "headers": headers,
                    "trailers": False,
                }
                await send(msg)
            elif action == "Send | http.response.body":
                more_body = True
                while more_body:
                    body: str = await questionary.text("Which body do you want to send?").ask_async()
                    more_body = await questionary.confirm("Is there more body?").ask_async()
                    msg = {
                        "type": "http.response.body",
                        "body": body.encode("utf-8"),
                        "more_body": more_body,
                    }
                    await send(msg)
                break
            action = await questionary.select("What do you want to do next?", choices=OPTIONS).ask_async()
