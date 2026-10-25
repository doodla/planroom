import { getNodeAutoInstrumentations } from '@opentelemetry/auto-instrumentations-node';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http';
import { NodeSDK } from '@opentelemetry/sdk-node';

// Traces only: spans go over OTLP/HTTP to OTEL_EXPORTER_OTLP_ENDPOINT
// (the otel-collector in platform). The storefront exports no metrics.
const sdk = new NodeSDK({
  serviceName: 'storefront',
  traceExporter: new OTLPTraceExporter(),
  instrumentations: [
    getNodeAutoInstrumentations({
      '@opentelemetry/instrumentation-fs': { enabled: false },
    }),
  ],
});

sdk.start();

process.on('SIGTERM', () => {
  sdk
    .shutdown()
    .catch((err: unknown) => console.error('telemetry shutdown failed', err))
    .finally(() => process.exit(0));
});
