import json, sys
sys.path.insert(0, ".")
from billing.models import Invoice
from billing.serialize import to_json
json.dump([to_json(Invoice("Z-9", 4200, due_date="2027-01-15")), to_json(Invoice("Z-10", 100))], open("_cross.json", "w"))
