from app import models, note, basic
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from .database import engine

models.Base.metadata.create_all(bind=engine)

app = FastAPI()

origins = [
    "http://localhost:3000",
]

app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


app.include_router(note.router, tags=["Notes"], prefix="/api/notes")
app.include_router(basic.router, tags=["Basic"], prefix="/api/basic")


@app.get("/api/healthchecker")
async def root():
    return {"message": "Welcome to FastAPI with SQLAlchemy"}
