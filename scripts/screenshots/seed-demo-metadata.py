"""Tags, favorites, printed marks and notes for the demo library, written straight into its fresh index."""
import json
import sqlite3
import sys

META = {
    "Twisted Vase": (["Vase mode", "Home"], 1, 1, "Print in vase mode, 0.6 mm nozzle."),
    "Knight Pawn": (["Chess", "Resin"], 1, 0, ""),
    "Royal Guard": (["Chess", "Resin"], 0, 1, ""),
    "Gearbox Kit": (["Functional"], 1, 0, "Plate 2 is optional."),
    "Spur Gear 18T": (["Functional"], 0, 1, ""),
    "Sea Serpent": (["Resin", "Display"], 0, 0, ""),
    "Infinity Knot": (["Display"], 1, 0, ""),
}

db = sqlite3.connect(sys.argv[1])
for title, (tags, favorite, printed, notes) in META.items():
    row = db.execute("SELECT library_id, path FROM models WHERE title = ?", (title,)).fetchone()
    if row is None:
        sys.exit(f"demo model missing: {title}")
    db.execute(
        "INSERT OR REPLACE INTO model_meta (library_id, path, display_name, favorite, printed, notes, tags)"
        " VALUES (?, ?, NULL, ?, ?, ?, ?)",
        (row[0], row[1], favorite, printed, notes, json.dumps(tags)),
    )
db.commit()
print("demo metadata written")
