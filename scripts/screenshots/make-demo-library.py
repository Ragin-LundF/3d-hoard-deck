"""English demo library with real meshes: lathe shapes, tori, gears, knots, a two-plate Bambu-style 3MF."""
import math, os, struct, sys, zipfile

root = sys.argv[1]

def write_stl(path, tris):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(b"3D Hoard Deck demo".ljust(80, b"\0") + struct.pack("<I", len(tris)))
        for a, b, c in tris:
            f.write(struct.pack("<12fH", 0, 0, 0, *a, *b, *c, 0))

def lathe(profile, segments=72, twist=0.0):
    """profile: list of (radius, z). Revolved around Z; optional twist per mm."""
    tris = []
    for i in range(segments):
        for j in range(len(profile) - 1):
            (r0, z0), (r1, z1) = profile[j], profile[j + 1]
            a0 = 2 * math.pi * i / segments; a1 = 2 * math.pi * (i + 1) / segments
            p = lambda r, z, a: (r * math.cos(a + twist * z), r * math.sin(a + twist * z), z)
            v00, v01, v10, v11 = p(r0, z0, a0), p(r0, z0, a1), p(r1, z1, a0), p(r1, z1, a1)
            tris += [(v00, v01, v11), (v00, v11, v10)]
    return tris

def tube(path_fn, radius, steps=300, sides=24):
    pts = [path_fn(t / steps) for t in range(steps + 1)]
    tris = []
    for i in range(steps):
        p0, p1 = pts[i], pts[i + 1]
        d = [p1[k] - p0[k] for k in range(3)]; n = math.sqrt(sum(x * x for x in d)) or 1; d = [x / n for x in d]
        up = (0, 0, 1) if abs(d[2]) < 0.9 else (1, 0, 0)
        u = [d[1] * up[2] - d[2] * up[1], d[2] * up[0] - d[0] * up[2], d[0] * up[1] - d[1] * up[0]]
        un = math.sqrt(sum(x * x for x in u)); u = [x / un for x in u]
        w = [d[1] * u[2] - d[2] * u[1], d[2] * u[0] - d[0] * u[2], d[0] * u[1] - d[1] * u[0]]
        ring = lambda c, s: [tuple(c[k] + radius * (math.cos(2 * math.pi * s / sides) * u[k] + math.sin(2 * math.pi * s / sides) * w[k]) for k in range(3)) for _ in [0]][0]
        for s in range(sides):
            a, b = ring(p0, s), ring(p0, s + 1); c, e = ring(p1, s), ring(p1, s + 1)
            tris += [(a, b, e), (a, e, c)]
    return tris

def gear(teeth=18, r=30, depth=4, h=8, hole=6):
    pts = []
    for i in range(teeth * 4):
        a = 2 * math.pi * i / (teeth * 4)
        rr = r + (depth if i % 4 in (1, 2) else 0)
        pts.append((rr * math.cos(a), rr * math.sin(a)))
    tris = []
    n = len(pts)
    for i in range(n):
        (x0, y0), (x1, y1) = pts[i], pts[(i + 1) % n]
        a0 = 2 * math.pi * i / n; a1 = 2 * math.pi * (i + 1) / n
        h0 = (hole * math.cos(a0), hole * math.sin(a0)); h1 = (hole * math.cos(a1), hole * math.sin(a1))
        tris += [((x0, y0, 0), (x1, y1, h), (x1, y1, 0)), ((x0, y0, 0), (x0, y0, h), (x1, y1, h))]
        for z, flip in ((0, True), (h, False)):
            q = [(h0[0], h0[1], z), (x0, y0, z), (x1, y1, z), (h1[0], h1[1], z)]
            t1, t2 = (q[0], q[1], q[2]), (q[0], q[2], q[3])
            tris += [t1[::-1], t2[::-1]] if flip else [t1, t2]
        tris += [((h0[0], h0[1], 0), (h1[0], h1[1], 0), (h1[0], h1[1], h)), ((h0[0], h0[1], 0), (h1[0], h1[1], h), (h0[0], h0[1], h))]
    return tris

pawn = [(0, 0), (14, 0), (14, 3), (11, 5), (8, 8), (6, 20), (9, 22), (5, 24), (7, 30), (6, 34), (3, 36), (0, 36.5)]
vase = [(0, 0), (22, 0), (24, 10), (28, 30), (24, 55), (16, 75), (18, 85), (0, 85)]
bowl = [(0, 0), (20, 0), (32, 8), (40, 20), (42, 24), (0, 24)]
knot = lambda t: ((20 + 8 * math.cos(3 * 2 * math.pi * t)) * math.cos(2 * 2 * math.pi * t),
                  (20 + 8 * math.cos(3 * 2 * math.pi * t)) * math.sin(2 * 2 * math.pi * t), 15 + 8 * math.sin(3 * 2 * math.pi * t))
