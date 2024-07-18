from aras import Receive, Send, Scope


class InvalidLifeSpanEventApp:
    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        match scope["type"]:
            case "lifespan":
                await handle_lifespan_protocol(scope, receive, send)
            case _:
                raise RuntimeError(f"Scope type {scope['type']} not supported")
            

async def handle_lifespan_protocol(scope: Scope, receive: Receive, send: Send) -> None:
    rec = await receive()
    
    if rec["type"] == "lifespan.startup":
        await send({
            "type": "lifespan.startup.invalid",
        })
