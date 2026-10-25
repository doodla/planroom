"""Telemetry: OpenTelemetry traces over OTLP, and a Prometheus counter on :9464."""

from __future__ import annotations

from opentelemetry import trace
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from prometheus_client import Counter, start_http_server

JOBS_PROCESSED = Counter(
    "worker_jobs_processed_total",
    "Jobs the worker finished, by kind and outcome.",
    ["kind", "outcome"],
)


def setup(metrics_port: int) -> trace.Tracer:
    """Export spans (to OTEL_EXPORTER_OTLP_ENDPOINT) and serve /metrics on metrics_port."""
    provider = TracerProvider(resource=Resource.create({"service.name": "worker"}))
    provider.add_span_processor(BatchSpanProcessor(OTLPSpanExporter()))
    trace.set_tracer_provider(provider)
    start_http_server(metrics_port)
    return trace.get_tracer("worker")