torus = lambda t: (25 * math.cos(2 * math.pi * t), 25 * math.sin(2 * math.pi * t), 10)

models = {
    "Miniatures/Forge Masters/2024-03 - Spring Release/Knight Pawn/Supported/knight_pawn_supported.stl": lathe(pawn),
    "Miniatures/Forge Masters/2024-03 - Spring Release/Knight Pawn/Unsupported/knight_pawn.stl": lathe(pawn),
    "Miniatures/Forge Masters/2024-03 - Spring Release/Royal Guard/royal_guard.stl": lathe([(r * 1.2, z * 1.3) for r, z in pawn]),
    "Miniatures/Forge Masters/2024-06 - Summer Release/Sea Serpent/sea_serpent.stl": tube(knot, 4),
    "Miniatures/Forge Masters/2024-06 - Summer Release/Ring Wraith/ring_wraith.stl.xxx": tube(torus, 6),
    "Home/Twisted Vase/twisted_vase.stl": lathe(vase, twist=0.05),
    "Home/Fruit Bowl/fruit_bowl.stl": lathe(bowl, segments=10),
    "Home/Spiral Planter/spiral_planter.stl": lathe([(r * 1.1, z * 0.8) for r, z in vase], twist=0.12),
    "Mechanical/Spur Gear 18T/spur_gear_18t.stl": gear(),
    "Mechanical/Spur Gear 36T/spur_gear_36t.stl": gear(teeth=36, r=45),
    "Mechanical/Infinity Knot/infinity_knot.stl": tube(knot, 6),
}
for rel, tris in models.items():
    write_stl(os.path.join(root, rel), tris)

def model_xml(tris):
    verts, index, faces = [], {}, []
    for t in tris:
        ids = []
        for v in t:
            key = tuple(round(c, 4) for c in v)
            if key not in index: index[key] = len(verts); verts.append(key)
            ids.append(index[key])
        faces.append(ids)
    v = "".join(f'<vertex x="{x}" y="{y}" z="{z}"/>' for x, y, z in verts)
    f = "".join(f'<triangle v1="{a}" v2="{b}" v3="{c}"/>' for a, b, c in faces)
    return f'<?xml version="1.0"?><model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1" type="model"><mesh><vertices>{v}</vertices><triangles>{f}</triangles></mesh></object></resources></model>'

def bambu_3mf(path, plates):
    """plates: list of lists of (name, tris, x, y). Bambu puts plate N's objects at an x offset in world space."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        resources, items, config, oid = [], [], ["<config>"], 2
        for p, objects in enumerate(plates, start=1):
            config.append(f'<plate><metadata key="plater_id" value="{p}"/>')
            for name, tris, x, y in objects:
                part = f"3D/Objects/object_{oid}.model"
                z.writestr(part, model_xml(tris))
                resources.append(f'<object id="{oid}" type="model"><components><component p:path="/{part}" objectid="1"/></components></object>')
                items.append(f'<item objectid="{oid}" transform="1 0 0 0 1 0 0 0 1 {x + (p - 1) * 307} {y} 4"/>')
                config.append(f'<model_instance><metadata key="object_id" value="{oid}"/></model_instance>')
                oid += 1
            config.append("</plate>")
        config.append("</config>")
        z.writestr("3D/3dmodel.model", '<?xml version="1.0"?><model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06"><resources>' + "".join(resources) + "</resources><build>" + "".join(items) + "</build></model>")
        z.writestr("Metadata/model_settings.config", "".join(config))

bambu_3mf(os.path.join(root, "Mechanical/Gearbox Kit/gearbox_kit.3mf"), [
    [("Gear 36T", gear(teeth=36, r=45), 128, 128), ("Gear 18T", gear(), 60, 60), ("Gear 18T", gear(), 196, 196)],
    [("Knot", tube(knot, 6), 128, 128)],
])
os.makedirs(os.path.join(root, "Tabletop/Dice Tower"), exist_ok=True)
bambu_3mf(os.path.join(root, "Tabletop/Dice Tower/dice_tower.3mf"), [[("Tower", lathe([(18, 0), (18, 90), (0, 90)], segments=6), 128, 128)]])
open(os.path.join(root, "Miniatures/Forge Masters/2024-03 - Spring Release/Knight Pawn/readme.txt"), "w").write("Print at 0.05 mm layers.")
print("demo library written")
