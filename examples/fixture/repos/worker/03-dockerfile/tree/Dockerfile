FROM python:3.12-slim

WORKDIR /app
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1

COPY pyproject.toml ./
COPY worker ./worker
RUN pip install --no-cache-dir .

EXPOSE 9464
USER nobody
CMD ["acme-worker"]
