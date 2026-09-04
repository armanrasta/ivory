# Contract sources

YAML manifests plus WAT/WASM. Deploy with **ivory-dev**, not the server binary:

```bash
ivory-dev new my-app
ivory-dev deploy my-app/contracts/tracker.yaml --chain local --key ivory-data/validator.key
```
