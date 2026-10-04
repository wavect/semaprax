# Invoice JSON contract

Python `billing.serialize.to_json` produces the object that `web/render.js` renders.

| key | type | notes |
| --- | --- | --- |
| number | string | invoice number |
| totalCents | integer | total in cents |
