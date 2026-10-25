# @acme/kit

Small shared helpers for Acme Shop's TypeScript apps.

```ts
import { formatMoney, isOrderId, formatOrderId } from '@acme/kit';

formatMoney(1250, 'USD'); // '$12.50'
isOrderId('ord_1f2e3d4c5b6a7980'); // true
formatOrderId('ord_1f2e3d4c5b6a7980'); // 'ORD-1F2E-3D4C-5B6A-7980'
```

## Releasing

Bump `version` in `package.json`, add a `CHANGELOG.md` entry, commit, and tag
`v<version>`. Pushing the tag runs `.github/workflows/release.yml`, which
tests, builds and publishes to npm.

Consumers pin an exact version (the storefront does), so they upgrade on
their own schedule.
